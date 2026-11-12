use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

const EN_US: &str = include_str!("../locales/en-US.json");
const ZH_CN: &str = include_str!("../locales/zh-CN.json");
const SHARED_EN_US: &str = include_str!("../../ditto/ui/locales/en-US.json");
const SHARED_ZH_CN: &str = include_str!("../../ditto/ui/locales/zh-CN.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    Chinese,
}

impl Language {
    pub fn from_tag(value: &str) -> Self {
        let normalized = value.to_ascii_lowercase();
        if normalized.starts_with("zh") {
            Self::Chinese
        } else {
            Self::English
        }
    }

    pub const fn tag(self) -> &'static str {
        match self {
            Self::English => "en-US",
            Self::Chinese => "zh-CN",
        }
    }
}

pub mod key {
    pub const APP_NAME: &str = "app.name";
    pub const SEARCH_PLACEHOLDER: &str = "popup.search.placeholder";
    pub const SEARCH_RESULT_COUNT: &str = "popup.search.resultCount";
    pub const SEARCHING: &str = "popup.search.searching";
    pub const SEARCH_CLEAR: &str = "popup.search.clear";
    pub const SHORTCUT_HINT: &str = "popup.shortcutHint";
    pub const SHORTCUT_HINT_COMPACT: &str = "popup.shortcutHintCompact";
    pub const GROUP_ALL: &str = "popup.group.all";
    pub const GROUP_UNGROUPED: &str = "popup.group.ungrouped";
    pub const GROUP_CHOOSE: &str = "popup.group.choose";
    pub const GROUP_BACK: &str = "popup.group.back";
    pub const GROUP_CURRENT: &str = "popup.group.current";
    pub const PREVIEW_TOGGLE: &str = "popup.preview.toggle";
    pub const PREVIEW_TITLE: &str = "popup.preview.title";
    pub const PREVIEW_COLOR: &str = "popup.preview.color";
    pub const PREVIEW_FILES: &str = "popup.preview.files";
    pub const PREVIEW_IMAGE: &str = "popup.preview.image";
    pub const PREVIEW_RICH: &str = "popup.preview.rich";
    pub const PREVIEW_CLOSE: &str = "popup.preview.close";
    pub const PREVIEW_OPEN: &str = "popup.preview.open";
    pub const PREVIEW_EMPTY: &str = "popup.preview.empty";
    pub const TITLE_ALWAYS_ON_TOP: &str = "popup.title.alwaysOnTop";
    pub const TITLE_CLOSE: &str = "popup.title.close";
    pub const STATE_LOADING: &str = "popup.state.loading";
    pub const STATE_EMPTY_TITLE: &str = "popup.state.empty.title";
    pub const STATE_EMPTY_BODY: &str = "popup.state.empty.body";
    pub const STATE_EMPTY_SEARCH: &str = "popup.state.empty.search";
    pub const STATE_ERROR_TITLE: &str = "popup.state.error.title";
    pub const STATE_ERROR_BODY: &str = "popup.state.error.body";
    pub const STATE_RETRY: &str = "popup.state.retry";
    pub const STATE_DISCONNECTED: &str = "popup.state.disconnected";
    pub const STATUS_PASTE_SENT: &str = "popup.status.pasteSent";
    pub const STATUS_PLAIN_PASTE_SENT: &str = "popup.status.plainPasteSent";
    pub const STATUS_PINNED: &str = "popup.status.pinned";
    pub const STATUS_MOVED: &str = "popup.status.moved";
    pub const STATUS_DELETED: &str = "popup.status.deleted";
    pub const STATUS_BUFFER_ASSIGNED: &str = "popup.status.bufferAssigned";
    pub const STATUS_BUFFER_CLEARED: &str = "popup.status.bufferCleared";
    pub const STATUS_BUFFER_PASTED: &str = "popup.status.bufferPasted";
    pub const STATUS_UNSUPPORTED: &str = "popup.status.unsupported";
    pub const STATUS_FAILED: &str = "popup.status.failed";
    pub const STATUS_SEARCH_FAILED: &str = "popup.status.searchFailed";
    pub const STATUS_RECONNECTED: &str = "popup.status.reconnected";
    pub const STATUS_TOOK: &str = "popup.status.took";
    pub const MENU_PASTE: &str = "popup.menu.paste";
    pub const MENU_PASTE_PLAIN: &str = "popup.menu.pastePlain";
    pub const MENU_PASTE_HTML: &str = "popup.menu.pasteHtml";
    pub const MENU_PASTE_RTF: &str = "popup.menu.pasteRtf";
    pub const MENU_PASTE_FILES: &str = "popup.menu.pasteFiles";
    pub const MENU_PIN: &str = "popup.menu.pin";
    pub const MENU_UNPIN: &str = "popup.menu.unpin";
    pub const MENU_MOVE: &str = "popup.menu.move";
    pub const MENU_DELETE: &str = "popup.menu.delete";
    pub const MENU_COPY_TO_BUFFER: &str = "popup.menu.copyToBuffer";
    pub const MENU_PASTE_BUFFER: &str = "popup.menu.pasteBuffer";
    pub const MENU_PREVIEW: &str = "popup.menu.preview";
    pub const MENU_BUFFER_EMPTY: &str = "popup.menu.bufferEmpty";
    pub const MENU_BUFFER_NAME: &str = "popup.menu.bufferName";
    pub const DELETE_TITLE: &str = "popup.delete.title";
    pub const DELETE_BODY: &str = "popup.delete.body";
    pub const DELETE_CONFIRM: &str = "popup.delete.confirm";
    pub const DELETE_CANCEL: &str = "popup.delete.cancel";
    pub const FORMAT_TEXT: &str = "popup.format.text";
    pub const FORMAT_HTML: &str = "popup.format.html";
    pub const FORMAT_RTF: &str = "popup.format.rtf";
    pub const FORMAT_IMAGE: &str = "popup.format.image";
    pub const FORMAT_FILES: &str = "popup.format.files";
    pub const FORMAT_CUSTOM: &str = "popup.format.custom";
    pub const BADGE_PINNED: &str = "popup.badge.pinned";
    pub const BADGE_PASTED: &str = "popup.badge.pasted";
    pub const BADGE_FIRST_TEN: &str = "popup.badge.firstTen";
    pub const BADGE_COPY_BUFFER: &str = "popup.badge.copyBuffer";
    pub const SOURCE_UNKNOWN: &str = "popup.source.unknown";
    pub const TIME_NOW: &str = "popup.time.now";
    pub const TIME_MINUTES: &str = "popup.time.minutes";
    pub const TIME_HOURS: &str = "popup.time.hours";
    pub const TIME_DAYS: &str = "popup.time.days";
}

