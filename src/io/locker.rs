//! File locking and state management.

use crate::common::OperationMode;
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Processing stage for tracking progress.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ProcessStage {
    /// Just started, initializing.
    Initializing,
    /// Currently processing data.
    Processing {
        /// Current byte offset being processed.
        current_offset: u64,
    },
    /// Writing final header/footer.
    Finalizing,
}

/// Lock file state, stored as JSON.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LockState {
    /// Unique session identifier.
    pub session_id: String,
    /// Absolute path to the target file.
    pub target_file: PathBuf,
    /// Current operation mode.
    pub operation: OperationMode,
    /// Unix timestamp when the lock was created.
    pub timestamp: u64,
    /// Current processing stage.
    pub stage: ProcessStage,
}

// Implement Serialize/Deserialize for OperationMode
impl Serialize for OperationMode {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            OperationMode::Encrypt => serializer.serialize_str("encrypt"),
            OperationMode::Decrypt => serializer.serialize_str("decrypt"),
            OperationMode::Recover => serializer.serialize_str("recover"),
        }
    }
}

impl<'de> Deserialize<'de> for OperationMode {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "encrypt" => Ok(OperationMode::Encrypt),
            "decrypt" => Ok(OperationMode::Decrypt),
            "recover" => Ok(OperationMode::Recover),
            _ => Err(serde::de::Error::custom("Invalid operation mode")),
        }
    }
}

/// File lock manager.
///
/// Ensures only one process can modify a file at a time.
pub struct LockManager {
    /// Path to the lock file.
    lock_path: PathBuf,
    /// Current lock state.
    state: LockState,
}

impl LockManager {
    /// Get the lock file path for a given target file.
    pub fn lock_path_for(target: &Path) -> PathBuf {
        let mut lock_path = target.to_path_buf();
        let file_name = lock_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        lock_path.set_file_name(format!("{}.lock", file_name));
        lock_path
    }

    /// Check if a lock file exists for the target.
    pub fn is_locked(target: &Path) -> bool {
        Self::lock_path_for(target).exists()
    }

    /// Read existing lock state if present.
    pub fn read_lock(target: &Path) -> Result<Option<LockState>> {
        let lock_path = Self::lock_path_for(target);
        if !lock_path.exists() {
            return Ok(None);
        }

        let mut file = File::open(&lock_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let state: LockState = serde_json::from_str(&contents)?;
        Ok(Some(state))
    }

    /// Acquire a lock on the target file.
    ///
    /// Returns an error if the file is already locked.
    pub fn acquire(target: &Path, operation: OperationMode) -> Result<Self> {
        let lock_path = Self::lock_path_for(target);

        // Check for existing lock
        if lock_path.exists() {
            return Err(AppError::FileLocked(lock_path.display().to_string()));
        }

        // Create new lock state
        let state = LockState {
            session_id: Uuid::new_v4().to_string(),
            target_file: target.canonicalize().unwrap_or_else(|_| target.to_path_buf()),
            operation,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            stage: ProcessStage::Initializing,
        };

        // Write lock file
        let mut file = File::create(&lock_path)?;
        let json = serde_json::to_string_pretty(&state)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;

        Ok(Self { lock_path, state })
    }

    /// Update the processing stage.
    pub fn update_stage(&mut self, stage: ProcessStage) -> Result<()> {
        self.state.stage = stage;
        self.write_state()
    }

    /// Write current state to disk.
    fn write_state(&self) -> Result<()> {
        let mut file = File::create(&self.lock_path)?;
        let json = serde_json::to_string_pretty(&self.state)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        Ok(())
    }

    /// Release the lock.
    pub fn release(self) -> Result<()> {
        if self.lock_path.exists() {
            fs::remove_file(&self.lock_path)?;
        }
        Ok(())
    }

    /// Get the current lock state.
    pub fn state(&self) -> &LockState {
        &self.state
    }
}

impl Drop for LockManager {
    fn drop(&mut self) {
        // Note: We don't auto-release on drop because we want explicit release
        // for crash safety. The lock file should persist if the process crashes.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_lock_acquire_release() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("test.mp4");
        fs::write(&target, b"test content").unwrap();

        // Acquire lock
        let lock = LockManager::acquire(&target, OperationMode::Encrypt).unwrap();
        assert!(LockManager::is_locked(&target));

        // Try to acquire again should fail
        let result = LockManager::acquire(&target, OperationMode::Encrypt);
        assert!(result.is_err());

        // Release lock
        lock.release().unwrap();
        assert!(!LockManager::is_locked(&target));
    }

    #[test]
    fn test_lock_state_serialization() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("test.mp4");
        fs::write(&target, b"test content").unwrap();

        // Acquire and update stage
        let mut lock = LockManager::acquire(&target, OperationMode::Encrypt).unwrap();
        lock.update_stage(ProcessStage::Processing { current_offset: 1024 }).unwrap();

        // Read back the lock state
        let state = LockManager::read_lock(&target).unwrap().unwrap();
        assert_eq!(state.session_id, lock.state().session_id);
        match state.stage {
            ProcessStage::Processing { current_offset } => {
                assert_eq!(current_offset, 1024);
            }
            _ => panic!("Wrong stage"),
        }

        lock.release().unwrap();
    }
}
