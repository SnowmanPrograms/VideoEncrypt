use crate::error::{GuiError, Result};
use crate::progress::{ProgressEvent, ProgressPhase, TauriProgressHandler};
use media_lock_core::{EncryptionTask, OperationMode, TaskStats};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskConfig {
    pub files: Vec<String>,
    pub password: String,
    pub mode: TaskMode,
    pub encrypt_audio: bool,
    pub scrub_metadata: bool,
    pub use_wal: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskMode {
    Encrypt,
    Decrypt,
}

impl From<TaskMode> for OperationMode {
    fn from(mode: TaskMode) -> Self {
        match mode {
            TaskMode::Encrypt => OperationMode::Encrypt,
            TaskMode::Decrypt => OperationMode::Decrypt,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskInfo {
    pub id: String,
    pub config: TaskConfig,
    pub status: TaskStatus,
    pub current_file_index: usize,
    pub total_files: usize,
    pub current_file: Option<String>,
    pub progress: f64,
    pub error: Option<String>,
    pub results: Vec<FileResult>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileResult {
    pub path: String,
    pub success: bool,
    pub error: Option<String>,
    pub stats: Option<FileStats>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileStats {
    pub file_size: u64,
    pub data_size: u64,
    pub iframe_count: usize,
    pub audio_count: usize,
    pub total_time_ms: u64,
    pub throughput_mbps: f64,
}

impl From<TaskStats> for FileStats {
    fn from(stats: TaskStats) -> Self {
        Self {
            file_size: stats.file_size,
            data_size: stats.data_size,
            iframe_count: stats.iframe_count,
            audio_count: stats.audio_count,
            total_time_ms: stats.total_time.as_millis() as u64,
            throughput_mbps: stats.perceived_speed_mbps(),
        }
    }
}

struct RunningTask {
    cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct TaskManager {
    tasks: Arc<RwLock<HashMap<String, TaskInfo>>>,
    running: Arc<RwLock<HashMap<String, RunningTask>>>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_task(&self, config: TaskConfig) -> Result<String> {
        if config.files.is_empty() {
            return Err(GuiError::NoFilesSelected);
        }
        if config.password.is_empty() {
            return Err(GuiError::EmptyPassword);
        }

        let id = Uuid::new_v4().to_string();
        let info = TaskInfo {
            id: id.clone(),
            config,
            status: TaskStatus::Pending,
            current_file_index: 0,
            total_files: 0,
            current_file: None,
            progress: 0.0,
            error: None,
            results: Vec::new(),
        };

        self.tasks.write().insert(id.clone(), info);
        Ok(id)
    }

    pub async fn start_task(&self, app_handle: AppHandle, task_id: String) -> Result<()> {
        let config = {
            let tasks = self.tasks.read();
            let task = tasks
                .get(&task_id)
                .ok_or_else(|| GuiError::TaskNotFound(task_id.clone()))?;

            if task.status == TaskStatus::Running {
                return Err(GuiError::TaskAlreadyRunning(task_id));
            }

            task.config.clone()
        };

        {
            let mut tasks = self.tasks.write();
            if let Some(task) = tasks.get_mut(&task_id) {
                task.status = TaskStatus::Running;
                task.total_files = config.files.len();
            }
        }

        let cancelled = Arc::new(AtomicBool::new(false));
        {
            self.running.write().insert(
                task_id.clone(),
                RunningTask {
                    cancelled: cancelled.clone(),
                },
            );
        }

        let manager = TaskManagerRef {
            tasks: self.tasks.clone(),
            running: self.running.clone(),
        };

        let task_id_clone = task_id.clone();

        tokio::spawn(async move {
            let result = manager
                .process_files(
                    app_handle.clone(),
                    &task_id_clone,
                    config,
                    cancelled.clone(),
                )
                .await;

            {
                let mut running = manager.running.write();
                running.remove(&task_id_clone);
            }

            if let Err(e) = result {
                let mut tasks = manager.tasks.write();
                if let Some(task) = tasks.get_mut(&task_id_clone) {
                    if task.status != TaskStatus::Cancelled {
                        task.status = TaskStatus::Failed;
                        task.error = Some(e.to_string());
                    }
                }

                let phase = if matches!(e, GuiError::Cancelled) {
                    ProgressPhase::Cancelled
                } else {
                    ProgressPhase::Failed
                };

                let _ = app_handle.emit(
                    "task-progress",
                    ProgressEvent {
                        task_id: task_id_clone,
                        phase,
                        total_bytes: 0,
                        processed_bytes: 0,
                        current_file: None,
                        message: e.to_string(),
                        stats: None,
                    },
                );
            }
        });

        Ok(())
    }

    fn update_status(&self, task_id: &str, status: TaskStatus) -> Result<()> {
        let mut tasks = self.tasks.write();
        if let Some(task) = tasks.get_mut(task_id) {
            task.status = status;
        }
        Ok(())
    }

    pub fn cancel_task(&self, task_id: &str) -> Result<()> {
        if let Some(running) = self.running.read().get(task_id) {
            running.cancelled.store(true, Ordering::Relaxed);
        }

        self.update_status(task_id, TaskStatus::Cancelled)
    }

    pub fn get_task_info(&self, task_id: &str) -> Option<TaskInfo> {
        self.tasks.read().get(task_id).cloned()
    }
}

struct TaskManagerRef {
    tasks: Arc<RwLock<HashMap<String, TaskInfo>>>,
    running: Arc<RwLock<HashMap<String, RunningTask>>>,
}

impl TaskManagerRef {
    async fn process_files(
        &self,
        app_handle: AppHandle,
        task_id: &str,
        config: TaskConfig,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        let total_files = config.files.len();

        for (index, file_path) in config.files.iter().enumerate() {
            if cancelled.load(Ordering::Relaxed) {
                self.update_status(task_id, TaskStatus::Cancelled)?;
                return Err(GuiError::Cancelled);
            }

            {
                let mut tasks = self.tasks.write();
                if let Some(task) = tasks.get_mut(task_id) {
                    task.current_file_index = index;
                    task.current_file = Some(file_path.clone());
                }
            }

            let _ = app_handle.emit(
                "task-progress",
                ProgressEvent {
                    task_id: task_id.to_string(),
                    phase: ProgressPhase::Checking,
                    total_bytes: 0,
                    processed_bytes: 0,
                    current_file: Some(file_path.clone()),
                    message: format!("Processing file {}/{}", index + 1, total_files),
                    stats: None,
                },
            );

            let handler = Arc::new(TauriProgressHandler::new(
                app_handle.clone(),
                task_id.to_string(),
            ));
            handler.set_current_file(Some(file_path.clone()));

            let result = self.process_single_file(
                file_path,
                &config,
                handler,
                cancelled.clone(),
            );

            let file_result = match result {
                Ok(stats) => FileResult {
                    path: file_path.clone(),
                    success: true,
                    error: None,
                    stats: Some(FileStats::from(stats)),
                },
                Err(e) if matches!(e, GuiError::Cancelled) => {
                    self.update_status(task_id, TaskStatus::Cancelled)?;
                    return Err(e);
                }
                Err(e) => FileResult {
                    path: file_path.clone(),
                    success: false,
                    error: Some(e.to_string()),
                    stats: None,
                },
            };

            {
                let mut tasks = self.tasks.write();
                if let Some(task) = tasks.get_mut(task_id) {
                    task.results.push(file_result);
                    task.progress = ((index + 1) as f64 / total_files as f64) * 100.0;
                }
            }
        }

        self.update_status(task_id, TaskStatus::Completed)?;
        {
            let mut tasks = self.tasks.write();
            if let Some(task) = tasks.get_mut(task_id) {
                task.current_file = None;
            }
        }

        let (success_count, failure_count) = {
            let tasks = self.tasks.read();
            if let Some(task) = tasks.get(task_id) {
                let success = task.results.iter().filter(|r| r.success).count();
                let failed = task.results.len().saturating_sub(success);
                (success, failed)
            } else {
                (0usize, 0usize)
            }
        };

        let message = if failure_count == 0 {
            "All files processed successfully".to_string()
        } else {
            format!(
                "Task completed with errors: {} succeeded, {} failed",
                success_count, failure_count
            )
        };

        let _ = app_handle.emit(
            "task-progress",
            ProgressEvent {
                task_id: task_id.to_string(),
                phase: ProgressPhase::Completed,
                total_bytes: 0,
                processed_bytes: 0,
                current_file: None,
                message,
                stats: None,
            },
        );

        Ok(())
    }

    fn process_single_file(
        &self,
        file_path: &str,
        config: &TaskConfig,
        handler: Arc<TauriProgressHandler>,
        cancelled: Arc<AtomicBool>,
    ) -> Result<TaskStats> {
        let path = PathBuf::from(file_path);
        let mode: OperationMode = config.mode.into();

        let task = EncryptionTask::new(path, mode)
            .with_password(config.password.clone())
            .with_audio(config.encrypt_audio)
            .with_metadata_scrub(config.scrub_metadata)
            .with_no_wal(!config.use_wal)
            .with_handler(handler);

        if cancelled.load(Ordering::Relaxed) {
            return Err(GuiError::Cancelled);
        }

        let stats = media_lock_core::run_task_with_stats(&task)?;
        Ok(stats)
    }

    fn update_status(&self, task_id: &str, status: TaskStatus) -> Result<()> {
        let mut tasks = self.tasks.write();
        if let Some(task) = tasks.get_mut(task_id) {
            task.status = status;
        }
        Ok(())
    }
}

pub fn collect_media_files(path: &PathBuf, recursive: bool) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.clone()];
    }

    if !path.is_dir() {
        return vec![];
    }

    let walker = if recursive {
        WalkDir::new(path).follow_links(false)
    } else {
        WalkDir::new(path).max_depth(1).follow_links(false)
    };

    walker
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            let ext = e.path().extension().and_then(|s| s.to_str()).unwrap_or("");
            is_supported_extension(ext)
        })
        .map(|e| e.path().to_path_buf())
        .collect()
}

pub fn is_supported_extension(ext: &str) -> bool {
    matches!(
        ext.to_lowercase().as_str(),
        "mp4" | "m4v" | "mov" | "mkv" | "webm" | "m4a" | "mka"
    )
}

pub fn get_file_state(path: &PathBuf) -> std::io::Result<FileState> {
    if media_lock_core::io::StreamingWal::wal_path_for(path).exists() {
        return Ok(FileState::RecoveryNeeded);
    }

    if media_lock_core::io::LockManager::lock_path_for(path).exists() {
        return Ok(FileState::Locked);
    }

    use media_lock_core::common::{FileFooter, FOOTER_MAGIC};
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    let mut file = File::open(path)?;
    let len = file.metadata()?.len();

    let footer_size = FileFooter::SIZE as u64;
    if len < footer_size {
        return Ok(FileState::Normal);
    }

    file.seek(SeekFrom::End(-(footer_size as i64)))?;
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic)?;

    if magic == FOOTER_MAGIC {
        Ok(FileState::Encrypted)
    } else {
        Ok(FileState::Normal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileState {
    Normal,
    Encrypted,
    Locked,
    RecoveryNeeded,
}
