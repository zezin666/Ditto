//! Paste target tracking and SendInput boundary. Implementation belongs to T4.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteMode {
    Normal,
    PlainText,
    Multi,
}

pub trait PasteTarget {
    fn is_target_available(&self) -> bool;
}
