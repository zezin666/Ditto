use std::{path::Path, thread, time::Duration};

use ditto_clipboard::WriteSuppression;
use ditto_model::ClipDetail;
use parking_lot::Mutex;

use crate::{
    BatchMode, PasteError, PasteFormat, PasteOptions, PasteResult, SendInputBackend, TargetWindow,
    WindowsClipboardBackend, WindowsTargetBackend, build_payload, clipboard::suppression_hash,
    elevation::UacElevationBackend,
};

pub trait ClipboardBackend: Send + Sync {
    fn snapshot(&self) -> PasteResult<Vec<PasteFormat>>;
    fn write(&self, formats: &[PasteFormat]) -> PasteResult<()>;
}

pub trait TargetBackend: Send + Sync {
    fn capture(&self) -> PasteResult<TargetWindow>;
    fn activate(&self, target: &TargetWindow) -> PasteResult<()>;
    fn is_current_process_elevated(&self) -> bool;
}

pub trait InputBackend: Send + Sync {
    fn send_ctrl_v(&self) -> PasteResult<()>;
}

pub trait ElevationBackend: Send + Sync {
    fn paste_elevated(
        &self,
        target: &TargetWindow,
        clip_id: i64,
        mode: ditto_uac_helper::HelperMode,
        focus_delay_ms: u64,
    ) -> PasteResult<()>;
}

#[derive(Debug, Clone)]
pub struct PasteReport {
    pub mode: BatchMode,
    pub elevated: bool,
    pub target_pid: Option<u32>,
    pub pasted_clip_ids: Vec<i64>,
    pub failed_clip_id: Option<i64>,
    pub restored_clipboard: bool,
}

#[derive(Debug, Clone)]
pub struct PasteFailure {
    pub error: PasteError,
    pub report: PasteReport,
}

pub struct PasteEngine<C, T, I, E> {
    data_root: std::path::PathBuf,
    suppression: std::sync::Arc<WriteSuppression>,
    clipboard: C,
    target: T,
    input: I,
    elevation: E,
}

pub type WindowsPasteEngine = PasteEngine<
    WindowsClipboardBackend,
    WindowsTargetBackend,
    SendInputBackend,
    UacElevationBackend,
>;

impl WindowsPasteEngine {
    pub fn new(
        data_root: impl Into<std::path::PathBuf>,
        suppression: std::sync::Arc<WriteSuppression>,
    ) -> Self {
        Self::with_backends(
            data_root,
            suppression,
            WindowsClipboardBackend,
            WindowsTargetBackend,
            SendInputBackend,
            UacElevationBackend::discover(),
        )
    }

    pub fn new_with_helper(
        data_root: impl Into<std::path::PathBuf>,
        suppression: std::sync::Arc<WriteSuppression>,
        helper_path: std::path::PathBuf,
    ) -> Self {
        Self::with_backends(
            data_root,
            suppression,
            WindowsClipboardBackend,
            WindowsTargetBackend,
            SendInputBackend,
            UacElevationBackend::new(helper_path),
        )
    }
}

