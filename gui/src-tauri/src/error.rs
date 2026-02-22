use serde::{Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GuiError {
    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Task already running: {0}")]
    TaskAlreadyRunning(String),

    #[error("No files selected")]
    NoFilesSelected,

    #[error("Password cannot be empty")]
    EmptyPassword,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Core error: {0}")]
    Core(#[from] media_lock_core::AppError),

    #[error("Task cancelled")]
    Cancelled,

    #[error("{0}")]
    Other(String),
}

impl Serialize for GuiError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("GuiError", 2)?;
        s.serialize_field("kind", self.kind_str())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

impl GuiError {
    fn kind_str(&self) -> &'static str {
        match self {
            GuiError::TaskNotFound(_) => "task_not_found",
            GuiError::TaskAlreadyRunning(_) => "task_already_running",
            GuiError::NoFilesSelected => "no_files_selected",
            GuiError::EmptyPassword => "empty_password",
            GuiError::Io(_) => "io",
            GuiError::Core(_) => "core",
            GuiError::Cancelled => "cancelled",
            GuiError::Other(_) => "other",
        }
    }
}

pub type Result<T> = std::result::Result<T, GuiError>;
