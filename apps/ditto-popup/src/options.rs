use std::path::PathBuf;

use crate::{localization::Language, platform::PositionMode, theme::Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureState {
    Default,
    Empty,
    Search,
    Preview,
    Groups,
    Multi,
}

impl CaptureState {
    fn from_setting(value: &str) -> Self {
        match value {
            "empty" => Self::Empty,
            "search" => Self::Search,
            "preview" => Self::Preview,
            "groups" => Self::Groups,
            "multi" => Self::Multi,
            _ => Self::Default,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Empty => "empty",
            Self::Search => "search",
            Self::Preview => "preview",
            Self::Groups => "groups",
            Self::Multi => "multi",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CaptureOptions {
    pub path: PathBuf,
    pub state: CaptureState,
}

#[derive(Debug, Clone)]
pub struct PopupOptions {
    pub mock: bool,
    pub auto_show: bool,
    pub hide_on_focus_loss: bool,
    pub language: Language,
    pub theme_override: Option<Theme>,
    pub dpi_scale_override: Option<f32>,
    pub capture: Option<CaptureOptions>,
    pub soak_cycles: Option<u32>,
    pub soak_log: PathBuf,
    pub bench_scroll: bool,
    pub bench_frames: u32,
    pub bench_log: PathBuf,
    pub latency_only: bool,
    pub perf_log: PathBuf,
}

impl Default for PopupOptions {
    fn default() -> Self {
        Self {
            mock: false,
            auto_show: false,
            hide_on_focus_loss: true,
            language: Language::English,
            theme_override: None,
            dpi_scale_override: None,
            capture: None,
            soak_cycles: None,
            soak_log: PathBuf::from("artifacts/rewrite/after/perf/popup-show-hide-20.csv"),
            bench_scroll: false,
            bench_frames: 300,
            bench_log: PathBuf::from("artifacts/rewrite/after/perf/scroll-10k.csv"),
            latency_only: false,
            perf_log: PathBuf::from("artifacts/rewrite/after/perf/popup-latency.csv"),
        }
    }
}

impl PopupOptions {
    pub fn parse() -> Self {
        let language = std::env::var("DITTO_LANG")
            .map_or(Language::English, |value| Language::from_tag(&value));
        let mut options = Self {
            mock: env_flag("DITTO_POPUP_MOCK"),
            auto_show: env_flag("DITTO_POPUP_AUTO_SHOW"),
            hide_on_focus_loss: !std::env::var("DITTO_POPUP_HIDE_ON_FOCUS_LOSS")
                .is_ok_and(|value| value == "0"),
            language,
            ..Default::default()
        };

        let mut capture_path = None;
        let mut capture_state = CaptureState::Default;
        for argument in std::env::args().skip(1) {
            match argument.as_str() {
                "--mock" => options.mock = true,
                "--auto-show" => options.auto_show = true,
                "--no-hide-on-focus-loss" => options.hide_on_focus_loss = false,
                "--bench-scroll" => {
                    options.mock = true;
                    options.auto_show = true;
                    options.bench_scroll = true;
                }
                "--latency-only" => {
                    options.mock = true;
                    options.auto_show = true;
                    options.latency_only = true;
                }
                _ => {
                    if let Some(value) = argument.strip_prefix("--lang=") {
                        options.language = Language::from_tag(value);
                    } else if let Some(value) = argument.strip_prefix("--theme=") {
                        options.theme_override = match value {
                            "light" => Some(Theme::Light),
                            "dark" => Some(Theme::Dark),
                            "hc" | "high-contrast" => Some(Theme::HighContrast),
                            _ => None,
                        };
                    } else if let Some(value) = argument.strip_prefix("--dpi-scale=") {
                        options.dpi_scale_override = value.parse::<f32>().ok();
                    } else if let Some(value) = argument.strip_prefix("--capture=") {
                        capture_path = Some(PathBuf::from(value));
                        options.mock = true;
                        options.auto_show = true;
                    } else if let Some(value) = argument.strip_prefix("--capture-state=") {
                        capture_state = CaptureState::from_setting(value);
                    } else if let Some(value) = argument.strip_prefix("--soak=") {
                        options.soak_cycles = value.parse().ok();
                        options.mock = true;
                        options.auto_show = true;
                    } else if let Some(value) = argument.strip_prefix("--soak-log=") {
                        options.soak_log = PathBuf::from(value);
                    } else if let Some(value) = argument.strip_prefix("--bench-frames=") {
                        options.bench_frames = value.parse().unwrap_or(options.bench_frames);
                    } else if let Some(value) = argument.strip_prefix("--bench-log=") {
                        options.bench_log = PathBuf::from(value);
                    } else if let Some(value) = argument.strip_prefix("--perf-log=") {
                        options.perf_log = PathBuf::from(value);
                    }
                }
            }
        }
        if let Some(path) = capture_path {
            options.capture = Some(CaptureOptions {
                path,
                state: capture_state,
            });
        }
        if let Ok(path) = std::env::var("DITTO_PERF_LOG") {
            options.perf_log = PathBuf::from(path);
        }
        options
    }

    pub fn default_position(&self) -> PositionMode {
        PositionMode::Caret
    }
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| {
        matches!(
            value.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_state_names_are_stable() {
        assert_eq!(CaptureState::Default.name(), "default");
        assert_eq!(CaptureState::Multi.name(), "multi");
    }
}
