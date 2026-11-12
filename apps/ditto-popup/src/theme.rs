use ditto_model::ThemeMode;
use ditto_ui_tokens::{Mode, color, parse_hex};
use egui::Color32;
use windows::Win32::{
    Graphics::Gdi::{
        COLOR_BTNFACE, COLOR_GRAYTEXT, COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT, COLOR_WINDOW,
        COLOR_WINDOWTEXT, GetSysColor,
    },
    UI::{
        Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
        WindowsAndMessaging::{
            SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
    HighContrast,
}

impl Theme {
    pub const fn mode(self) -> Mode {
        match self {
            Self::Light => Mode::Light,
            Self::Dark => Mode::Dark,
            Self::HighContrast => Mode::HighContrast,
        }
    }

    pub const fn dark(self) -> bool {
        matches!(self, Self::Dark)
    }

    pub const fn high_contrast(self) -> bool {
        matches!(self, Self::HighContrast)
    }
}

impl From<ThemeMode> for Theme {
    fn from(value: ThemeMode) -> Self {
        match value {
            ThemeMode::Light => Self::Light,
            ThemeMode::Dark => Self::Dark,
            ThemeMode::HighContrast => Self::HighContrast,
            ThemeMode::System => {
                if high_contrast_enabled() {
                    Self::HighContrast
                } else if ditto_win::system_theme() == ThemeMode::Dark {
                    Self::Dark
                } else {
                    Self::Light
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub high_contrast: bool,
    pub canvas: Color32,
    pub surface: Color32,
    pub surface_alt: Color32,
    pub surface_sunken: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_disabled: Color32,
    pub accent: Color32,
    pub accent_fill: Color32,
    pub on_accent: Color32,
    pub hover: Color32,
    pub selection: Color32,
    pub selection_text: Color32,
    pub focus: Color32,
    pub danger: Color32,
    pub pinned: Color32,
    pub pasted: Color32,
    pub scroll_thumb: Color32,
    pub scroll_thumb_hover: Color32,
}

impl Palette {
    pub fn for_theme(theme: Theme) -> Self {
        let mode = theme.mode();
        if theme.high_contrast() {
            let window = system_color(COLOR_WINDOW);
            let text = system_color(COLOR_WINDOWTEXT);
            let highlight = system_color(COLOR_HIGHLIGHT);
            let highlight_text = system_color(COLOR_HIGHLIGHTTEXT);
            return Self {
                high_contrast: true,
                canvas: window,
                surface: window,
                surface_alt: system_color(COLOR_BTNFACE),
                surface_sunken: window,
                border: text,
                border_strong: text,
                text_primary: text,
                text_secondary: text,
                text_disabled: system_color(COLOR_GRAYTEXT),
                accent: highlight,
                accent_fill: highlight,
                on_accent: highlight_text,
                hover: highlight,
                selection: highlight,
                selection_text: highlight_text,
                focus: highlight,
                danger: text,
                pinned: text,
                pasted: text,
                scroll_thumb: text,
                scroll_thumb_hover: text,
            };
        }

        Self {
            high_contrast: false,
            canvas: token_color(color::canvas(mode)),
            surface: token_color(color::surface(mode)),
            surface_alt: token_color(color::surface_alt(mode)),
            surface_sunken: token_color(color::surface_sunken(mode)),
            border: token_color(color::border(mode)),
            border_strong: token_color(color::border_strong(mode)),
            text_primary: token_color(color::text_primary(mode)),
            text_secondary: token_color(color::text_secondary(mode)),
            text_disabled: token_color(color::text_disabled(mode)),
            accent: token_color(color::accent(mode)),
            accent_fill: token_color(color::accent_fill(mode)),
            on_accent: token_color(color::on_accent(mode)),
            hover: token_color(color::hover(mode)),
            selection: token_color(color::selection(mode)),
            selection_text: token_color(color::selection_text(mode)),
            focus: token_color(color::focus(mode)),
            danger: token_color(color::danger(mode)),
            pinned: token_color(color::pinned(mode)),
            pasted: token_color(color::pasted(mode)),
            scroll_thumb: token_color(color::scroll_thumb(mode)),
            scroll_thumb_hover: token_color(color::scroll_thumb_hover(mode)),
        }
    }
}

pub fn high_contrast_enabled() -> bool {
    let mut high_contrast = HIGHCONTRASTW::default();
    let result = unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            std::mem::size_of::<HIGHCONTRASTW>() as u32,
            Some((&mut high_contrast as *mut HIGHCONTRASTW).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS::default(),
        )
    };
    result.is_ok() && high_contrast.dwFlags.contains(HCF_HIGHCONTRASTON)
}

fn token_color(value: Option<&str>) -> Color32 {
    value.map_or_else(|| system_color(COLOR_WINDOWTEXT), hex_color)
}

fn hex_color(value: &str) -> Color32 {
    let [red, green, blue, alpha] = parse_hex(value);
    Color32::from_rgba_unmultiplied(red, green, blue, alpha)
}

fn system_color(index: windows::Win32::Graphics::Gdi::SYS_COLOR_INDEX) -> Color32 {
    let value = unsafe { GetSysColor(index) };
    Color32::from_rgb(
        (value & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        ((value >> 16) & 0xFF) as u8,
    )
}
