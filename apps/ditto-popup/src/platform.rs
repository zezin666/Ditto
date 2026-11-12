use std::ffi::c_void;

use ditto_ui_tokens::{Mode, dimension};
use windows::{
    Win32::{
        Foundation::{HWND, POINT, RECT},
        Graphics::{
            Dwm::{
                DWM_WINDOW_CORNER_PREFERENCE, DWMWA_BORDER_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
                DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
            },
            Gdi::{
                ClientToScreen, GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO,
                MonitorFromPoint,
            },
        },
        System::{
            ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
            Threading::{
                AttachThreadInput, GR_GDIOBJECTS, GR_USEROBJECTS, GetCurrentProcess,
                GetCurrentThreadId, GetGuiResources,
            },
        },
        UI::{
            HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI},
            Input::KeyboardAndMouse::{GetAsyncKeyState, SetFocus, VK_LBUTTON},
            WindowsAndMessaging::{
                GUITHREADINFO, GWL_EXSTYLE, GetCursorPos, GetForegroundWindow, GetGUIThreadInfo,
                GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId, HWND_TOPMOST, SW_SHOW,
                SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
                SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow, WS_EX_NOACTIVATE,
            },
        },
    },
    core::BOOL,
};

use crate::theme::{Palette, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionMode {
    Caret,
    Cursor,
    Last,
}

impl PositionMode {
    pub fn from_setting(value: &str) -> Self {
        match value {
            "cursor" => Self::Cursor,
            "last" => Self::Last,
            _ => Self::Caret,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub dpi: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct ResourceSample {
    pub gdi_objects: u32,
    pub user_objects: u32,
    pub private_bytes: usize,
    pub working_set_bytes: usize,
}

pub fn configure_window(hwnd: HWND, theme: Theme, palette: Palette) {
    let rounded = DWM_WINDOW_CORNER_PREFERENCE(DWMWCP_ROUND.0);
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&raw const rounded).cast::<c_void>(),
            std::mem::size_of_val(&rounded) as u32,
        );
        let dark = i32::from(theme.dark());
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&raw const dark).cast::<c_void>(),
            std::mem::size_of_val(&dark) as u32,
        );
        if !theme.high_contrast() {
            let border = colorref(palette.border);
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                (&raw const border).cast::<c_void>(),
                std::mem::size_of_val(&border) as u32,
            );
        }
    }
}

pub fn position_and_size(
    hwnd: HWND,
    logical_width: f32,
    logical_height: f32,
    position_mode: PositionMode,
    last_position: Option<POINT>,
) -> Option<Placement> {
    if hwnd.0.is_null() {
        return None;
    }
    let anchor = match position_mode {
        PositionMode::Caret => caret_position().or_else(cursor_position)?,
        PositionMode::Cursor | PositionMode::Last => cursor_position()?,
    };
    if position_mode == PositionMode::Last
        && let Some(last) = last_position
    {
        return clamp_to_monitor(hwnd, last, logical_width, logical_height, anchor);
    }
    clamp_to_monitor(hwnd, anchor, logical_width, logical_height, anchor)
}

pub fn center_on_primary(
    hwnd: HWND,
    physical_width: i32,
    physical_height: i32,
) -> Option<Placement> {
    let mut cursor = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut cursor);
    }
    let monitor = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) };
    let work = monitor_work_area(monitor)?;
    let x = work.left + (work.right - work.left - physical_width).max(0) / 2;
    let y = work.top + (work.bottom - work.top - physical_height).max(0) / 2;
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            x,
            y,
            physical_width,
            physical_height,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )
        .ok()?;
    }
    Some(Placement {
        x,
        y,
        width: physical_width,
        height: physical_height,
        dpi: dpi_for_monitor(monitor).max(96),
    })
}

pub fn dpi_for_window(hwnd: HWND) -> u32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 { 96 } else { dpi }
}

/// Gives the popup real OS keyboard focus for the keyboard-first hotkey path.
///
/// `WS_EX_NOACTIVATE` is cleared, then the current thread is temporarily
/// attached to the foreground thread so `SetForegroundWindow` is allowed even
/// though the process was started in the background. This intentionally steals
/// focus from the target app; the core has already snapshotted that target for
/// paste, and hide restores it.
pub fn activate_popup(hwnd: HWND, _previous_foreground: HWND) -> bool {
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style & !(WS_EX_NOACTIVATE.0 as isize));
        // Show/raise without SWP_NOACTIVATE so the activation request is not
        // suppressed by the layer style.
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
        let foreground = GetForegroundWindow();
        let foreground_thread = GetWindowThreadProcessId(foreground, None);
        let current_thread = GetCurrentThreadId();
        let attached = !foreground.0.is_null()
            && foreground_thread != 0
            && foreground_thread != current_thread
            && AttachThreadInput(current_thread, foreground_thread, true).as_bool();
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(hwnd));
        if attached {
            let _ = AttachThreadInput(current_thread, foreground_thread, false);
        }
        GetForegroundWindow() == hwnd
    }
}

pub fn activate_for_user(hwnd: HWND, previous_foreground: HWND) -> bool {
    activate_popup(hwnd, previous_foreground)
}

pub fn restore_no_activate(hwnd: HWND) {
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_NOACTIVATE.0 as isize);
    }
}

pub fn restore_foreground(previous_foreground: HWND) {
    if !previous_foreground.0.is_null() {
        unsafe {
            let _ = SetForegroundWindow(previous_foreground);
        }
    }
}

pub fn current_foreground() -> HWND {
    unsafe { GetForegroundWindow() }
}

