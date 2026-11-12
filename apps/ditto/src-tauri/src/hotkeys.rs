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

/// A configurable global shortcut action. First-ten chords stay popup-scoped
/// (they are only meaningful while the list is focused) and are not part of
/// this global set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    ToggleOpen,
    PlainPaste,
    CopyBufferCopy(u8),
    CopyBufferPaste(u8),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    pub chord: String,
    pub shortcut: Shortcut,
    pub action: HotkeyAction,
}

impl Binding {
    fn parse(chord: &str, action: HotkeyAction) -> Result<Option<Self>, String> {
        let chord = chord.trim();
        if chord.is_empty() {
            return Ok(None);
        }
        let shortcut = parse_chord(chord)?;
        Ok(Some(Self {
            chord: chord.to_string(),
            shortcut,
            action,
        }))
    }
}

pub fn binding(chord: &str, action: HotkeyAction) -> Result<Option<Binding>, String> {
    Binding::parse(chord, action)
}

/// Reads every global chord from the persisted settings JSON.
///
/// The legacy defaults for copy buffers and plain-text paste are "unassigned";
/// registering them would hijack common Alt/Ctrl chords in every application.
pub fn bindings_from_settings(values: &serde_json::Value) -> Result<Vec<Binding>, String> {
    let string = |path: &str| {
        values
            .pointer(path)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
    };
    let mut bindings = Vec::new();
    let open = {
        let configured = string("/hotkeys/open_chord");
        if configured.trim().is_empty() {
            DEFAULT_OPEN_CHORD
        } else {
            configured
        }
    };
    if let Some(binding) = Binding::parse(open, HotkeyAction::ToggleOpen)? {
        bindings.push(binding);
    }
    if let Some(binding) =
        Binding::parse(string("/hotkeys/open_alt_chord"), HotkeyAction::ToggleOpen)?
    {
        bindings.push(binding);
    }
    if let Some(binding) = Binding::parse(
        string("/hotkeys/plain_text_paste_chord"),
        HotkeyAction::PlainPaste,
    )? {
        bindings.push(binding);
    }
    if let Some(copy_buffers) = values
        .pointer("/hotkeys/copy_buffers")
        .and_then(serde_json::Value::as_array)
    {
        for (index, item) in copy_buffers.iter().enumerate() {
            let slot = u8::try_from(index + 1).unwrap_or(u8::MAX);
            if !(1..=5).contains(&slot) {
                continue;
            }
            if let Some(binding) = Binding::parse(
                item.get("copy")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(""),
                HotkeyAction::CopyBufferCopy(slot),
            )? {
                bindings.push(binding);
            }
            if let Some(binding) = Binding::parse(
                item.get("paste")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(""),
                HotkeyAction::CopyBufferPaste(slot),
            )? {
                bindings.push(binding);
            }
            // Cut is not a global action in v1: it needs a popup selection.
        }
    }
    validate_bindings(&bindings)?;
    Ok(bindings)
}

/// Rejects duplicate chords (including two actions on one chord) so Settings
/// can report a conflict instead of silently registering the last one.
pub fn validate_bindings(bindings: &[Binding]) -> Result<(), String> {
    for (index, binding) in bindings.iter().enumerate() {
        for other in &bindings[index + 1..] {
            if binding.shortcut == other.shortcut {
                return Err(format!(
                    "shortcut {} is assigned to more than one action",
                    binding.chord
                ));
            }
        }
    }
    Ok(())
}

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

/// Registers every binding. On the first failure, bindings registered by this
/// call are removed again and the error is returned; the caller never ends up
/// with a partially applied hotkey set.
pub fn register_all(
    registrar: &mut dyn ShortcutRegistrar,
    bindings: &[Binding],
) -> Result<(), String> {
    let mut registered = Vec::new();
    for binding in bindings {
        if let Err(error) = registrar.register(&binding.shortcut) {
            for shortcut in registered {
                let _ = registrar.unregister(&shortcut);
            }
            return Err(format!(
                "{} could not be registered: {error}",
                binding.chord
            ));
        }
        registered.push(binding.shortcut);
    }
    Ok(())
}