impl<C, T, I, E> PasteEngine<C, T, I, E>
where
    C: ClipboardBackend,
    T: TargetBackend,
    I: InputBackend,
    E: ElevationBackend,
{
    pub fn with_backends(
        data_root: impl Into<std::path::PathBuf>,
        suppression: std::sync::Arc<WriteSuppression>,
        clipboard: C,
        target: T,
        input: I,
        elevation: E,
    ) -> Self {
        Self {
            data_root: data_root.into(),
            suppression,
            clipboard,
            target,
            input,
            elevation,
        }
    }

    pub fn paste_many(
        &self,
        clips: &[ClipDetail],
        options: &PasteOptions,
    ) -> Result<PasteReport, PasteFailure> {
        let options = options.clone().normalized();
        let mode = if clips.len() > 1 {
            BatchMode::Multi
        } else if options.plain_text {
            BatchMode::PlainText
        } else {
            BatchMode::Normal
        };
        let mut report = PasteReport {
            mode,
            elevated: false,
            target_pid: None,
            pasted_clip_ids: Vec::new(),
            failed_clip_id: None,
            restored_clipboard: false,
        };

        if clips.is_empty() {
            return Err(PasteFailure {
                error: PasteError::NotFound(0),
                report,
            });
        }

        let target = match self.target.capture() {
            Ok(target) => target,
            Err(error) => {
                report.failed_clip_id = Some(clips[0].summary.id);
                return Err(PasteFailure { error, report });
            }
        };
        report.target_pid = Some(target.pid);
        report.elevated =
            target.integrity.is_elevated_target() && !self.target.is_current_process_elevated();

        let snapshot = if options.restore_clipboard_on_failure {
            self.clipboard
                .snapshot()
                .ok()
                .filter(|formats| !formats.is_empty())
        } else {
            None
        };
        let helper_mode = match mode {
            BatchMode::Normal => ditto_uac_helper::HelperMode::Normal,
            BatchMode::PlainText => ditto_uac_helper::HelperMode::Plain,
            BatchMode::Multi => ditto_uac_helper::HelperMode::Multi,
        };

        for (index, clip) in clips.iter().enumerate() {
            let result = self.paste_one(clip, &target, &options, helper_mode);
            if let Err(error) = result {
                report.failed_clip_id = Some(clip.summary.id);
                if let Some(snapshot) = snapshot.as_ref() {
                    if self.clipboard.write(snapshot).is_ok() {
                        report.restored_clipboard = true;
                    }
                }
                return Err(PasteFailure { error, report });
            }
            report.pasted_clip_ids.push(clip.summary.id);
            if index + 1 < clips.len() {
                thread::sleep(Duration::from_millis(options.multi_interval_ms));
            }
        }
        Ok(report)
    }

    fn paste_one(
        &self,
        clip: &ClipDetail,
        target: &TargetWindow,
        options: &PasteOptions,
        helper_mode: ditto_uac_helper::HelperMode,
    ) -> PasteResult<()> {
        let formats = build_payload(clip, Path::new(&self.data_root), options)?;
        let hash = suppression_hash(&formats)?;
        self.suppression.remember(hash);
        self.clipboard.write(&formats)?;

        if target.integrity.is_elevated_target() && !self.target.is_current_process_elevated() {
            return self.elevation.paste_elevated(
                target,
                clip.summary.id,
                helper_mode,
                options.focus_delay_ms,
            );
        }

        self.target.activate(target)?;
        if options.focus_delay_ms > 0 {
            thread::sleep(Duration::from_millis(options.focus_delay_ms));
        }
        self.input.send_ctrl_v()
    }
}

