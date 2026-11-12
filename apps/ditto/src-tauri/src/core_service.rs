use std::{path::Path, sync::Arc};

use ditto_clipboard::{CaptureConfig, CapturedClip, ClipboardMonitor};
use ditto_ipc::{ErrorCode, Event, Request, Response};
use ditto_model::{ClipQuery, SearchMode};
use ditto_storage::{Storage, StorageError};
use ditto_win::PipeServer;
use parking_lot::Mutex;

type BroadcastSlot = Arc<Mutex<Option<PipeServer>>>;

pub struct CoreService {
    storage: Storage,
    pipe: PipeServer,
    clipboard: ClipboardMonitor,
}

impl CoreService {
    pub fn start(pipe_name: &str, storage: Storage) -> ditto_model::Result<Self> {
        let settings = storage
            .get_settings()
            .map_err(ditto_model::DittoError::from)?;
        let capture_config = capture_config(&settings.values);
        let broadcast_slot = Arc::new(Mutex::new(None::<PipeServer>));
        let storage_for_requests = storage.clone();
        let broadcasts_for_requests = Arc::clone(&broadcast_slot);
        let pipe = PipeServer::start_with_handler(pipe_name, move |request| {
            handle_request(&storage_for_requests, request, &broadcasts_for_requests)
        })
        .map_err(|error| ditto_model::DittoError::Ipc(error.to_string()))?;
        *broadcast_slot.lock() = Some(pipe.clone());

        let storage_for_capture = storage.clone();
        let pipe_for_capture = pipe.clone();
        let clipboard = ClipboardMonitor::start_with_config(capture_config, move |captured| {
            persist_capture(&storage_for_capture, &pipe_for_capture, captured);
        })
        .map_err(ditto_model::DittoError::from)?;

        Ok(Self {
            storage,
            pipe,
            clipboard,
        })
    }

    pub fn pipe(&self) -> &PipeServer {
        &self.pipe
    }

    pub fn stop(&mut self) {
        self.clipboard.stop();
        let _ = self.storage.checkpoint();
    }
}