/// Atomically replaces `previous` with `next`. Bindings that keep the same
/// chord/action stay registered; removed chords are unregistered and new ones
/// registered. Failure restores the previous set exactly.
pub fn swap_bindings(
    registrar: &mut dyn ShortcutRegistrar,
    previous: &[Binding],
    next: &[Binding],
) -> Result<(), String> {
    let unchanged = |binding: &Binding| {
        next.iter().any(|candidate| {
            candidate.action == binding.action && candidate.shortcut == binding.shortcut
        })
    };
    let mut removed = Vec::new();
    for binding in previous {
        if !unchanged(binding) && registrar.unregister(&binding.shortcut).is_ok() {
            removed.push(binding.shortcut);
        }
    }
    let mut added = Vec::new();
    for binding in next {
        let already = previous.iter().any(|candidate| {
            candidate.action == binding.action && candidate.shortcut == binding.shortcut
        });
        if already {
            continue;
        }
        if let Err(error) = registrar.register(&binding.shortcut) {
            for shortcut in &added {
                let _ = registrar.unregister(shortcut);
            }
            for shortcut in &removed {
                let _ = registrar.register(shortcut);
            }
            return Err(format!(
                "{} could not be registered: {error}",
                binding.chord
            ));
        }
        added.push(binding.shortcut);
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

    #[test]
    fn settings_bindings_include_every_global_action() {
        let values = serde_json::json!({
            "hotkeys": {
                "open_chord": "Ctrl+`",
                "open_alt_chord": "Ctrl+Alt+K",
                "plain_text_paste_chord": "Ctrl+Shift+P",
                "copy_buffers": [
                    {"slot": 1, "copy": "Ctrl+Alt+1", "paste": "Alt+1", "cut": ""},
                    {"slot": 2, "copy": "", "paste": "", "cut": ""}
                ]
            }
        });
        let bindings = bindings_from_settings(&values).expect("bindings");
        assert!(
            bindings
                .iter()
                .any(|binding| binding.action == HotkeyAction::ToggleOpen)
        );
        assert!(
            bindings
                .iter()
                .any(|binding| binding.action == HotkeyAction::PlainPaste)
        );
        assert!(
            bindings
                .iter()
                .any(|binding| binding.action == HotkeyAction::CopyBufferCopy(1))
        );
        assert!(
            bindings
                .iter()
                .any(|binding| binding.action == HotkeyAction::CopyBufferPaste(1))
        );
        assert_eq!(bindings.len(), 5);
    }

    #[test]
    fn unassigned_legacy_chords_are_not_registered() {
        let values = serde_json::json!({
            "hotkeys": {
                "open_chord": "Ctrl+`",
                "plain_text_paste_chord": "",
                "copy_buffers": [{"slot": 1, "copy": "", "paste": "", "cut": ""}]
            }
        });
        let bindings = bindings_from_settings(&values).expect("bindings");
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].action, HotkeyAction::ToggleOpen);
    }

    #[test]
    fn duplicate_chords_are_rejected() {
        let values = serde_json::json!({
            "hotkeys": {
                "open_chord": "Ctrl+Alt+K",
                "open_alt_chord": "Ctrl+Alt+K"
            }
        });
        assert!(bindings_from_settings(&values).is_err());
    }

    #[test]
    fn swap_bindings_moves_buffers_and_rolls_back_on_conflict() {
        let previous = bindings_from_settings(&serde_json::json!({
            "hotkeys": {"open_chord": "Ctrl+`"}
        }))
        .expect("previous");
        let next = bindings_from_settings(&serde_json::json!({
            "hotkeys": {
                "open_chord": "Ctrl+Alt+K",
                "copy_buffers": [{"copy": "Ctrl+Alt+1", "paste": "Alt+1"}]
            }
        }))
        .expect("next");
        let mut registrar = MockRegistrar::default();
        register_all(&mut registrar, &previous).expect("initial");
        swap_bindings(&mut registrar, &previous, &next).expect("swap");
        let next_chords = |action: HotkeyAction| {
            next.iter()
                .find(|binding| binding.action == action)
                .map(|binding| binding.shortcut.to_string())
        };
        assert!(
            !registrar
                .registered
                .contains(&previous[0].shortcut.to_string())
        );
        assert!(
            registrar
                .registered
                .contains(&next_chords(HotkeyAction::ToggleOpen).unwrap())
        );
        assert!(
            registrar
                .registered
                .contains(&next_chords(HotkeyAction::CopyBufferCopy(1)).unwrap())
        );

        let failing = bindings_from_settings(&serde_json::json!({
            "hotkeys": {"open_chord": "Ctrl+Alt+J"}
        }))
        .expect("failing");
        registrar.fail_on = Some(failing[0].shortcut.to_string());
        let error = swap_bindings(&mut registrar, &next, &failing).expect_err("conflict");
        assert!(error.contains("could not be registered"));
        assert!(
            registrar
                .registered
                .contains(&next_chords(HotkeyAction::ToggleOpen).unwrap())
        );
        assert!(
            registrar
                .registered
                .contains(&next_chords(HotkeyAction::CopyBufferCopy(1)).unwrap())
        );
        assert!(
            !registrar
                .registered
                .contains(&failing[0].shortcut.to_string())
        );
    }
}