pub fn is_window_visible(hwnd: HWND) -> bool {
    unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool() }
}

pub fn primary_click_outside(hwnd: HWND) -> bool {
    let state = unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) };
    // Low bit = pressed since the last poll; high bit = currently down. Polling
    // at 50 ms plus the down-state check catches ordinary and injected clicks.
    let pressed = (state & 1) != 0 || (state as u16 & 0x8000) != 0;
    if !pressed {
        return false;
    }
    let mut cursor = POINT::default();
    let mut rect = RECT::default();
    let cursor_ok = unsafe { GetCursorPos(&mut cursor) }.is_ok();
    let rect_ok = unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok();
    if !cursor_ok || !rect_ok {
        return false;
    }
    !(cursor.x >= rect.left
        && cursor.x < rect.right
        && cursor.y >= rect.top
        && cursor.y < rect.bottom)
}

/// Hides on an outside click or when the user switches to another
/// application. The popup itself is shown without activation, so the
/// foreground remains the original target until the user clicks elsewhere.
pub fn outside_click_or_foreground_change(hwnd: HWND, previous_foreground: HWND) -> bool {
    let foreground = current_foreground();
    if !foreground.0.is_null() && foreground != hwnd && foreground != previous_foreground {
        return true;
    }
    primary_click_outside(hwnd)
}

pub fn sample_process() -> Option<ResourceSample> {
    let process = unsafe { GetCurrentProcess() };
    let mut counters = PROCESS_MEMORY_COUNTERS {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..Default::default()
    };
    unsafe {
        GetProcessMemoryInfo(process, &mut counters, counters.cb).ok()?;
    }
    let gdi_objects = unsafe { GetGuiResources(process, GR_GDIOBJECTS) };
    let user_objects = unsafe { GetGuiResources(process, GR_USEROBJECTS) };
    Some(ResourceSample {
        gdi_objects,
        user_objects,
        private_bytes: counters.PagefileUsage,
        working_set_bytes: counters.WorkingSetSize,
    })
}

fn clamp_to_monitor(
    hwnd: HWND,
    point: POINT,
    logical_width: f32,
    logical_height: f32,
    fallback_anchor: POINT,
) -> Option<Placement> {
    let monitor = unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST) };
    let work = monitor_work_area(monitor)?;
    let dpi = dpi_for_monitor(monitor).max(96);
    let scale = dpi as f64 / 96.0;
    let width = (f64::from(logical_width) * scale).round() as i32;
    let height = (f64::from(logical_height) * scale).round() as i32;
    let margin = (f64::from(dimension::space_sm(Mode::Light)) * scale).round() as i32;
    let min_x = work.left + margin;
    let min_y = work.top + margin;
    let max_x = (work.right - margin - width).max(min_x);
    let max_y = (work.bottom - margin - height).max(min_y);

    let mut anchor = point;
    if anchor.x < work.left
        || anchor.x > work.right
        || anchor.y < work.top
        || anchor.y > work.bottom
    {
        anchor = fallback_anchor;
    }

    let x = (anchor.x + margin).clamp(min_x, max_x);
    let mut y = (anchor.y + margin).clamp(min_y, max_y);
    if y + height > work.bottom - margin {
        y = (anchor.y - height - margin).clamp(min_y, max_y);
    }

    unsafe {
        SetWindowPos(
            hwnd,
            None,
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )
        .ok()?;
    }
    Some(Placement {
        x,
        y,
        width,
        height,
        dpi,
    })
}

fn monitor_work_area(monitor: HMONITOR) -> Option<RECT> {
    if monitor.0.is_null() {
        return None;
    }
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let valid: BOOL = unsafe { GetMonitorInfoW(monitor, &mut info) };
    valid.as_bool().then_some(info.rcWork)
}

fn dpi_for_monitor(monitor: HMONITOR) -> u32 {
    let mut dpi_x = 96_u32;
    let mut dpi_y = 96_u32;
    let _ = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    dpi_x.max(96)
}

fn cursor_position() -> Option<POINT> {
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point) }.ok().map(|()| point)
}

fn caret_position() -> Option<POINT> {
    let foreground = unsafe { GetForegroundWindow() };
    if foreground.0.is_null() {
        return None;
    }
    let thread_id = unsafe { GetWindowThreadProcessId(foreground, None) };
    if thread_id == 0 {
        return None;
    }
    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        GetGUIThreadInfo(thread_id, &mut info).ok()?;
    }
    if info.hwndCaret.0.is_null() {
        return None;
    }
    let mut point = POINT {
        x: info.rcCaret.left,
        y: info.rcCaret.bottom,
    };
    let converted = unsafe { ClientToScreen(info.hwndCaret, &mut point) };
    if !converted.as_bool() {
        return None;
    }
    Some(point)
}

fn colorref(color: egui::Color32) -> u32 {
    u32::from(color.r()) | (u32::from(color.g()) << 8) | (u32::from(color.b()) << 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_mode_accepts_known_settings() {
        assert_eq!(PositionMode::from_setting("cursor"), PositionMode::Cursor);
        assert_eq!(PositionMode::from_setting("last"), PositionMode::Last);
        assert_eq!(PositionMode::from_setting("caret"), PositionMode::Caret);
        assert_eq!(
            PositionMode::from_setting("unexpected"),
            PositionMode::Caret
        );
    }

    #[test]
    fn colorref_uses_windows_channel_order() {
        assert_eq!(
            colorref(egui::Color32::from_rgb(0x12, 0x34, 0x56)),
            0x563412
        );
    }
}
