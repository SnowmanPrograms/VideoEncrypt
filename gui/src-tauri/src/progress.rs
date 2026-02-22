use media_lock_core::{AppError, ProgressHandler};
use parking_lot::RwLock;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize)]
pub struct ProgressEvent {
    pub task_id: String,
    pub phase: ProgressPhase,
    pub total_bytes: u64,
    pub processed_bytes: u64,
    pub current_file: Option<String>,
    pub message: String,
    pub stats: Option<TaskProgressStats>,
}

#[derive(Debug, Clone, Serialize)]
pub enum ProgressPhase {
    Idle,
    Checking,
    Analyzing,
    Backup,
    Processing,
    Finalizing,
    Completed,
    Failed,
    Cancelled,
}

impl Default for ProgressPhase {
    fn default() -> Self {
        ProgressPhase::Idle
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskProgressStats {
    pub iframe_count: usize,
    pub audio_count: usize,
    pub parse_time_ms: u64,
    pub kdf_time_ms: u64,
    pub io_time_ms: u64,
    pub crypto_time_ms: u64,
    pub throughput_mbps: f64,
}

pub struct TauriProgressHandler {
    app_handle: AppHandle,
    task_id: String,
    state: Arc<RwLock<ProgressState>>,
}

#[derive(Debug, Default)]
struct ProgressState {
    total_bytes: u64,
    processed_bytes: u64,
    phase: ProgressPhase,
    message: String,
}

impl TauriProgressHandler {
    pub fn new(app_handle: AppHandle, task_id: String) -> Self {
        Self {
            app_handle,
            task_id,
            state: Arc::new(RwLock::new(ProgressState::default())),
        }
    }

    fn emit(&self, phase: ProgressPhase, message: &str, processed_delta: u64) {
        let mut state = self.state.write();
        state.phase = phase.clone();
        state.message = message.to_string();
        state.processed_bytes += processed_delta;

        let event = ProgressEvent {
            task_id: self.task_id.clone(),
            phase,
            total_bytes: state.total_bytes,
            processed_bytes: state.processed_bytes,
            current_file: None,
            message: state.message.clone(),
            stats: None,
        };

        let _ = self.app_handle.emit("task-progress", &event);
    }

    fn emit_with_stats(
        &self,
        phase: ProgressPhase,
        message: &str,
        stats: Option<TaskProgressStats>,
    ) {
        let state = self.state.read();

        let event = ProgressEvent {
            task_id: self.task_id.clone(),
            phase,
            total_bytes: state.total_bytes,
            processed_bytes: state.processed_bytes,
            current_file: None,
            message: message.to_string(),
            stats,
        };

        let _ = self.app_handle.emit("task-progress", &event);
    }

    pub fn set_total_bytes(&self, total: u64) {
        self.state.write().total_bytes = total;
    }

    #[allow(dead_code)]
    pub fn get_processed_bytes(&self) -> u64 {
        self.state.read().processed_bytes
    }
}

impl ProgressHandler for TauriProgressHandler {
    fn on_start(&self, total_bytes: u64, message: &str) {
        self.set_total_bytes(total_bytes);
        self.emit(ProgressPhase::Processing, message, 0);
    }

    fn on_progress(&self, delta_bytes: u64) {
        self.emit(ProgressPhase::Processing, "", delta_bytes);
    }

    fn on_message(&self, message: &str) {
        let phase = match message {
            m if m.contains("check") || m.contains("Checking") => ProgressPhase::Checking,
            m if m.contains("analyz") || m.contains("Analyzing") => ProgressPhase::Analyzing,
            m if m.contains("backup") || m.contains("WAL") => ProgressPhase::Backup,
            m if m.contains("finaliz") || m.contains("Finalizing") => ProgressPhase::Finalizing,
            _ => return,
        };
        self.emit(phase, message, 0);
    }

    fn on_finish(&self) {
        self.emit_with_stats(
            ProgressPhase::Completed,
            "Task completed successfully",
            None,
        );
    }

    fn on_error(&self, err: &AppError) {
        self.emit_with_stats(ProgressPhase::Failed, &err.to_string(), None);
    }
}
