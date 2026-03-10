//! Core domain models and interfaces.
//!
//! This module defines the fundamental data structures and traits used
//! throughout the library.

use crate::error::{AppError, Result};
use std::path::PathBuf;
use std::sync::Arc;

// ============================================================================
// Region Types
// ============================================================================

/// Kind of region to process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionKind {
    /// Video I-Frame (keyframe)
    VideoIFrame,
    /// Audio sample
    AudioSample,
    /// Metadata (title, GPS, etc.)
    Metadata,
}

/// Describes a region in the file to process.
#[derive(Debug, Clone)]
pub struct Region {
    /// Byte offset from the start of the file.
    pub offset: u64,
    /// Length of the region in bytes.
    pub len: usize,
    /// Type of content in this region.
    pub kind: RegionKind,
}

// ============================================================================
// File Footer (On-Disk Format)
// ============================================================================

/// Magic number for encrypted files: "RUST_ENC"
pub const FOOTER_MAGIC: [u8; 8] = *b"RUST_ENC";

/// Current footer version.
pub const FOOTER_VERSION: u8 = 1;

/// File footer structure stored at the end of encrypted files.
/// Total size: 8 + 1 + 16 + 8 + 8 + 32 = 73 bytes (aligned to 80 for safety)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FileFooter {
    /// Magic number: "RUST_ENC"
    pub magic: [u8; 8],
    /// Footer format version.
    pub version: u8,
    /// Salt used for key derivation.
    pub salt: [u8; 16],
    /// Nonce for AES-CTR (8 bytes for large file support).
    pub nonce: [u8; 8],
    /// Original file length before footer was appended.
    pub original_len: u64,
    /// Checksum of sampled original data for verification.
    pub checksum: [u8; 32],
}

impl FileFooter {
    /// Size of the footer in bytes.
    pub const SIZE: usize = 73;

    /// Create a new footer with the given parameters.
    pub fn new(salt: [u8; 16], nonce: [u8; 8], original_len: u64, checksum: [u8; 32]) -> Self {
        Self {
            magic: FOOTER_MAGIC,
            version: FOOTER_VERSION,
            salt,
            nonce,
            original_len,
            checksum,
        }
    }

    /// Serialize the footer to bytes.
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0..8].copy_from_slice(&self.magic);
        bytes[8] = self.version;
        bytes[9..25].copy_from_slice(&self.salt);
        bytes[25..33].copy_from_slice(&self.nonce);
        bytes[33..41].copy_from_slice(&self.original_len.to_be_bytes());
        bytes[41..73].copy_from_slice(&self.checksum);
        bytes
    }

    /// Deserialize footer from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < Self::SIZE {
            return Err(AppError::InvalidStructure("Footer too short".to_string()));
        }

        let mut magic = [0u8; 8];
        magic.copy_from_slice(&bytes[0..8]);

        if magic != FOOTER_MAGIC {
            return Err(AppError::NotEncrypted);
        }

        let version = bytes[8];
        let mut salt = [0u8; 16];
        salt.copy_from_slice(&bytes[9..25]);
        let mut nonce = [0u8; 8];
        nonce.copy_from_slice(&bytes[25..33]);
        let original_len = u64::from_be_bytes(bytes[33..41].try_into().unwrap());
        let mut checksum = [0u8; 32];
        checksum.copy_from_slice(&bytes[41..73]);

        Ok(Self {
            magic,
            version,
            salt,
            nonce,
            original_len,
            checksum,
        })
    }
}

// ============================================================================
// Configuration Types
// ============================================================================

/// Operation mode for the encryption task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationMode {
    /// Encrypt the file.
    Encrypt,
    /// Decrypt the file.
    Decrypt,
    /// Recover from a crashed session.
    Recover,
}

/// Configuration for an encryption/decryption task.
#[derive(Debug, Clone)]
pub struct EncryptionConfig {
    /// Password for encryption/decryption. If None, must be provided interactively.
    pub password: Option<String>,
    /// Whether to encrypt audio tracks.
    pub encrypt_audio: bool,
    /// Whether to scrub sensitive metadata.
    pub scrub_metadata: bool,
    /// Operation mode.
    pub operation: OperationMode,
    /// Disable WAL for faster (but unsafe) operation.
    pub no_wal: bool,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            password: None,
            encrypt_audio: false,
            scrub_metadata: false,
            operation: OperationMode::Encrypt,
            no_wal: false,
        }
    }
}

// ============================================================================
// Progress Handler Trait
// ============================================================================

/// Callback interface for progress updates.
///
/// Implementations must be thread-safe (`Send + Sync`) to allow
/// the core library to run in background threads.
pub trait ProgressHandler: Send + Sync {
    /// Called when a task starts.
    ///
    /// # Arguments
    /// * `total_bytes` - Total bytes to process (for percentage calculation).
    /// * `message` - Description of current phase (may be i18n key).
    fn on_start(&self, total_bytes: u64, message: &str);

