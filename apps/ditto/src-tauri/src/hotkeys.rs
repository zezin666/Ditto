//! Global hotkey parsing and atomic registration swaps.
//!
//! Settings store user-facing chords such as `Ctrl+\`` or `Ctrl+Alt+D`. The
//! `global-hotkey` parser used by Tauri accepts exactly that syntax, but a
//! chord may be syntactically valid and still be owned by another application.
//! Registration is swapped atomically: the previous chord is restored when the
//! new one cannot be registered.

use serde_json::{Map, Value};
use tauri_plugin_global_shortcut::Shortcut;

pub const DEFAULT_OPEN_CHORD: &str = "Ctrl+`";
pub const FALLBACK_OPEN_CHORD: &str = "Ctrl+Alt+D";

/// A configurable global shortcut action. First-ten chords stay popup-scoped
/// (they are only meaningful while the list is focused) and are not part of
/// this global set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    ToggleOpen,
    ToggleOpenAlt,
    PlainPaste,
    CopyBufferCopy(u8),
    CopyBufferPaste(u8),
}

impl HotkeyAction {
    /// Stable identifier used by the Settings UI and persisted conflict list.
    pub fn conflict_key(self) -> String {
        match self {
            Self::ToggleOpen => "open".to_string(),
            Self::ToggleOpenAlt => "openAlt".to_string(),
            Self::PlainPaste => "plainTextPaste".to_string(),
            Self::CopyBufferCopy(slot) => format!("copyBuffer.{slot}.copy"),
            Self::CopyBufferPaste(slot) => format!("copyBuffer.{slot}.paste"),
        }
    }

    pub const fn is_open(self) -> bool {
        matches!(self, Self::ToggleOpen | Self::ToggleOpenAlt)
    }
}

/// Why a configured chord is not part of the effective global set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingIssue {
    pub action: HotkeyAction,
    pub chord: String,
    /// Stable code persisted for the Settings UI: `in_use`, `invalid`,
    /// `duplicate`, or `fallback`.
    pub reason: &'static str,
    /// The chord that is effective instead, when one could be registered.
    pub effective: Option<String>,
    /// Human-readable detail for logs only.
    pub detail: Option<String>,
}

