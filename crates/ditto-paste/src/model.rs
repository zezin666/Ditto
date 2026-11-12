#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchMode {
    Normal,
    PlainText,
    Multi,
}

impl BatchMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::PlainText => "plain",
            Self::Multi => "multi",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasteFormat {
    pub name: String,
    pub data: Vec<u8>,
    pub seq: u32,
    pub is_primary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasteOptions {
    pub plain_text: bool,
    pub formats: Vec<String>,
    pub focus_delay_ms: u64,
    pub multi_interval_ms: u64,
    pub restore_clipboard_on_failure: bool,
}

impl Default for PasteOptions {
    fn default() -> Self {
        Self {
            plain_text: false,
            formats: Vec::new(),
            focus_delay_ms: 0,
            multi_interval_ms: 180,
            restore_clipboard_on_failure: true,
        }
    }
}

impl PasteOptions {
    pub fn normalized(mut self) -> Self {
        self.focus_delay_ms = self.focus_delay_ms.min(300);
        self.multi_interval_ms = self.multi_interval_ms.clamp(180, 2_000);
        self.formats = self
            .formats
            .into_iter()
            .map(|format| format.trim().to_string())
            .filter(|format| !format.is_empty())
            .collect();
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrityLevel {
    Untrusted,
    Low,
    Medium,
    High,
    System,
    Unknown,
}

impl IntegrityLevel {
    pub const fn is_elevated_target(self) -> bool {
        matches!(self, Self::High | Self::System)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetWindow {
    pub hwnd: isize,
    pub focus_hwnd: isize,
    pub pid: u32,
    pub thread_id: u32,
    pub class_name: String,
    pub integrity: IntegrityLevel,
}
