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
    pub total_is_estimate: bool,
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

    fn load(
        &mut self,
        query: &str,
        group: GroupFilter,
        limit: usize,
        cursor: Option<&str>,
    ) -> ProviderResult<ClipPage>;

    fn groups(&mut self) -> ProviderResult<Vec<GroupNode>>;

    fn settings(&mut self) -> ProviderResult<SettingsSnapshot>;

    fn copy_buffers(&mut self) -> ProviderResult<Vec<CopyBufferSlot>>;

    fn paste(&mut self, clip_id: i64, mode: PasteMode) -> ProviderResult<()>;

    /// Pastes a selected batch through the core's single `clip_ids` request so
    /// the engine's ordering, stop-on-failure, interval, and suppression rules
    /// apply to the real popup path.
    fn paste_many(&mut self, clip_ids: &[i64], mode: PasteMode) -> ProviderResult<()> {
        for clip_id in clip_ids {
            self.paste(*clip_id, mode)?;
        }
        Ok(())
    }

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

    fn paste_ids(&mut self, clip_ids: &[i64], mode: PasteMode) -> ProviderResult<()> {
        let (plain_text, formats) = match mode {
            PasteMode::Original => (false, Vec::new()),
            PasteMode::PlainText => (true, Vec::new()),
            PasteMode::Html => (false, vec!["HTML Format".to_string()]),
            PasteMode::Rtf => (false, vec!["Rich Text Format".to_string()]),
            PasteMode::Files => (false, vec!["CF_HDROP".to_string()]),
        };
        let Some(first) = clip_ids.first().copied() else {
            return Ok(());
        };
        match self.call(Request::Paste {
            clip_id: first,
            plain_text,
            formats,
            clip_ids: clip_ids.to_vec(),
        })? {
            Response::PasteResult { outcome } if outcome.ok => Ok(()),
            Response::PasteResult { outcome } => Err(ProviderError::Core(outcome.message)),
            other => Err(unexpected("paste", &other)),
        }
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

    fn load(
        &mut self,
        query: &str,
        group: GroupFilter,
        limit: usize,
        cursor: Option<&str>,
    ) -> ProviderResult<ClipPage> {
        let request = query_request_for_group(query, limit, group, cursor);
        match self.call(request)? {
            Response::Clips {
                items,
                total,
                total_is_estimate,
                took_ms,
                next_cursor,
                ..
            } => {
                self.update_buffer_cache(&items);
                Ok(ClipPage {
                    items,
                    total,
                    total_is_estimate,
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
            Response::Settings { snapshot, .. } => Ok(snapshot),
            other => Err(ProviderError::InvalidResponse(format!(
                "expected Settings, received {}",
                response_name(&other)
            ))),
        }
    }

    fn copy_buffers(&mut self) -> ProviderResult<Vec<CopyBufferSlot>> {
        match self.call(Request::GetCopyBuffers)? {
            Response::CopyBuffers { slots } => {
                self.buffer_clips = [None; 5];
                for slot in &slots {
                    if (1..=5).contains(&slot.slot)
                        && let Some(clip_id) = slot.clip_id
                    {
                        self.buffer_clips[usize::from(slot.slot - 1)] = Some(clip_id);
                    }
                }
                Ok(slots)
            }
            other => Err(ProviderError::InvalidResponse(format!(
                "expected CopyBuffers, received {}",
                response_name(&other)
            ))),
        }
    }

    fn paste(&mut self, clip_id: i64, mode: PasteMode) -> ProviderResult<()> {
        self.paste_ids(&[clip_id], mode)
    }

    fn paste_many(&mut self, clip_ids: &[i64], mode: PasteMode) -> ProviderResult<()> {
        self.paste_ids(clip_ids, mode)
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

    fn assign_copy_buffer(&mut self, clip_id: i64, slot: u8) -> ProviderResult<()> {
        match self.call(Request::CopyBufferAssign {
            slot,
            clip_id: Some(clip_id),
        })? {
            Response::Ok | Response::PopupVisibility { .. } => {
                self.buffer_clips[usize::from(slot.saturating_sub(1))] = Some(clip_id);
                Ok(())
            }
            other => Err(unexpected("assign copy buffer", &other)),
        }
    }

    fn clear_copy_buffer(&mut self, slot: u8) -> ProviderResult<()> {
        match self.call(Request::CopyBufferClear { slot })? {
            Response::Ok | Response::PopupVisibility { .. } => {
                if let Some(entry) = self
                    .buffer_clips
                    .get_mut(usize::from(slot.saturating_sub(1)))
                {
                    *entry = None;
                }
                Ok(())
            }
            other => Err(unexpected("clear copy buffer", &other)),
        }
    }

    fn paste_copy_buffer(&mut self, slot: u8, plain_text: bool) -> ProviderResult<()> {
        let slots = self.copy_buffers()?;
        let clip_id = slots
            .iter()
            .find(|buffer| buffer.slot == slot)
            .and_then(|buffer| buffer.clip_id)
            .ok_or_else(|| ProviderError::Core(format!("copy buffer {slot} is empty")))?;
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
    query_request_for_group(query, limit, GroupFilter::All, None)
}

fn query_request_for_group(
    query: &str,
    limit: usize,
    group: GroupFilter,
    cursor: Option<&str>,
) -> Request {
    let limit = u32::try_from(limit).unwrap_or(u32::MAX);
    let group_id = match group {
        GroupFilter::All => None,
        GroupFilter::Ungrouped => Some(-1),
        GroupFilter::Group(group_id) => Some(group_id),
    };
    if query.trim().is_empty() {
        Request::GetClips {
            query: String::new(),
            group_id,
            limit,
            cursor: cursor.map(ToString::to_string),
            sort: ditto_model::ClipSort::Recent,
        }
    } else {
        Request::Search {
            query: query.to_string(),
            group_id,
            mode: ditto_model::SearchMode::Contains,
            sort: ditto_model::ClipSort::Recent,
            limit,
            cursor: cursor.map(ToString::to_string),
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
        Response::Clip { .. } => "Clip",
        Response::Groups { .. } => "Groups",
        Response::CopyBuffers { .. } => "CopyBuffers",
        Response::Settings { .. } => "Settings",
        Response::PasteResult { .. } => "PasteResult",
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
            Request::Search { query, limit: 200, .. } if query == "needle"
        ));
    }

    #[test]
    fn group_filters_are_forwarded_to_the_core() {
        assert!(matches!(
            query_request_for_group("", 500, GroupFilter::All, None),
            Request::GetClips { group_id: None, .. }
        ));
        assert!(matches!(
            query_request_for_group("", 500, GroupFilter::Ungrouped, None),
            Request::GetClips {
                group_id: Some(-1),
                ..
            }
        ));
        assert!(matches!(
            query_request_for_group("", 500, GroupFilter::Group(42), None),
            Request::GetClips {
                group_id: Some(42),
                ..
            }
        ));
        assert!(matches!(
            query_request_for_group("needle", 500, GroupFilter::Group(42), None),
            Request::Search {
                group_id: Some(42),
                ..
            }
        ));
    }
}
