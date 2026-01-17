//! # Media Lock Core Library
//!
//! In-place, high-performance, crash-safe media file encryption system.
//!
//! This library provides the core functionality for encrypting video files
//! (MP4, MKV) without creating temporary copies, using AES-256-CTR encryption
//! with Argon2id key derivation.

pub mod common;
pub mod crypto;
pub mod error;
pub mod i18n;
pub mod io;
pub mod parsers;
pub mod workflow;

// Re-export commonly used types
pub use common::{EncryptionConfig, EncryptionTask, OperationMode, ProgressHandler, Region, RegionKind};
pub use error::{AppError, Result};
pub use workflow::{TaskStats, run_task_with_stats};

