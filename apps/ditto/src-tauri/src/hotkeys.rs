//! Global hotkey parsing and atomic registration swaps.
//!
//! Settings store user-facing chords such as `Ctrl+\`` or `Ctrl+Alt+D`. The
//! `global-hotkey` parser used by Tauri accepts exactly that syntax, but a
//! chord may be syntactically valid and still be owned by another application.
//! Registration is swapped atomically: the previous chord is restored when the
//! new one cannot be registered.

use tauri_plugin_global_shortcut::Shortcut;

pub const DEFAULT_OPEN_CHORD: &str = "Ctrl+`";
pub const FALLBACK_OPEN_CHORD: &str = "Ctrl+Alt+D";

pub fn parse_chord(chord: &str) -> Result<Shortcut, String> {
    let chord = chord.trim();
    if chord.is_empty() {
        return Err("hotkey chord is empty".to_string());
    }
    chord
        .parse::<Shortcut>()
        .map_err(|error| format!("invalid hotkey {chord:?}: {error}"))
}

/// Probes whether Win32 will let this process own the chord.
///
/// Only the simple chords offered by the UI are probed; unknown syntax falls
/// back to letting the global-shortcut plugin report the conflict.
pub fn chord_available(chord: &str) -> bool {
    let Some((modifiers, virtual_key)) = win32_chord(chord) else {
        return true;
    };
    ditto_win::hotkey_available(modifiers, virtual_key)
}

fn win32_chord(chord: &str) -> Option<(u32, u32)> {
    const MOD_ALT: u32 = 0x0001;
    const MOD_CONTROL: u32 = 0x0002;
    const MOD_SHIFT: u32 = 0x0004;
    const MOD_WIN: u32 = 0x0008;
    let mut modifiers = 0_u32;
    let mut key = None;
    for token in chord.split('+') {
        match token.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= MOD_CONTROL,
            "alt" => modifiers |= MOD_ALT,
            "shift" => modifiers |= MOD_SHIFT,
            "win" | "super" | "meta" => modifiers |= MOD_WIN,
            other => {
                let virtual_key = match other {
                    "" => return None,
                    "`" | "backquote" => 0xC0,
                    "space" => 0x20,
                    "tab" => 0x09,
                    "enter" | "return" => 0x0D,
                    "esc" | "escape" => 0x1B,
                    "left" => 0x25,
                    "up" => 0x26,
                    "right" => 0x27,
                    "down" => 0x28,
                    value if value.len() == 1 => {
                        let byte = value.as_bytes()[0];
                        if byte.is_ascii_alphanumeric() {
                            u32::from(byte.to_ascii_uppercase())
                        } else {
                            return None;
                        }
                    }
                    value if value.starts_with('f') => {
                        let number = value[1..].parse::<u32>().ok()?;
                        if (1..=24).contains(&number) {
                            0x70 + number - 1
                        } else {
                            return None;
                        }
                    }
                    _ => return None,
                };
                key = Some(virtual_key);
            }
        }
    }
    Some((modifiers, key?))
}

pub trait ShortcutRegistrar {
    fn register(&mut self, shortcut: &Shortcut) -> Result<(), String>;
    fn unregister(&mut self, shortcut: &Shortcut) -> Result<(), String>;
}

/// Replaces `previous` with `next`. If `next` cannot be registered, `previous`
/// is restored and the conflict is returned to the caller.
pub fn swap(
    registrar: &mut dyn ShortcutRegistrar,
    previous: Option<&Shortcut>,
    next: Option<&Shortcut>,
) -> Result<(), String> {
    if let Some(previous) = previous {
        registrar.unregister(previous)?;
    }
    if let Some(next) = next
        && let Err(error) = registrar.register(next)
    {
        if let Some(previous) = previous {
            let _ = registrar.register(previous);
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[derive(Default)]
    struct MockRegistrar {
        registered: BTreeSet<String>,
        fail_on: Option<String>,
        calls: Vec<String>,
    }

    impl ShortcutRegistrar for MockRegistrar {
        fn register(&mut self, shortcut: &Shortcut) -> Result<(), String> {
            let chord = shortcut.to_string();
            self.calls.push(format!("register {chord}"));
            if self.fail_on.as_deref() == Some(chord.as_str()) {
                return Err("already in use".to_string());
            }
            self.registered.insert(chord);
            Ok(())
        }

        fn unregister(&mut self, shortcut: &Shortcut) -> Result<(), String> {
            let chord = shortcut.to_string();
            self.calls.push(format!("unregister {chord}"));
            self.registered.remove(&chord);
            Ok(())
        }
    }

    #[test]
    fn default_and_ui_chords_parse() {
        assert!(parse_chord("Ctrl+`").is_ok());
        assert!(parse_chord("Ctrl+Alt+D").is_ok());
        assert!(parse_chord("Ctrl+Shift+V").is_ok());
        assert!(parse_chord("").is_err());
    }

    #[test]
    fn swap_replaces_the_old_chord() {
        let previous = parse_chord("Ctrl+`").expect("previous");
        let next = parse_chord("Ctrl+Alt+D").expect("next");
        let mut registrar = MockRegistrar::default();
        swap(&mut registrar, Some(&previous), Some(&next)).expect("swap");
        assert!(registrar.registered.contains(&next.to_string()));
        assert!(!registrar.registered.contains(&previous.to_string()));
    }

    #[test]
    fn failed_registration_restores_the_previous_chord() {
        let previous = parse_chord("Ctrl+`").expect("previous");
        let next = parse_chord("Ctrl+Alt+D").expect("next");
        let mut registrar = MockRegistrar {
            fail_on: Some(next.to_string()),
            ..MockRegistrar::default()
        };
        let error = swap(&mut registrar, Some(&previous), Some(&next)).expect_err("conflict");
        assert!(error.contains("already in use"));
        assert!(
            registrar.registered.contains(&previous.to_string()),
            "the previous chord must be restored"
        );
    }

    #[test]
    fn generic_win32_probe_understands_supported_chords() {
        assert_eq!(win32_chord("Ctrl+`"), Some((0x0002, 0xC0)));
        assert_eq!(win32_chord("Ctrl+Alt+D"), Some((0x0003, 0x44)));
        assert_eq!(win32_chord("Shift+F5"), Some((0x0004, 0x74)));
        assert_eq!(win32_chord("Ctrl+Nope"), None);
    }
}