fn persist_capture(storage: &Storage, pipe: &PipeServer, captured: CapturedClip) {
    if !captured.warnings.is_empty() {
        log::warn!(
            "clipboard capture completed with {} format warnings",
            captured.warnings.len()
        );
    }
    let settings = storage.get_settings().ok();
    let allow_duplicates = settings
        .as_ref()
        .and_then(|settings| settings.values.pointer("/clipboard/allow_duplicates"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let new_clip = captured.into_new_clip();
    match storage.insert_clip(&new_clip, allow_duplicates) {
        Ok(outcome) if outcome.inserted => {
            let event = Event::ClipAdded { clip: outcome.clip };
            log_broadcast(pipe.broadcast(&event), "ClipAdded");
        }
        Ok(outcome) => {
            let event = Event::ClipUpdated {
                id: outcome.clip.id,
                fields: vec!["duplicate".to_string()],
                state_rev: outcome.clip.state_rev,
            };
            log_broadcast(pipe.broadcast(&event), "ClipUpdated");
        }
        Err(error) => {
            log::error!("clipboard capture insert failed: {error}");
        }
    }
}

pub(crate) fn handle_request(
    storage: &Storage,
    request: &Request,
    broadcasts: &BroadcastSlot,
) -> Response {
    let method = request_method(request);
    let started = std::time::Instant::now();
    let response = match request {
        Request::GetClips {
            query,
            group_id,
            limit,
            cursor,
            sort,
        } => search_response(
            storage,
            ClipQuery {
                query: query.clone(),
                group_id: *group_id,
                mode: SearchMode::Contains,
                sort: *sort,
                limit: *limit,
                cursor: cursor.clone(),
                include_preview: true,
            },
        ),
        Request::Search {
            query,
            group_id,
            mode,
            sort,
            limit,
            cursor,
        } => search_response(
            storage,
            ClipQuery {
                query: query.clone(),
                group_id: *group_id,
                mode: *mode,
                sort: *sort,
                limit: *limit,
                cursor: cursor.clone(),
                include_preview: true,
            },
        ),
        Request::GetClip { clip_id } => match storage.get_clip(*clip_id) {
            Ok(clip) => Response::Clip {
                clip: Box::new(clip),
            },
            Err(error) => storage_error(error),
        },
        Request::Paste { .. } => Response::Error {
            code: ErrorCode::NotImplemented,
            message: "paste engine is owned by T4".to_string(),
            retryable: false,
        },
        Request::SetPinned { clip_id, pinned } => match storage.set_pinned(*clip_id, *pinned) {
            Ok(_) => {
                broadcast_clip_updated(storage, broadcasts, *clip_id, "pinned");
                Response::Ok
            }
            Err(error) => storage_error(error),
        },
        Request::MoveToGroup { clip_id, group_id } => {
            match storage.move_clips_to_group(&[*clip_id], *group_id) {
                Ok(0) => storage_error(StorageError::NotFound(format!("clip {clip_id}"))),
                Ok(_) => {
                    broadcast_clip_updated(storage, broadcasts, *clip_id, "group_id");
                    Response::Ok
                }
                Err(error) => storage_error(error),
            }
        }
        Request::Delete { clip_ids } => match storage.soft_delete_clips(clip_ids) {
            Ok(deleted) => {
                if deleted > 0 {
                    let state_rev = storage.state_rev().unwrap_or_default();
                    log_broadcast(
                        broadcast(
                            broadcasts,
                            Event::ClipDeleted {
                                ids: clip_ids.clone(),
                                state_rev,
                            },
                        ),
                        "ClipDeleted",
                    );
                }
                Response::Ok
            }
            Err(error) => storage_error(error),
        },
        Request::GetGroups => match storage.list_groups() {
            Ok(groups) => Response::Groups { groups },
            Err(error) => storage_error(error),
        },
        Request::CreateGroup {
            name,
            parent_id,
            sort_order,
        } => match storage.create_group(name, *parent_id, *sort_order) {
            Ok(_) => {
                broadcast_group_changed(storage, broadcasts);
                Response::Ok
            }
            Err(error) => storage_error(error),
        },
        Request::RenameGroup { group_id, name } => match storage.rename_group(*group_id, name) {
            Ok(()) => {
                broadcast_group_changed(storage, broadcasts);
                Response::Ok
            }
            Err(error) => storage_error(error),
        },
        Request::DeleteGroup { group_id } => match storage.delete_group(*group_id) {
            Ok(()) => {
                broadcast_group_changed(storage, broadcasts);
                Response::Ok
            }
            Err(error) => storage_error(error),
        },
        Request::MoveGroup {
            group_id,
            parent_id,
            sort_order,
        } => match storage.move_group(*group_id, *parent_id, *sort_order) {
            Ok(()) => {
                broadcast_group_changed(storage, broadcasts);
                Response::Ok
            }
            Err(error) => storage_error(error),
        },
        Request::GetCopyBuffers => match storage.copy_buffers() {
            Ok(slots) => Response::CopyBuffers { slots },
            Err(error) => storage_error(error),
        },
        Request::CopyBufferAssign { slot, clip_id } => {
            match storage.assign_copy_buffer(*slot, *clip_id) {
                Ok(()) => {
                    if let Some(clip_id) = clip_id {
                        broadcast_clip_updated(storage, broadcasts, *clip_id, "copy_buffer");
                    }
                    Response::Ok
                }
                Err(error) => storage_error(error),
            }
        }
        Request::CopyBufferClear { slot } => match storage.clear_copy_buffer(*slot) {
            Ok(()) => Response::Ok,
            Err(error) => storage_error(error),
        },
        Request::GetSettings => match storage.get_settings() {
            Ok(snapshot) => Response::Settings {
                snapshot,
                restart_required: false,
            },
            Err(error) => storage_error(error),
        },
        Request::SetSettings { values } => {
            let keys = setting_keys(values);
            match storage.set_settings(values) {
                Ok(snapshot) => {
                    let revision = snapshot.revision;
                    log_broadcast(
                        broadcast(
                            broadcasts,
                            Event::SettingsChanged {
                                keys: keys.clone(),
                                revision,
                            },
                        ),
                        "SettingsChanged",
                    );
                    Response::Settings {
                        snapshot,
                        restart_required: keys.iter().any(|key| {
                            matches!(
                                key.as_str(),
                                "general.language" | "general.run_at_startup" | "data.path"
                            )
                        }),
                    }
                }
                Err(error) => storage_error(error),
            }
        }
        Request::Shutdown => Response::Ok,
        Request::Hello { .. } | Request::ShowPopup | Request::HidePopup => Response::Error {
            code: ErrorCode::Internal,
            message: "control request bypassed the pipe dispatcher".to_string(),
            retryable: true,
        },
    };
    log::debug!(
        "IPC request handled: method={method} took_ms={} result={}",
        started.elapsed().as_millis() as u64,
        response_kind(&response)
    );
    response
}

fn search_response(storage: &Storage, query: ClipQuery) -> Response {
    match storage.search_clips(&query) {
        Ok(page) => Response::Clips {
            items: page.items,
            total: page.total,
            total_is_estimate: page.total_is_estimate,
            took_ms: page.took_ms,
            next_cursor: page.next_cursor,
        },
        Err(error) => storage_error(error),
    }
}

fn storage_error(error: StorageError) -> Response {
    let (code, retryable) = match &error {
        StorageError::NotFound(_) => (ErrorCode::NotFound, false),
        StorageError::Conflict(_) | StorageError::InvalidData(_) => (ErrorCode::Conflict, false),
        StorageError::Sqlite(error) => {
            let message = error.to_string().to_ascii_lowercase();
            if message.contains("constraint") || message.contains("unique") {
                (ErrorCode::Conflict, false)
            } else {
                (
                    ErrorCode::Database,
                    message.contains("locked") || message.contains("busy"),
                )
            }
        }
        StorageError::Migration(_) => (ErrorCode::NotReady, true),
        StorageError::Io(_) | StorageError::Json(_) => (ErrorCode::Internal, true),
    };
    Response::Error {
        code,
        message: error.to_string(),
        retryable,
    }
}

fn broadcast_clip_updated(
    storage: &Storage,
    broadcasts: &BroadcastSlot,
    clip_id: i64,
    field: &str,
) {
    let state_rev = storage.state_rev().unwrap_or_default();
    log_broadcast(
        broadcast(
            broadcasts,
            Event::ClipUpdated {
                id: clip_id,
                fields: vec![field.to_string()],
                state_rev,
            },
        ),
        "ClipUpdated",
    );
}

fn broadcast_group_changed(storage: &Storage, broadcasts: &BroadcastSlot) {
    let state_rev = storage.state_rev().unwrap_or_default();
    log_broadcast(
        broadcast(broadcasts, Event::GroupChanged { state_rev }),
        "GroupChanged",
    );
}

fn broadcast(broadcasts: &BroadcastSlot, event: Event) -> ditto_model::Result<()> {
    let pipe = broadcasts.lock().clone();
    pipe.map_or(Ok(()), |pipe| pipe.broadcast(&event))
}

fn log_broadcast(result: ditto_model::Result<()>, event: &str) {
    if let Err(error) = result {
        log::warn!("IPC broadcast failed for {event}: {error}");
    }
}

fn capture_config(values: &serde_json::Value) -> CaptureConfig {
    let defaults = CaptureConfig::default();
    let clipboard = values.get("clipboard");
    CaptureConfig {
        debounce_ms: clipboard
            .and_then(|value| value.get("process_delay_ms"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(defaults.debounce_ms),
        max_clip_bytes: clipboard
            .and_then(|value| value.get("max_clip_bytes"))
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(defaults.max_clip_bytes),
        max_image_bytes: clipboard
            .and_then(|value| value.get("max_image_bytes"))
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(defaults.max_image_bytes),
        ignored_formats: clipboard
            .and_then(|value| value.get("ignored_formats"))
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn setting_keys(value: &serde_json::Value) -> Vec<String> {
    let mut keys = Vec::new();
    collect_setting_keys("", value, &mut keys);
    keys.sort();
    keys
}

fn collect_setting_keys(prefix: &str, value: &serde_json::Value, output: &mut Vec<String>) {
    if let Some(object) = value.as_object() {
        for (key, child) in object {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            collect_setting_keys(&path, child, output);
        }
    } else if !prefix.is_empty() {
        output.push(prefix.to_string());
    }
}

fn request_method(request: &Request) -> &'static str {
    match request {
        Request::Hello { .. } => "Hello",
        Request::ShowPopup => "ShowPopup",
        Request::HidePopup => "HidePopup",
        Request::GetClips { .. } => "GetClips",
        Request::Search { .. } => "Search",
        Request::GetClip { .. } => "GetClip",
        Request::Paste { .. } => "Paste",
        Request::SetPinned { .. } => "SetPinned",
        Request::MoveToGroup { .. } => "MoveToGroup",
        Request::Delete { .. } => "Delete",
        Request::GetGroups => "GetGroups",
        Request::CreateGroup { .. } => "CreateGroup",
        Request::RenameGroup { .. } => "RenameGroup",
        Request::DeleteGroup { .. } => "DeleteGroup",
        Request::MoveGroup { .. } => "MoveGroup",
        Request::GetCopyBuffers => "GetCopyBuffers",
        Request::CopyBufferAssign { .. } => "CopyBufferAssign",
        Request::CopyBufferClear { .. } => "CopyBufferClear",
        Request::GetSettings => "GetSettings",
        Request::SetSettings { .. } => "SetSettings",
        Request::Shutdown => "Shutdown",
    }
}

fn response_kind(response: &Response) -> &'static str {
    match response {
        Response::HelloAck { .. } => "HelloAck",
        Response::PopupVisibility { .. } => "PopupVisibility",
        Response::Clips { .. } => "Clips",
        Response::Clip { .. } => "Clip",
        Response::Groups { .. } => "Groups",
        Response::CopyBuffers { .. } => "CopyBuffers",
        Response::Settings { .. } => "Settings",
        Response::Ok => "Ok",
        Response::Error { .. } => "Error",
    }
}

pub fn resolve_data_root(exe_dir: &Path) -> ditto_model::Result<ditto_model::DataRoot> {
    ditto_model::DataRoot::resolve(None, false, exe_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_path_returns_groups_and_settings_from_temp_storage() {
        let temp = tempfile::tempdir().expect("temp root");
        let storage = Storage::open_data_root(temp.path()).expect("storage");
        let broadcasts = Arc::new(Mutex::new(None));
        assert!(matches!(
            handle_request(&storage, &Request::GetGroups, &broadcasts),
            Response::Groups { groups } if groups.is_empty()
        ));
        assert!(matches!(
            handle_request(&storage, &Request::GetSettings, &broadcasts),
            Response::Settings { .. }
        ));
        assert!(matches!(
            handle_request(
                &storage,
                &Request::Paste {
                    clip_id: 1,
                    plain_text: false
                },
                &broadcasts
            ),
            Response::Error {
                code: ErrorCode::NotImplemented,
                ..
            }
        ));
    }
}
