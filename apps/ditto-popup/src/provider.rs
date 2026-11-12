use std::fmt;

use ditto_ipc::{ClientKind, ErrorCode, Request, Response};
use ditto_model::{ClipSummary, CopyBufferSlot, GroupNode, SettingsSnapshot};
use ditto_win::{PipeClient, hello_request, pipe_name};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupFilter {
    All,
    Ungrouped,
    Group(i64),
}

impl GroupFilter {
    pub fn matches(self, clip: &ClipSummary) -> bool {
        match self {
            Self::All => true,
            Self::Ungrouped => clip.group_id.is_none(),
            Self::Group(group_id) => clip.group_id == Some(group_id),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteMode {
    Original,
    PlainText,
    Html,
    Rtf,
    Files,
}

#[derive(Debug, Clone)]
pub struct ClipPage {
    pub items: Vec<ClipSummary>,
    pub total: u64,
    pub took_ms: u64,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    Disconnected(String),
    Unsupported(String),
    Core(String),
    InvalidResponse(String),
}

impl ProviderError {
    pub const fn is_unsupported(&self) -> bool {
        matches!(self, Self::Unsupported(_))
    }

    pub const fn is_disconnected(&self) -> bool {
        matches!(self, Self::Disconnected(_))
    }
}

impl fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected(message)
            | Self::Unsupported(message)
            | Self::Core(message)
            | Self::InvalidResponse(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ProviderError {}

pub type ProviderResult<T> = Result<T, ProviderError>;

pub trait ClipProvider: Send {
    fn name(&self) -> &'static str;

    fn load(&mut self, query: &str, group: GroupFilter, limit: usize) -> ProviderResult<ClipPage>;

    fn groups(&mut self) -> ProviderResult<Vec<GroupNode>>;

    fn settings(&mut self) -> ProviderResult<SettingsSnapshot>;

    fn copy_buffers(&mut self) -> ProviderResult<Vec<CopyBufferSlot>>;

    fn paste(&mut self, clip_id: i64, mode: PasteMode) -> ProviderResult<()>;

    fn set_pinned(&mut self, clip_id: i64, pinned: bool) -> ProviderResult<()>;

    fn move_to_group(&mut self, clip_id: i64, group_id: Option<i64>) -> ProviderResult<()>;

    fn delete(&mut self, clip_ids: &[i64]) -> ProviderResult<()>;

    fn assign_copy_buffer(&mut self, clip_id: i64, slot: u8) -> ProviderResult<()>;

    fn clear_copy_buffer(&mut self, slot: u8) -> ProviderResult<()>;

    fn paste_copy_buffer(&mut self, slot: u8, plain_text: bool) -> ProviderResult<()>;
}

pub struct IpcProvider {
    client: Option<PipeClient>,
    next_request_id: u64,
    buffer_clips: [Option<i64>; 5],
}

impl Default for IpcProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl IpcProvider {
    pub const fn new() -> Self {
        Self {
            client: None,
            next_request_id: 2,
            buffer_clips: [None; 5],
        }
    }

    fn ensure_connected(&mut self) -> ProviderResult<()> {
        if self.client.is_some() {
            return Ok(());
        }
        let name = pipe_name().map_err(|error| ProviderError::Disconnected(error.to_string()))?;
        let client = PipeClient::connect(&name)
            .map_err(|error| ProviderError::Disconnected(error.to_string()))?;
        client
            .send(1, &hello_request(ClientKind::Popup))
            .map_err(|error| ProviderError::Disconnected(error.to_string()))?;
        match client
            .read_response()
            .map_err(|error| ProviderError::Disconnected(error.to_string()))?
        {
            Response::HelloAck { .. } => {
                self.client = Some(client);
                Ok(())
            }
            Response::Error { message, .. } => Err(ProviderError::Disconnected(format!(
                "hello rejected: {message}"
            ))),
            _ => Err(ProviderError::InvalidResponse(
                "unexpected response to Hello".to_string(),
            )),
        }
    }

    fn call(&mut self, request: Request) -> ProviderResult<Response> {
        let mut last_error = None;
        for _ in 0..2 {
            self.ensure_connected()?;
            let request_id = self.next_request_id;
            self.next_request_id = self.next_request_id.saturating_add(1);
            let Some(client) = self.client.as_ref() else {
                return Err(ProviderError::Disconnected(
                    "named pipe client is not connected".to_string(),
                ));
            };
            if let Err(error) = client.send(request_id, &request) {
                last_error = Some(ProviderError::Disconnected(error.to_string()));
                self.client = None;
                continue;
            }
            match client.read_response() {
                Ok(Response::Error {
                    code,
                    message,
                    retryable,
                }) => {
                    let error = match code {
                        ErrorCode::NotImplemented => ProviderError::Unsupported(message),
                        ErrorCode::NotReady | ErrorCode::Busy if retryable => {
                            ProviderError::Disconnected(message)
                        }
                        _ => ProviderError::Core(format!("{}: {message}", code.wire_name())),
                    };
                    if retryable {
                        self.client = None;
                        last_error = Some(error);
                        continue;
                    }
                    return Err(error);
                }
                Ok(response) => return Ok(response),
                Err(error) => {
                    last_error = Some(ProviderError::Disconnected(error.to_string()));
                    self.client = None;
                }
            }
        }
        Err(last_error.unwrap_or_else(|| {
            ProviderError::Disconnected("named pipe request failed".to_string())
        }))
    }

    fn update_buffer_cache(&mut self, items: &[ClipSummary]) {
        self.buffer_clips = [None; 5];
        for item in items {
            if let Some(slot) = item.copy_buffer
                && (1..=5).contains(&slot)
            {
                self.buffer_clips[usize::from(slot - 1)] = Some(item.id);
            }
        }
    }
}

impl ClipProvider for IpcProvider {
    fn name(&self) -> &'static str {
        "ipc"
    }

    fn load(&mut self, query: &str, group: GroupFilter, limit: usize) -> ProviderResult<ClipPage> {
        let request = query_request(query, limit);
        match self.call(request)? {
            Response::Clips {
                mut items,
                mut total,
                took_ms,
                next_cursor,
            } => {
                if group != GroupFilter::All {
                    items.retain(|clip| group.matches(clip));
                    total = items.len() as u64;
                }
                self.update_buffer_cache(&items);
                Ok(ClipPage {
                    items,
                    total,
                    took_ms,
                    next_cursor,
                })
            }
            other => Err(ProviderError::InvalidResponse(format!(
                "expected Clips, received {}",
                response_name(&other)
            ))),
        }
    }

    fn groups(&mut self) -> ProviderResult<Vec<GroupNode>> {
        match self.call(Request::GetGroups)? {
            Response::Groups { groups } => Ok(groups),
            other => Err(ProviderError::InvalidResponse(format!(
                "expected Groups, received {}",
                response_name(&other)
            ))),
        }
    }

    fn settings(&mut self) -> ProviderResult<SettingsSnapshot> {
        match self.call(Request::GetSettings)? {
            Response::Settings { values } => Ok(values),
            other => Err(ProviderError::InvalidResponse(format!(
                "expected Settings, received {}",
                response_name(&other)
            ))),
        }
    }

    fn copy_buffers(&mut self) -> ProviderResult<Vec<CopyBufferSlot>> {
        Ok(self
            .buffer_clips
            .iter()
            .enumerate()
            .map(|(index, clip_id)| CopyBufferSlot {
                slot: index as u8 + 1,
                name: format!("Buffer {}", index + 1),
                clip_id: *clip_id,
            })
            .collect())
    }

    fn paste(&mut self, clip_id: i64, mode: PasteMode) -> ProviderResult<()> {
        match mode {
            PasteMode::Original | PasteMode::PlainText => {
                let plain_text = mode == PasteMode::PlainText;
                match self.call(Request::Paste {
                    clip_id,
                    plain_text,
                })? {
                    Response::Ok | Response::PopupVisibility { .. } => Ok(()),
                    other => Err(unexpected("paste", &other)),
                }
            }
            PasteMode::Html | PasteMode::Rtf | PasteMode::Files => Err(ProviderError::Unsupported(
                "format-specific paste requires a protocol extension".to_string(),
            )),
        }
    }

    fn set_pinned(&mut self, clip_id: i64, pinned: bool) -> ProviderResult<()> {
        match self.call(Request::SetPinned { clip_id, pinned })? {
            Response::Ok | Response::PopupVisibility { .. } => Ok(()),
            other => Err(unexpected("pin", &other)),
        }
    }

    fn move_to_group(&mut self, clip_id: i64, group_id: Option<i64>) -> ProviderResult<()> {
        match self.call(Request::MoveToGroup { clip_id, group_id })? {
            Response::Ok | Response::PopupVisibility { .. } => Ok(()),
            other => Err(unexpected("move", &other)),
        }
    }

    fn delete(&mut self, clip_ids: &[i64]) -> ProviderResult<()> {
        match self.call(Request::Delete {
            clip_ids: clip_ids.to_vec(),
        })? {
            Response::Ok | Response::PopupVisibility { .. } => Ok(()),
            other => Err(unexpected("delete", &other)),
        }
    }

    fn assign_copy_buffer(&mut self, _clip_id: i64, _slot: u8) -> ProviderResult<()> {
        Err(ProviderError::Unsupported(
            "copy-buffer assignment requires a protocol extension".to_string(),
        ))
    }

    fn clear_copy_buffer(&mut self, _slot: u8) -> ProviderResult<()> {
        Err(ProviderError::Unsupported(
            "copy-buffer clearing requires a protocol extension".to_string(),
        ))
    }

    fn paste_copy_buffer(&mut self, slot: u8, plain_text: bool) -> ProviderResult<()> {
        let index = usize::from(slot.saturating_sub(1));
        let clip_id = self
            .buffer_clips
            .get(index)
            .copied()
            .flatten()
            .ok_or_else(|| ProviderError::Unsupported(format!("copy buffer {slot} is empty")))?;
        self.paste(
            clip_id,
            if plain_text {
                PasteMode::PlainText
            } else {
                PasteMode::Original
            },
        )
    }
}

pub fn query_request(query: &str, limit: usize) -> Request {
    let limit = u32::try_from(limit).unwrap_or(u32::MAX);
    if query.trim().is_empty() {
        Request::GetClips {
            query: String::new(),
            limit,
        }
    } else {
        Request::Search {
            query: query.to_string(),
            limit,
        }
    }
}

fn unexpected(operation: &str, response: &Response) -> ProviderError {
    ProviderError::InvalidResponse(format!("{operation} returned {}", response_name(response)))
}

const fn response_name(response: &Response) -> &'static str {
    match response {
        Response::HelloAck { .. } => "HelloAck",
        Response::PopupVisibility { .. } => "PopupVisibility",
        Response::Clips { .. } => "Clips",
        Response::Groups { .. } => "Groups",
        Response::Settings { .. } => "Settings",
        Response::Ok => "Ok",
        Response::Error { .. } => "Error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_requests_recent_clips() {
        assert!(matches!(
            query_request("   ", 200),
            Request::GetClips { limit: 200, .. }
        ));
    }

    #[test]
    fn non_empty_query_requests_search() {
        assert!(matches!(
            query_request("needle", 200),
            Request::Search { query, limit: 200 } if query == "needle"
        ));
    }
}