impl BindingIssue {
    fn new(
        action: HotkeyAction,
        chord: impl Into<String>,
        reason: &'static str,
        detail: Option<String>,
    ) -> Self {
        Self {
            action,
            chord: chord.into(),
            reason,
            effective: None,
            detail,
        }
    }
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

/// Reads every global chord from the persisted settings JSON and reports each
/// entry that cannot be used instead of failing the whole set. Startup uses
/// this to keep the working chords and reconcile the persisted settings.
///
/// The legacy defaults for copy buffers and plain-text paste are "unassigned";
/// registering them would hijack common Alt/Ctrl chords in every application.
pub fn bindings_with_issues(values: &serde_json::Value) -> (Vec<Binding>, Vec<BindingIssue>) {
    let string = |path: &str| {
        values
            .pointer(path)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
    };
    let mut bindings = Vec::new();
    let mut issues = Vec::new();
    let mut push = |chord: &str, action: HotkeyAction| {
        let chord = chord.trim();
        if chord.is_empty() {
            return;
        }
        match parse_chord(chord) {
            Err(error) => issues.push(BindingIssue::new(action, chord, "invalid", Some(error))),
            Ok(shortcut) => {
                if let Some(existing) = bindings
                    .iter()
                    .find(|binding: &&Binding| binding.shortcut == shortcut)
                {
                    issues.push(BindingIssue::new(
                        action,
                        chord,
                        "duplicate",
                        Some(format!(
                            "also assigned to {}",
                            existing.action.conflict_key()
                        )),
                    ));
                } else {
                    bindings.push(Binding {
                        chord: chord.to_string(),
                        shortcut,
                        action,
                    });
                }
            }
        }
    };
    let open = {
        let configured = string("/hotkeys/open_chord");
        if configured.trim().is_empty() {
            DEFAULT_OPEN_CHORD
        } else {
            configured
        }
    };
    push(open, HotkeyAction::ToggleOpen);
    push(
        string("/hotkeys/open_alt_chord"),
        HotkeyAction::ToggleOpenAlt,
    );
    push(
        string("/hotkeys/plain_text_paste_chord"),
        HotkeyAction::PlainPaste,
    );
    if let Some(copy_buffers) = values
        .pointer("/hotkeys/copy_buffers")
        .and_then(serde_json::Value::as_array)
    {
        for (index, item) in copy_buffers.iter().enumerate() {
            let slot = u8::try_from(index + 1).unwrap_or(u8::MAX);
            if !(1..=5).contains(&slot) {
                continue;
            }
            push(
                item.get("copy")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(""),
                HotkeyAction::CopyBufferCopy(slot),
            );
            push(
                item.get("paste")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(""),
                HotkeyAction::CopyBufferPaste(slot),
            );
            // Cut is not a global action in v1: it is deferred (Should) and
            // has no popup/provider action either.
        }
    }
    (bindings, issues)
}

/// Strict variant used by the runtime Settings Apply path. Any invalid or
/// duplicate chord rejects the whole update so it can roll back atomically.
pub fn bindings_from_settings(values: &serde_json::Value) -> Result<Vec<Binding>, String> {
    let (bindings, issues) = bindings_with_issues(values);
    if let Some(issue) = issues.first() {
        return Err(format!(
            "{} {}: {}",
            issue.chord,
            issue.reason,
            issue.detail.as_deref().unwrap_or("not usable")
        ));
    }
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

/// Registers every binding independently at startup. Successful chords stay
/// registered even when a later chord is owned by another application; each
/// failure is reported so the caller can clear the persisted value and tell
/// the user which binding is not effective.
pub fn register_all_collecting(
    registrar: &mut dyn ShortcutRegistrar,
    bindings: &[Binding],
) -> (Vec<Binding>, Vec<BindingIssue>) {
    let mut registered = Vec::new();
    let mut issues = Vec::new();
    for binding in bindings {
        match registrar.register(&binding.shortcut) {
            Ok(()) => registered.push(binding.clone()),
            Err(error) => issues.push(BindingIssue::new(
                binding.action,
                binding.chord.clone(),
                "in_use",
                Some(error),
            )),
        }
    }
    (registered, issues)
}

/// Builds the persisted settings overrides that reconcile the startup attempt:
/// the effective open chord, every failed non-open chord cleared, and the
/// conflict list that Settings displays. `values` is the settings snapshot
/// read before registration.
pub fn reconcile_settings(values: &Value, effective_open: &str, issues: &[BindingIssue]) -> Value {
    let mut root = values.clone();
    if !root.is_object() {
        root = serde_json::json!({});
    }
    let root = root.as_object_mut().expect("settings root is an object");
    let hotkeys = root
        .entry("hotkeys")
        .or_insert_with(|| Value::Object(Map::new()));
    if !hotkeys.is_object() {
        *hotkeys = Value::Object(Map::new());
    }
    let hotkeys = hotkeys.as_object_mut().expect("hotkeys is an object");
    hotkeys.insert(
        "open_chord".to_string(),
        Value::String(effective_open.to_string()),
    );
    for issue in issues {
        match issue.action {
            // The open action is always replaced by the effective fallback
            // above, so its original chord is only reported, not kept.
            HotkeyAction::ToggleOpen => {}
            HotkeyAction::ToggleOpenAlt => {
                hotkeys.insert("open_alt_chord".to_string(), Value::String(String::new()));
            }
            HotkeyAction::PlainPaste => {
                hotkeys.insert(
                    "plain_text_paste_chord".to_string(),
                    Value::String(String::new()),
                );
            }
            HotkeyAction::CopyBufferCopy(slot) | HotkeyAction::CopyBufferPaste(slot) => {
                let field = if matches!(issue.action, HotkeyAction::CopyBufferCopy(_)) {
                    "copy"
                } else {
                    "paste"
                };
                let buffers = hotkeys
                    .entry("copy_buffers")
                    .or_insert_with(|| Value::Array(Vec::new()));
                if let Some(items) = buffers.as_array_mut() {
                    let index = usize::from(slot.saturating_sub(1));
                    if let Some(item) = items.get_mut(index).and_then(Value::as_object_mut) {
                        item.insert(field.to_string(), Value::String(String::new()));
                    }
                }
            }
        }
    }
    let conflicts = issues
        .iter()
        .map(|issue| {
            serde_json::json!({
                "action": issue.action.conflict_key(),
                "chord": issue.chord,
                "reason": issue.reason,
                "effective": issue.effective,
            })
        })
        .collect::<Vec<_>>();
    hotkeys.insert("startup_conflicts".to_string(), Value::Array(conflicts));
    serde_json::json!({ "hotkeys": Value::Object(hotkeys.clone()) })
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

    #[test]
    fn open_alt_has_its_own_action_for_reconciliation() {
        let (bindings, issues) = bindings_with_issues(&serde_json::json!({
            "hotkeys": {
                "open_chord": "Ctrl+`",
                "open_alt_chord": "Ctrl+Alt+K"
            }
        }));
        assert!(issues.is_empty());
        assert!(
            bindings
                .iter()
                .any(|binding| binding.action == HotkeyAction::ToggleOpenAlt)
        );
    }

    #[test]
    fn tolerant_parse_reports_invalid_and_duplicate_bindings() {
        let (bindings, issues) = bindings_with_issues(&serde_json::json!({
            "hotkeys": {
                "open_chord": "Ctrl+`",
                "open_alt_chord": "Ctrl+`",
                "plain_text_paste_chord": "NotAChord",
                "copy_buffers": [{"copy": "Ctrl+Alt+1", "paste": "Ctrl+Alt+1"}]
            }
        }));
        assert_eq!(bindings.len(), 2, "open and copy remain usable");
        assert!(issues.iter().any(|issue| {
            issue.action == HotkeyAction::ToggleOpenAlt && issue.reason == "duplicate"
        }));
        assert!(
            issues
                .iter()
                .any(|issue| issue.reason == "invalid" && issue.action == HotkeyAction::PlainPaste)
        );
        assert!(issues.iter().any(|issue| {
            issue.action == HotkeyAction::CopyBufferPaste(1) && issue.reason == "duplicate"
        }));
        assert!(
            bindings_from_settings(&serde_json::json!({
                "hotkeys": {"open_chord": "Ctrl+`", "open_alt_chord": "Ctrl+`"}
            }))
            .is_err(),
            "the Apply path must stay strict"
        );
    }

    #[test]
    fn reconciliation_clears_only_the_uneffective_bindings() {
        let values = serde_json::json!({
            "hotkeys": {
                "open_chord": "Ctrl+Alt+K",
                "open_alt_chord": "Ctrl+Alt+J",
                "plain_text_paste_chord": "Ctrl+Shift+P",
                "copy_buffers": [
                    {"slot": 1, "copy": "Ctrl+Alt+1", "paste": "Alt+1", "cut": "Ctrl+Alt+X"},
                    {"slot": 2, "copy": "Ctrl+Alt+2", "paste": "", "cut": ""}
                ],
                "first_ten_enabled": true
            }
        });
        let issues = vec![BindingIssue {
            action: HotkeyAction::CopyBufferCopy(1),
            chord: "Ctrl+Alt+1".to_string(),
            reason: "in_use",
            effective: None,
            detail: Some("already in use".to_string()),
        }];
        let reconciled = reconcile_settings(&values, "Ctrl+Alt+J", &issues);
        let hotkeys = &reconciled["hotkeys"];
        assert_eq!(hotkeys["open_chord"], "Ctrl+Alt+J");
        assert_eq!(hotkeys["open_alt_chord"], "Ctrl+Alt+J");
        assert_eq!(hotkeys["plain_text_paste_chord"], "Ctrl+Shift+P");
        assert_eq!(hotkeys["copy_buffers"][0]["copy"], "");
        assert_eq!(hotkeys["copy_buffers"][0]["paste"], "Alt+1");
        assert_eq!(hotkeys["copy_buffers"][0]["cut"], "Ctrl+Alt+X");
        assert_eq!(hotkeys["copy_buffers"][1]["copy"], "Ctrl+Alt+2");
        assert_eq!(hotkeys["first_ten_enabled"], true);
        assert_eq!(
            hotkeys["startup_conflicts"][0]["action"],
            "copyBuffer.1.copy"
        );
        assert_eq!(hotkeys["startup_conflicts"][0]["reason"], "in_use");

        let fallback = reconcile_settings(
            &values,
            "Ctrl+`",
            &[BindingIssue {
                action: HotkeyAction::ToggleOpen,
                chord: "Ctrl+Alt+K".to_string(),
                reason: "fallback",
                effective: Some("Ctrl+`".to_string()),
                detail: None,
            }],
        );
        assert_eq!(fallback["hotkeys"]["open_chord"], "Ctrl+`");
        assert_eq!(
            fallback["hotkeys"]["startup_conflicts"][0]["effective"],
            "Ctrl+`"
        );
        // A conflict list is always persisted, so a later successful launch
        // clears stale warnings instead of leaving them behind.
        assert_eq!(
            reconcile_settings(&values, "Ctrl+Alt+K", &[])["hotkeys"]["startup_conflicts"],
            serde_json::json!([])
        );
    }

    #[test]
    fn independent_registration_keeps_working_chords() {
        let bindings = bindings_from_settings(&serde_json::json!({
            "hotkeys": {
                "open_chord": "Ctrl+`",
                "copy_buffers": [{"copy": "Ctrl+Alt+1", "paste": ""}]
            }
        }))
        .expect("bindings");
        let mut registrar = MockRegistrar {
            fail_on: Some(bindings[1].shortcut.to_string()),
            ..MockRegistrar::default()
        };
        let (registered, issues) = register_all_collecting(&mut registrar, &bindings);
        assert_eq!(registered.len(), 1);
        assert_eq!(registered[0].action, HotkeyAction::ToggleOpen);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].action, HotkeyAction::CopyBufferCopy(1));
        assert_eq!(issues[0].reason, "in_use");
        assert!(
            registrar
                .registered
                .contains(&bindings[0].shortcut.to_string()),
            "the open chord must stay registered"
        );
    }
}