pub struct Localizer {
    language: Language,
    strings: BTreeMap<String, String>,
}

impl Localizer {
    pub fn load(language: Language) -> Self {
        // The shared settings catalog is the canonical bundle; the popup
        // bundle only carries fallback copy for a standalone popup build.
        let mut strings = parse_bundle(match language {
            Language::English => SHARED_EN_US,
            Language::Chinese => SHARED_ZH_CN,
        });
        overlay_bundle(
            &mut strings,
            match language {
                Language::English => EN_US,
                Language::Chinese => ZH_CN,
            },
        );
        for path in shared_locale_candidates(language) {
            if let Ok(bundle) = fs::read_to_string(path) {
                overlay_bundle(&mut strings, &bundle);
            }
        }
        Self { language, strings }
    }

    pub const fn language(&self) -> Language {
        self.language
    }

    pub fn text<'a>(&'a self, key: &'a str) -> &'a str {
        self.strings.get(key).map_or(key, String::as_str)
    }

    pub fn format(&self, key: &str, values: &[(&str, &str)]) -> String {
        let mut output = self.text(key).to_string();
        for (name, value) in values {
            output = output.replace(&format!("{{{name}}}"), value);
        }
        output
    }
}

fn parse_bundle(bundle: &str) -> BTreeMap<String, String> {
    serde_json::from_str(bundle).unwrap_or_default()
}

fn overlay_bundle(strings: &mut BTreeMap<String, String>, bundle: &str) {
    strings.extend(parse_bundle(bundle));
}

fn shared_locale_candidates(language: Language) -> Vec<PathBuf> {
    let file = match language {
        Language::English => "en-US.json",
        Language::Chinese => "zh-CN.json",
    };
    let mut candidates = Vec::new();
    if let Some(directory) = std::env::var_os("DITTO_LOCALE_DIR") {
        candidates.push(PathBuf::from(directory).join(file));
    }
    if let Ok(current) = std::env::current_dir() {
        push_locale_candidate(&mut candidates, &current, file);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(directory) = exe.parent()
    {
        push_locale_candidate(&mut candidates, directory, file);
    }
    candidates
}

fn push_locale_candidate(candidates: &mut Vec<PathBuf>, root: &Path, file: &str) {
    for relative in [
        Path::new("apps/ditto/ui/locales").join(file),
        Path::new("../ui/locales").join(file),
        Path::new("locales").join(file),
    ] {
        let candidate = root.join(relative);
        if candidate.is_file() && !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_locale_bundles_define_every_t3_key() {
        let required = [
            key::APP_NAME,
            key::SEARCH_PLACEHOLDER,
            key::SEARCH_RESULT_COUNT,
            key::GROUP_ALL,
            key::PREVIEW_TITLE,
            key::STATE_EMPTY_TITLE,
            key::STATUS_UNSUPPORTED,
            key::MENU_DELETE,
            key::DELETE_TITLE,
            key::FORMAT_HTML,
            key::BADGE_COPY_BUFFER,
            key::TIME_MINUTES,
        ];
        for bundle in [EN_US, ZH_CN] {
            let parsed = parse_bundle(bundle);
            for key in required {
                assert!(
                    parsed.get(key).is_some_and(|value| !value.is_empty()),
                    "missing {key}"
                );
            }
        }
    }

    #[test]
    fn placeholders_are_substituted() {
        let locale = Localizer::load(Language::English);
        let formatted = locale.format(key::SEARCH_RESULT_COUNT, &[("count", "42")]);
        assert!(formatted.contains("42"));
    }
}