#[allow(dead_code)]
struct _FormatSequence(Mutex<Vec<PasteFormat>>);

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicU32, Ordering},
        },
    };

    use ditto_clipboard::WriteSuppression;
    use ditto_model::{ClipDetail, ClipFormatInfo, ClipKind, ClipSummary};
    use parking_lot::Mutex;

    use super::*;
    use crate::{IntegrityLevel, TargetWindow};

    #[derive(Default)]
    struct MockClipboard {
        writes: Mutex<Vec<Vec<String>>>,
        snapshot: Mutex<Vec<PasteFormat>>,
    }

    impl ClipboardBackend for MockClipboard {
        fn snapshot(&self) -> PasteResult<Vec<PasteFormat>> {
            Ok(self.snapshot.lock().clone())
        }

        fn write(&self, formats: &[PasteFormat]) -> PasteResult<()> {
            self.writes
                .lock()
                .push(formats.iter().map(|format| format.name.clone()).collect());
            Ok(())
        }
    }

    #[derive(Default)]
    struct MockTarget {
        activations: AtomicU32,
        elevated: AtomicBool,
    }

    impl TargetBackend for MockTarget {
        fn capture(&self) -> PasteResult<TargetWindow> {
            Ok(TargetWindow {
                hwnd: 1,
                focus_hwnd: 1,
                pid: 42,
                thread_id: 1,
                class_name: "Notepad".to_string(),
                integrity: IntegrityLevel::Medium,
            })
        }

        fn activate(&self, _target: &TargetWindow) -> PasteResult<()> {
            self.activations.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn is_current_process_elevated(&self) -> bool {
            self.elevated.load(Ordering::SeqCst)
        }
    }

    #[derive(Default)]
    struct MockInput {
        sends: AtomicU32,
    }

    impl InputBackend for MockInput {
        fn send_ctrl_v(&self) -> PasteResult<()> {
            self.sends.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[derive(Default)]
    struct MockElevation {
        calls: AtomicU32,
    }

    impl ElevationBackend for MockElevation {
        fn paste_elevated(
            &self,
            _target: &TargetWindow,
            _clip_id: i64,
            _mode: ditto_uac_helper::HelperMode,
            _focus_delay_ms: u64,
        ) -> PasteResult<()> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn detail(id: i64) -> ClipDetail {
        ClipDetail {
            summary: ClipSummary {
                id,
                kind: ClipKind::Text,
                preview_text: format!("sample {id}"),
                source_app: None,
                created_at_ms: 0,
                last_pasted_at_ms: None,
                pinned: false,
                pin_order: None,
                group_id: None,
                copy_buffer: None,
                formats: vec!["CF_UNICODETEXT".to_string()],
                size_bytes: 9,
                has_image: false,
                image_thumb_ref: None,
                pasted_count: 0,
                state_rev: 1,
            },
            text_content: Some(format!("sample {id}")),
            quick_paste_text: String::new(),
            description: String::new(),
            source_window_title: None,
            formats: vec![ClipFormatInfo {
                format_name: "CF_UNICODETEXT".to_string(),
                seq: 0,
                size_bytes: 2,
                mime: Some("text/plain".to_string()),
                sha256: Vec::new(),
                data: Some(vec![0, 0]),
                data_path: None,
                is_primary: true,
            }],
            dont_auto_delete: false,
            updated_at_ms: 0,
        }
    }

    #[test]
    fn multiple_clips_are_pasted_in_order_and_each_activation_happens() {
        let engine = PasteEngine::with_backends(
            PathBuf::from("."),
            Arc::new(WriteSuppression::default()),
            MockClipboard::default(),
            MockTarget::default(),
            MockInput::default(),
            MockElevation::default(),
        );
        let report = engine
            .paste_many(
                &[detail(1), detail(2), detail(3)],
                &PasteOptions {
                    multi_interval_ms: 50,
                    ..PasteOptions::default()
                },
            )
            .expect("paste");
        assert_eq!(report.pasted_clip_ids, vec![1, 2, 3]);
        assert_eq!(report.mode, BatchMode::Multi);
        assert_eq!(engine.target.activations.load(Ordering::SeqCst), 3);
        assert_eq!(engine.input.sends.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn elevated_target_uses_elevation_backend_without_direct_send() {
        let target = MockTarget::default();
        target.elevated.store(false, Ordering::SeqCst);
        let elevation = MockElevation::default();
        let engine = PasteEngine::with_backends(
            PathBuf::from("."),
            Arc::new(WriteSuppression::default()),
            MockClipboard::default(),
            ElevatedTarget,
            MockInput::default(),
            elevation,
        );
        let report = engine
            .paste_many(&[detail(7)], &PasteOptions::default())
            .expect("paste");
        assert!(report.elevated);
        assert_eq!(engine.elevation.calls.load(Ordering::SeqCst), 1);
        assert_eq!(engine.input.sends.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn paste_stops_at_the_first_failure_and_reports_the_success_prefix() {
        let engine = PasteEngine::with_backends(
            PathBuf::from("."),
            Arc::new(WriteSuppression::default()),
            MockClipboard {
                snapshot: Mutex::new(vec![PasteFormat {
                    name: "CF_TEXT".to_string(),
                    data: b"original".to_vec(),
                    seq: 0,
                    is_primary: true,
                }]),
                ..MockClipboard::default()
            },
            FailSecondTarget::default(),
            MockInput::default(),
            MockElevation::default(),
        );
        let failure = engine
            .paste_many(
                &[detail(1), detail(2), detail(3)],
                &PasteOptions {
                    multi_interval_ms: 180,
                    ..PasteOptions::default()
                },
            )
            .expect_err("second paste must fail");
        assert_eq!(failure.report.pasted_clip_ids, vec![1]);
        assert_eq!(failure.report.failed_clip_id, Some(2));
        assert!(failure.report.restored_clipboard);
        assert_eq!(engine.input.sends.load(Ordering::SeqCst), 1);
        assert_eq!(
            engine.clipboard.writes.lock().last(),
            Some(&vec!["CF_TEXT".to_string()])
        );
    }

    struct ElevatedTarget;

    impl TargetBackend for ElevatedTarget {
        fn capture(&self) -> PasteResult<TargetWindow> {
            Ok(TargetWindow {
                hwnd: 1,
                focus_hwnd: 1,
                pid: 42,
                thread_id: 1,
                class_name: "AdminNotepad".to_string(),
                integrity: IntegrityLevel::High,
            })
        }

        fn activate(&self, _target: &TargetWindow) -> PasteResult<()> {
            Ok(())
        }

        fn is_current_process_elevated(&self) -> bool {
            false
        }
    }

    #[derive(Default)]
    struct FailSecondTarget {
        activations: AtomicU32,
    }

    impl TargetBackend for FailSecondTarget {
        fn capture(&self) -> PasteResult<TargetWindow> {
            Ok(TargetWindow {
                hwnd: 1,
                focus_hwnd: 1,
                pid: 42,
                thread_id: 1,
                class_name: "Notepad".to_string(),
                integrity: IntegrityLevel::Medium,
            })
        }

        fn activate(&self, _target: &TargetWindow) -> PasteResult<()> {
            let activation = self.activations.fetch_add(1, Ordering::SeqCst) + 1;
            if activation == 2 {
                Err(PasteError::Clipboard("target closed".to_string()))
            } else {
                Ok(())
            }
        }

        fn is_current_process_elevated(&self) -> bool {
            false
        }
    }
}
