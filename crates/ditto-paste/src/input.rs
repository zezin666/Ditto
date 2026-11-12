use std::mem::size_of;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, SendInput, VIRTUAL_KEY,
    VK_CONTROL, VK_V,
};

use crate::{InputBackend, PasteError, PasteResult};

#[derive(Debug, Default, Clone, Copy)]
pub struct SendInputBackend;

impl InputBackend for SendInputBackend {
    fn send_ctrl_v(&self) -> PasteResult<()> {
        send_ctrl_v()
    }
}

pub fn send_ctrl_v() -> PasteResult<()> {
    let inputs = [
        keyboard_input(VK_CONTROL, false),
        keyboard_input(VK_V, false),
        keyboard_input(VK_V, true),
        keyboard_input(VK_CONTROL, true),
    ];
    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    if sent == inputs.len() as u32 {
        Ok(())
    } else {
        Err(PasteError::Internal(format!(
            "SendInput accepted {sent} of {} events",
            inputs.len()
        )))
    }
}

fn keyboard_input(key: VIRTUAL_KEY, key_up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: if key_up {
                    KEYBD_EVENT_FLAGS(2)
                } else {
                    KEYBD_EVENT_FLAGS::default()
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
