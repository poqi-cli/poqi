use crate::semantic_bootstrap::SemanticBootstrapEvent;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tokio::sync::mpsc::UnboundedSender;

pub(crate) const MEBIBYTE: u64 = 1024 * 1024;
const PROGRESS_UPDATE_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResumeAction {
    StartFresh,
    Resume(u64),
    VerifyComplete,
    DeleteAndRestart,
}

pub(crate) fn resume_action(
    existing_bytes: Option<u64>,
    expected_total: Option<u64>,
) -> ResumeAction {
    let Some(existing) = existing_bytes else {
        return ResumeAction::StartFresh;
    };
    if existing == 0 {
        return ResumeAction::StartFresh;
    }
    match expected_total {
        Some(total) if existing < total => ResumeAction::Resume(existing),
        Some(total) if existing == total => ResumeAction::VerifyComplete,
        Some(_) => ResumeAction::DeleteAndRestart,
        None => ResumeAction::Resume(existing),
    }
}

pub(crate) fn temp_download_path(dest: &Path) -> PathBuf {
    let filename = dest
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download");
    dest.with_file_name(format!("{filename}.download"))
}

pub(crate) fn send_status(
    event_tx: Option<&UnboundedSender<SemanticBootstrapEvent>>,
    message: impl Into<String>,
) {
    if let Some(tx) = event_tx {
        let _ = tx.send(SemanticBootstrapEvent::Status(message.into()));
    }
}

pub(crate) struct ProgressReporter<'a> {
    phase: &'static str,
    label: &'a str,
    total_bytes: Option<u64>,
    event_tx: Option<&'a UnboundedSender<SemanticBootstrapEvent>>,
    last_marker: Option<u64>,
    last_emit: Instant,
}

impl<'a> ProgressReporter<'a> {
    pub(crate) fn new(
        phase: &'static str,
        label: &'a str,
        total_bytes: Option<u64>,
        event_tx: Option<&'a UnboundedSender<SemanticBootstrapEvent>>,
    ) -> Self {
        let now = Instant::now();
        Self {
            phase,
            label,
            total_bytes,
            event_tx,
            last_marker: None,
            last_emit: now.checked_sub(PROGRESS_UPDATE_INTERVAL).unwrap_or(now),
        }
    }

    pub(crate) fn emit(&mut self, bytes_done: u64) {
        self.last_marker = Some(progress_marker(bytes_done, self.total_bytes));
        self.last_emit = Instant::now();
        send_status(
            self.event_tx,
            format_progress(self.phase, self.label, bytes_done, self.total_bytes),
        );
    }

    pub(crate) fn maybe_emit(&mut self, bytes_done: u64) {
        let marker = progress_marker(bytes_done, self.total_bytes);
        if should_emit_progress(self.last_marker, marker, self.last_emit.elapsed()) {
            self.emit(bytes_done);
        }
    }

    pub(crate) fn finish(&mut self, bytes_done: u64) {
        self.emit(bytes_done);
    }
}

pub(crate) fn format_progress(
    phase: &str,
    label: &str,
    bytes_done: u64,
    total_bytes: Option<u64>,
) -> String {
    if let Some(total) = total_bytes {
        return format!(
            "{phase} {label}: {}% ({} / {} MiB)",
            progress_percent(bytes_done, total),
            format_mib_value(bytes_done),
            format_mib_value(total)
        );
    }
    format!("{phase} {label}: {} MiB", format_mib_value(bytes_done))
}

pub(crate) fn format_mib(bytes: u64) -> String {
    format!("{} MiB", format_mib_value(bytes))
}

fn should_emit_progress(last_marker: Option<u64>, next_marker: u64, elapsed: Duration) -> bool {
    last_marker != Some(next_marker) || elapsed >= PROGRESS_UPDATE_INTERVAL
}

fn progress_marker(bytes_done: u64, total_bytes: Option<u64>) -> u64 {
    if let Some(total) = total_bytes {
        return progress_percent(bytes_done, total);
    }
    bytes_done / MEBIBYTE
}

const fn progress_percent(bytes_done: u64, total_bytes: u64) -> u64 {
    if total_bytes == 0 {
        return 100;
    }
    let clamped = if bytes_done > total_bytes {
        total_bytes
    } else {
        bytes_done
    };
    clamped.saturating_mul(100) / total_bytes
}

fn format_mib_value(bytes: u64) -> String {
    let whole = bytes / MEBIBYTE;
    let tenths = ((bytes % MEBIBYTE) * 10) / MEBIBYTE;
    format!("{whole}.{tenths}")
}

#[cfg(test)]
mod tests {
    use super::{
        format_progress, progress_percent, resume_action, should_emit_progress, ResumeAction,
        MEBIBYTE, PROGRESS_UPDATE_INTERVAL,
    };
    use std::time::Duration;

    #[test]
    fn resume_action_handles_known_totals() {
        assert_eq!(resume_action(None, Some(100)), ResumeAction::StartFresh);
        assert_eq!(resume_action(Some(0), Some(100)), ResumeAction::StartFresh);
        assert_eq!(resume_action(Some(42), Some(100)), ResumeAction::Resume(42));
        assert_eq!(
            resume_action(Some(100), Some(100)),
            ResumeAction::VerifyComplete
        );
        assert_eq!(
            resume_action(Some(150), Some(100)),
            ResumeAction::DeleteAndRestart
        );
    }

    #[test]
    fn resume_action_resumes_when_total_is_unknown() {
        assert_eq!(resume_action(Some(42), None), ResumeAction::Resume(42));
    }

    #[test]
    fn progress_percent_clamps_to_expected_size() {
        assert_eq!(progress_percent(0, 100), 0);
        assert_eq!(progress_percent(42, 100), 42);
        assert_eq!(progress_percent(150, 100), 100);
        assert_eq!(progress_percent(10, 0), 100);
    }

    #[test]
    fn format_progress_uses_percent_and_mib_for_known_total() {
        assert_eq!(
            format_progress(
                "Downloading",
                "model.onnx_data",
                512 * MEBIBYTE,
                Some(MEBIBYTE * 1024)
            ),
            "Downloading model.onnx_data: 50% (512.0 / 1024.0 MiB)"
        );
    }

    #[test]
    fn format_progress_uses_mib_when_total_is_unknown() {
        assert_eq!(
            format_progress("Downloading", "onnxruntime.dll", 2 * MEBIBYTE, None),
            "Downloading onnxruntime.dll: 2.0 MiB"
        );
    }

    #[test]
    fn progress_updates_when_marker_changes_or_interval_elapses() {
        assert!(should_emit_progress(Some(41), 42, Duration::ZERO));
        assert!(should_emit_progress(Some(42), 42, PROGRESS_UPDATE_INTERVAL));
        assert!(!should_emit_progress(
            Some(42),
            42,
            PROGRESS_UPDATE_INTERVAL
                .checked_sub(Duration::from_millis(1))
                .unwrap_or_default()
        ));
    }
}
