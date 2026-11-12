use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use ditto_model::{CopyBufferSlot, GroupNode, SettingsSnapshot};

use crate::provider::{
    ClipPage, ClipProvider, GroupFilter, PasteMode, ProviderError, ProviderResult,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    Paste,
    Pin,
    Move,
    Delete,
    AssignCopyBuffer,
    ClearCopyBuffer,
    PasteCopyBuffer,
}

#[derive(Debug, Clone)]
pub enum ProviderAction {
    Paste { clip_ids: Vec<i64>, mode: PasteMode },
    SetPinned { clip_id: i64, pinned: bool },
    MoveToGroup { clip_id: i64, group_id: Option<i64> },
    Delete { clip_ids: Vec<i64> },
    AssignCopyBuffer { clip_id: i64, slot: u8 },
    ClearCopyBuffer { slot: u8 },
    PasteCopyBuffer { slot: u8, plain_text: bool },
}

impl ProviderAction {
    pub const fn kind(&self) -> ActionKind {
        match self {
            Self::Paste { .. } => ActionKind::Paste,
            Self::SetPinned { .. } => ActionKind::Pin,
            Self::MoveToGroup { .. } => ActionKind::Move,
            Self::Delete { .. } => ActionKind::Delete,
            Self::AssignCopyBuffer { .. } => ActionKind::AssignCopyBuffer,
            Self::ClearCopyBuffer { .. } => ActionKind::ClearCopyBuffer,
            Self::PasteCopyBuffer { .. } => ActionKind::PasteCopyBuffer,
        }
    }

    pub const fn refreshes_list(&self) -> bool {
        matches!(
            self,
            Self::SetPinned { .. }
                | Self::MoveToGroup { .. }
                | Self::Delete { .. }
                | Self::AssignCopyBuffer { .. }
                | Self::ClearCopyBuffer { .. }
        )
    }
}

pub enum ProviderCommand {
    Load {
        request_id: u64,
        query: String,
        group: GroupFilter,
        limit: usize,
        cursor: Option<String>,
    },
    RefreshMetadata,
    Action(ProviderAction),
}

#[derive(Debug)]
pub struct ProviderMetadata {
    pub groups: Vec<GroupNode>,
    pub settings: SettingsSnapshot,
    pub copy_buffers: Vec<CopyBufferSlot>,
}

pub enum ProviderEvent {
    Loaded {
        request_id: u64,
        result: ProviderResult<ClipPage>,
    },
    Metadata(ProviderResult<ProviderMetadata>),
    ActionDone {
        action: ActionKind,
        result: ProviderResult<()>,
    },
}

pub struct ProviderWorker {
    sender: Sender<ProviderCommand>,
    receiver: Receiver<ProviderEvent>,
    name: &'static str,
}

impl ProviderWorker {
    pub fn start(provider: Box<dyn ClipProvider>) -> Self {
        let name = provider.name();
        let (command_sender, command_receiver) = mpsc::channel();
        let (event_sender, event_receiver) = mpsc::channel();
        thread::Builder::new()
            .name(format!("ditto-popup-provider-{name}"))
            .spawn(move || provider_loop(provider, command_receiver, event_sender))
            .expect("provider worker thread");
        Self {
            sender: command_sender,
            receiver: event_receiver,
            name,
        }
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }

    pub fn send(&self, command: ProviderCommand) {
        let _ = self.sender.send(command);
    }

    pub fn try_recv(&self) -> Option<ProviderEvent> {
        self.receiver.try_recv().ok()
    }
}

fn provider_loop(
    mut provider: Box<dyn ClipProvider>,
    receiver: Receiver<ProviderCommand>,
    sender: Sender<ProviderEvent>,
) {
    while let Ok(command) = receiver.recv() {
        match command {
            ProviderCommand::Load {
                request_id,
                query,
                group,
                limit,
                cursor,
            } => {
                let result = provider.load(&query, group, limit, cursor.as_deref());
                if sender
                    .send(ProviderEvent::Loaded { request_id, result })
                    .is_err()
                {
                    break;
                }
            }
            ProviderCommand::RefreshMetadata => {
                let result = load_metadata(provider.as_mut());
                if sender.send(ProviderEvent::Metadata(result)).is_err() {
                    break;
                }
            }
            ProviderCommand::Action(action) => {
                let action_kind = action.kind();
                let result = run_action(provider.as_mut(), action);
                if sender
                    .send(ProviderEvent::ActionDone {
                        action: action_kind,
                        result,
                    })
                    .is_err()
                {
                    break;
                }
            }
        }
    }
}

fn load_metadata(provider: &mut dyn ClipProvider) -> ProviderResult<ProviderMetadata> {
    Ok(ProviderMetadata {
        groups: provider.groups()?,
        settings: provider.settings()?,
        copy_buffers: provider.copy_buffers()?,
    })
}

fn run_action(provider: &mut dyn ClipProvider, action: ProviderAction) -> ProviderResult<()> {
    match action {
        ProviderAction::Paste { clip_ids, mode } => provider.paste_many(&clip_ids, mode),
        ProviderAction::SetPinned { clip_id, pinned } => provider.set_pinned(clip_id, pinned),
        ProviderAction::MoveToGroup { clip_id, group_id } => {
            provider.move_to_group(clip_id, group_id)
        }
        ProviderAction::Delete { clip_ids } => provider.delete(&clip_ids),
        ProviderAction::AssignCopyBuffer { clip_id, slot } => {
            provider.assign_copy_buffer(clip_id, slot)
        }
        ProviderAction::ClearCopyBuffer { slot } => provider.clear_copy_buffer(slot),
        ProviderAction::PasteCopyBuffer { slot, plain_text } => {
            provider.paste_copy_buffer(slot, plain_text)
        }
    }
}

#[allow(dead_code)]
fn _assert_provider_error_is_send(error: ProviderError) -> impl Send {
    error
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockProvider;

    #[test]
    fn worker_delivers_mock_metadata_and_page() {
        let worker = ProviderWorker::start(Box::new(MockProvider::new(32)));
        worker.send(ProviderCommand::RefreshMetadata);
        worker.send(ProviderCommand::Load {
            request_id: 7,
            query: String::new(),
            group: GroupFilter::All,
            limit: 32,
            cursor: None,
        });
        let mut saw_metadata = false;
        let mut saw_page = false;
        for _ in 0..100 {
            while let Some(event) = worker.try_recv() {
                match event {
                    ProviderEvent::Metadata(Ok(metadata)) => {
                        assert_eq!(metadata.groups.len(), 50);
                        saw_metadata = true;
                    }
                    ProviderEvent::Loaded {
                        request_id: 7,
                        result: Ok(page),
                    } => {
                        assert_eq!(page.items.len(), 32);
                        saw_page = true;
                    }
                    _ => {}
                }
            }
            if saw_metadata && saw_page {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(saw_metadata);
        assert!(saw_page);
    }
}