    /// Called with incremental progress updates.
    ///
    /// # Arguments
    /// * `delta_bytes` - Bytes processed in this batch.
    fn on_progress(&self, delta_bytes: u64);

    /// Called when the current phase changes.
    ///
    /// # Arguments
    /// * `message` - Description of the new phase.
    fn on_message(&self, message: &str);

    /// Called when the task completes successfully.
    fn on_finish(&self);

    /// Called when a non-fatal error occurs.
    ///
    /// # Arguments
    /// * `err` - The error that occurred.
    fn on_error(&self, err: &AppError);
}

/// No-op progress handler for headless/testing scenarios.
pub struct NoOpProgress;

impl ProgressHandler for NoOpProgress {
    fn on_start(&self, _total_bytes: u64, _message: &str) {}
    fn on_progress(&self, _delta_bytes: u64) {}
    fn on_message(&self, _message: &str) {}
    fn on_finish(&self) {}
    fn on_error(&self, _err: &AppError) {}
}

// ============================================================================
// Encryption Task Builder
// ============================================================================

/// Main entry point for the library.
///
/// Use the builder pattern to configure and execute encryption tasks.
///
/// # Example
/// ```ignore
/// use media_lock_core::{EncryptionTask, OperationMode};
///
/// let task = EncryptionTask::new("video.mp4".into(), OperationMode::Encrypt)
///     .with_password("secret".to_string())
///     .with_metadata_scrub(true);
///
/// task.run()?;
/// ```
pub struct EncryptionTask {
    /// Path to the input file.
    pub input_path: PathBuf,
    /// Task configuration.
    pub config: EncryptionConfig,
    /// Optional progress handler for UI updates.
    pub handler: Option<Arc<dyn ProgressHandler>>,
}

impl EncryptionTask {
    /// Create a new encryption task.
    pub fn new(path: PathBuf, mode: OperationMode) -> Self {
        Self {
            input_path: path,
            config: EncryptionConfig {
                operation: mode,
                ..Default::default()
            },
            handler: None,
        }
    }

    /// Set the password for encryption/decryption.
    pub fn with_password(mut self, pwd: String) -> Self {
        self.config.password = Some(pwd);
        self
    }

    /// Set whether to encrypt audio tracks.
    pub fn with_audio(mut self, enable: bool) -> Self {
        self.config.encrypt_audio = enable;
        self
    }

    /// Set whether to scrub metadata.
    pub fn with_metadata_scrub(mut self, enable: bool) -> Self {
        self.config.scrub_metadata = enable;
        self
    }

    /// Set the progress handler.
    pub fn with_handler(mut self, handler: Arc<dyn ProgressHandler>) -> Self {
        self.handler = Some(handler);
        self
    }

    /// Disable WAL for faster but unsafe operation.
    pub fn with_no_wal(mut self, enable: bool) -> Self {
        self.config.no_wal = enable;
        self
    }

    /// Execute the encryption/decryption task.
    ///
    /// This is a blocking call that runs the entire workflow.
    pub fn run(self) -> Result<()> {
        crate::workflow::run_task(&self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_footer_roundtrip() {
        let salt = [0x11u8; 16];
        let nonce = [0x22u8; 8];
        let original_len = 123_456_789u64;
        let checksum = [0x33u8; 32];

        let footer = FileFooter::new(salt, nonce, original_len, checksum);
        let bytes = footer.to_bytes();
        let parsed = FileFooter::from_bytes(&bytes).unwrap();

        assert_eq!(parsed.magic, FOOTER_MAGIC);
        assert_eq!(parsed.version, FOOTER_VERSION);
        assert_eq!(parsed.salt, salt);
        assert_eq!(parsed.nonce, nonce);
        assert_eq!(parsed.original_len, original_len);
        assert_eq!(parsed.checksum, checksum);
    }

    #[test]
    fn test_file_footer_rejects_short_buffer() {
        let err = FileFooter::from_bytes(&[0u8; FileFooter::SIZE - 1]).unwrap_err();
        assert!(matches!(err, AppError::InvalidStructure(msg) if msg.contains("Footer too short")));
    }

    #[test]
    fn test_file_footer_rejects_bad_magic() {
        let mut bytes = [0u8; FileFooter::SIZE];
        bytes[0..8].copy_from_slice(b"NOT_ENC!");
        bytes[8] = FOOTER_VERSION;

        let err = FileFooter::from_bytes(&bytes).unwrap_err();
        assert!(matches!(err, AppError::NotEncrypted));
    }
}
