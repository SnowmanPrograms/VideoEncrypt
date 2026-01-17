//! IO safety module.
//!
//! Provides file locking and Write-Ahead Logging (WAL) for crash safety.

mod locker;
mod wal;

pub use locker::{LockManager, LockState, ProcessStage};
pub use wal::StreamingWal;

