use crate::error::{GuiError, Result};
use crate::task_manager::{self, FileState, TaskConfig, TaskInfo, TaskManager};
use std::path::PathBuf;
use tauri::{command, AppHandle, State};
use tauri_plugin_dialog::FilePath;

#[command]
pub async fn select_files(
    app_handle: AppHandle,
    recursive: bool,
) -> Result<Vec<FileInfo>> {
    use tauri_plugin_dialog::DialogExt;

    let (tx, rx) = std::sync::mpsc::channel();

    app_handle
        .dialog()
        .file()
        .add_filter("Media Files", &["mp4", "m4v", "mov", "mkv", "webm", "m4a", "mka"])
        .pick_files(move |paths| {
            let _ = tx.send(paths);
        });

    let paths = rx.recv().map_err(|_| GuiError::Other("Dialog cancelled".into()))?;

    match paths {
        Some(paths) => {
            let mut files = Vec::new();
            for path in paths {
                let path_buf = file_path_to_path_buf(&path);
                if path_buf.is_dir() {
                    let collected = task_manager::collect_media_files(&path_buf, recursive);
                    for p in collected {
                        if let Ok(info) = get_file_info(&p) {
                            files.push(info);
                        }
                    }
                } else if let Ok(info) = get_file_info(&path_buf) {
                    files.push(info);
                }
            }
            Ok(files)
        }
        None => Err(GuiError::Other("No files selected".into())),
    }
}

#[command]
pub async fn select_folder(
    app_handle: AppHandle,
    recursive: bool,
) -> Result<Vec<FileInfo>> {
    use tauri_plugin_dialog::DialogExt;

    let (tx, rx) = std::sync::mpsc::channel();

    app_handle
        .dialog()
        .file()
        .pick_folder(move |folder| {
            let _ = tx.send(folder);
        });

    let folder = rx.recv().map_err(|_| GuiError::Other("Dialog cancelled".into()))?;

    match folder {
        Some(folder) => {
            let path_buf = file_path_to_path_buf(&folder);
            let collected = task_manager::collect_media_files(&path_buf, recursive);
            let mut files = Vec::new();
            for p in collected {
                if let Ok(info) = get_file_info(&p) {
                    files.push(info);
                }
            }
            Ok(files)
        }
        None => Err(GuiError::Other("No folder selected".into())),
    }
}

fn file_path_to_path_buf(fp: &FilePath) -> PathBuf {
    match fp {
        FilePath::Path(p) => p.clone(),
        FilePath::Url(url) => url.to_file_path().unwrap_or_default(),
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct FileInfo {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub state: FileState,
}

fn get_file_info(path: &PathBuf) -> Result<FileInfo> {
    let metadata = std::fs::metadata(path)?;
    let state = task_manager::get_file_state(path).unwrap_or(FileState::Normal);

    Ok(FileInfo {
        path: path.to_string_lossy().to_string(),
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        size: metadata.len(),
        state,
    })
}

#[command]
pub async fn start_task(
    app_handle: AppHandle,
    manager: State<'_, TaskManager>,
    config: TaskConfig,
) -> Result<String> {
    let task_id = manager.create_task(config)?;
    manager.inner().start_task(app_handle, task_id.clone()).await?;
    Ok(task_id)
}

#[command]
pub fn cancel_task(manager: State<'_, TaskManager>, task_id: String) -> Result<()> {
    manager.inner().cancel_task(&task_id)
}

#[command]
pub fn get_task_status(manager: State<'_, TaskManager>, task_id: String) -> Result<TaskInfo> {
    manager
        .inner()
        .get_task_info(&task_id)
        .ok_or(GuiError::TaskNotFound(task_id))
}

#[command]
pub fn get_supported_extensions() -> Vec<&'static str> {
    vec!["mp4", "m4v", "mov", "mkv", "webm", "m4a", "mka"]
}

#[command]
pub fn check_file_status(path: String) -> Result<FileState> {
    let path = PathBuf::from(path);
    task_manager::get_file_state(&path).map_err(GuiError::Io)
}

/// Accept file/folder paths from drag-and-drop and return FileInfo entries.
/// Files with unsupported extensions are silently ignored.
#[command]
pub fn add_dropped_files(paths: Vec<String>, recursive: bool) -> Result<Vec<FileInfo>> {
    let mut files = Vec::new();
    for p in paths {
        let path_buf = PathBuf::from(&p);
        if path_buf.is_dir() {
            let collected = task_manager::collect_media_files(&path_buf, recursive);
            for mp in collected {
                if let Ok(info) = get_file_info(&mp) {
                    files.push(info);
                }
            }
        } else if path_buf.is_file() {
            let ext = path_buf
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            let supported = ["mp4", "m4v", "mov", "mkv", "webm", "m4a", "mka"];
            if supported.contains(&ext.as_str()) {
                if let Ok(info) = get_file_info(&path_buf) {
                    files.push(info);
                }
            }
        }
    }
    Ok(files)
}
