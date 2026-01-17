//! Core workflow orchestration.
//!
//! This module implements the main encryption/decryption workflow.

use crate::common::{
    EncryptionTask, FileFooter, NoOpProgress, OperationMode, ProgressHandler,
    Region, RegionKind, FOOTER_MAGIC,
};
use crate::crypto::{derive_key, generate_nonce, generate_salt, CryptoEngine};
use crate::error::{AppError, Result};
use crate::io::{LockManager, ProcessStage, WalManager};
use crate::parsers::detect_parser;
use crate::t;

use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::time::{Duration, Instant};

/// Default batch size for processing (16 MB).
const BATCH_SIZE: usize = 16 * 1024 * 1024;

/// Task performance statistics.
#[derive(Debug, Clone, Default)]
pub struct TaskStats {
    /// Total file size in bytes.
    pub file_size: u64,
    /// Total bytes to encrypt/decrypt.
    pub data_size: u64,
    /// Number of I-frames found.
    pub iframe_count: usize,
    /// Number of audio samples found.
    pub audio_count: usize,
    /// Number of metadata regions found.
    pub metadata_count: usize,
    /// Time spent parsing the container.
    pub parse_time: Duration,
    /// Time spent on key derivation (Argon2).
    pub kdf_time: Duration,
    /// Time spent on I/O operations (read + write + sync).
    pub io_time: Duration,
    /// Time spent on encryption/decryption.
    pub crypto_time: Duration,
    /// Total elapsed time.
    pub total_time: Duration,
}

impl TaskStats {
    /// Calculate encryption throughput in MB/s.
    pub fn crypto_throughput_mbps(&self) -> f64 {
        if self.crypto_time.as_secs_f64() > 0.0 {
            (self.data_size as f64 / 1_000_000.0) / self.crypto_time.as_secs_f64()
        } else {
            0.0
        }
    }

    /// Calculate I/O throughput in MB/s.
    pub fn io_throughput_mbps(&self) -> f64 {
        if self.io_time.as_secs_f64() > 0.0 {
            (self.data_size as f64 / 1_000_000.0) / self.io_time.as_secs_f64()
        } else {
            0.0
        }
    }

    /// Calculate perceived speed (file size / total time) in MB/s.
    pub fn perceived_speed_mbps(&self) -> f64 {
        if self.total_time.as_secs_f64() > 0.0 {
            (self.file_size as f64 / 1_000_000.0) / self.total_time.as_secs_f64()
        } else {
            0.0
        }
    }

    /// Data ratio (encrypted data / file size) as percentage.
    pub fn data_ratio_percent(&self) -> f64 {
        if self.file_size > 0 {
            (self.data_size as f64 / self.file_size as f64) * 100.0
        } else {
            0.0
        }
    }
}

/// Execute the encryption/decryption task and return stats.
pub fn run_task(task: &EncryptionTask) -> Result<()> {
    let _ = run_task_with_stats(task)?;
    Ok(())
}

/// Execute the encryption/decryption task and return detailed stats.
pub fn run_task_with_stats(task: &EncryptionTask) -> Result<TaskStats> {
    let total_start = Instant::now();
    let mut stats = TaskStats::default();
    
    let path = &task.input_path;
    let handler: &dyn ProgressHandler = task
        .handler
        .as_ref()
        .map(|h| h.as_ref())
        .unwrap_or(&NoOpProgress);

    // Get file size
    stats.file_size = std::fs::metadata(path)?.len();

    // 1. Initial checks
    handler.on_message(t!("status_checking"));

    // Check 1: File lock
    let mut locker = LockManager::acquire(path, task.config.operation)?;

    // Check 2: Disaster recovery
    if WalManager::needs_recovery(path) {
        if task.config.operation != OperationMode::Recover {
            return Err(AppError::PreviousSessionFailed);
        }
        handler.on_message(t!("status_recovering"));
        WalManager::recover(path)?;
        if task.config.operation == OperationMode::Recover {
            locker.release()?;
            handler.on_finish();
            stats.total_time = total_start.elapsed();
            return Ok(stats);
        }
    }

    // Get password
    let password = task
        .config
        .password
        .as_ref()
        .ok_or(AppError::InvalidPassword)?;

    // 2. Open file and detect state
    let mut file = OpenOptions::new().read(true).write(true).open(path)?;

    // Check 3: Magic header check
    let file_state = detect_file_state(&mut file)?;
    validate_state(file_state, task.config.operation)?;

    // 3. Parse structure
    handler.on_message(t!("status_analyzing"));

    let parse_start = Instant::now();
    let file_for_parsing = File::open(path)?;
    let mut reader = BufReader::new(file_for_parsing);
    let parser = detect_parser(path)?;
    let regions = parser.scan_regions(
        &mut reader,
        task.config.encrypt_audio,
        task.config.scrub_metadata,
    )?;
    stats.parse_time = parse_start.elapsed();

    // Count region types
    for region in &regions {
        match region.kind {
            RegionKind::VideoIFrame => stats.iframe_count += 1,
            RegionKind::AudioSample => stats.audio_count += 1,
            RegionKind::Metadata => stats.metadata_count += 1,
        }
    }

    if regions.is_empty() {
        handler.on_message("No regions found to process");
        locker.release()?;
        handler.on_finish();
        stats.total_time = total_start.elapsed();
        return Ok(stats);
    }

    // 4. Calculate total work
    let total_bytes: u64 = regions.iter().map(|r| r.len as u64).sum();
    stats.data_size = total_bytes;
    handler.on_start(total_bytes, t!("status_processing"));

    // 5. Setup crypto (KDF)
    let kdf_start = Instant::now();
    let (salt, nonce, engine) = match task.config.operation {
        OperationMode::Encrypt => {
            let salt = generate_salt();
            let nonce = generate_nonce();
            let key = derive_key(password, &salt)?;
            (salt, nonce, CryptoEngine::new(key, nonce))
        }
        OperationMode::Decrypt => {
            let footer = read_footer(&mut file)?;
            let key = derive_key(password, &footer.salt)?;
            (footer.salt, footer.nonce, CryptoEngine::new(key, footer.nonce))
        }
        OperationMode::Recover => unreachable!(),
    };
    stats.kdf_time = kdf_start.elapsed();

    // 6. Batch processing
    let batches = chunk_regions(regions, BATCH_SIZE);
    let use_wal = !task.config.no_wal;
    let mut wal = if use_wal { Some(WalManager::new(path)) } else { None };

    locker.update_stage(ProcessStage::Processing { current_offset: 0 })?;

    for batch in batches {
        if use_wal {
            // Safe mode: Use WAL for crash recovery
            let wal_ref = wal.as_mut().unwrap();
            
            // A. WAL Write + Read (I/O)
            let io_start = Instant::now();
            wal_ref.begin_batch(&mut file, &batch)?;
            stats.io_time += io_start.elapsed();

            // B. Encrypt in RAM (Crypto)
            let crypto_start = Instant::now();
            let mut data = wal_ref.get_batch_data();
            engine.process_regions(&batch, &mut data, task.config.scrub_metadata);
            stats.crypto_time += crypto_start.elapsed();

            // C. Write Back + Sync (I/O)
            let io_start = Instant::now();
            write_batch_data(&mut file, &batch, &data)?;
            file.sync_all()?;
            stats.io_time += io_start.elapsed();

            // D. Commit (I/O)
            let io_start = Instant::now();
            wal_ref.commit_batch()?;
            stats.io_time += io_start.elapsed();
        } else {
            // Fast mode: Direct read-process-write without WAL
            // A. Read (I/O)
            let io_start = Instant::now();
            let mut data = read_batch_data(&mut file, &batch)?;
            stats.io_time += io_start.elapsed();

            // B. Encrypt in RAM (Crypto)
            let crypto_start = Instant::now();
            engine.process_regions(&batch, &mut data, task.config.scrub_metadata);
            stats.crypto_time += crypto_start.elapsed();

            // C. Write Back (I/O) - no sync per batch
            let io_start = Instant::now();
            write_batch_data(&mut file, &batch, &data)?;
            stats.io_time += io_start.elapsed();
        }

        // E. Update progress
        let batch_bytes: u64 = batch.iter().map(|r| r.len as u64).sum();
        handler.on_progress(batch_bytes);
    }

    // 7. Finalize
    handler.on_message(t!("status_finalizing"));
    locker.update_stage(ProcessStage::Finalizing)?;

    // Final sync for no-wal mode
    if !use_wal {
        file.sync_all()?;
    }

    match task.config.operation {
        OperationMode::Encrypt => {
            append_footer(&mut file, salt, nonce)?;
        }
        OperationMode::Decrypt => {
            remove_footer(&mut file)?;
        }
        OperationMode::Recover => {}
    }

    // 8. Release lock
    if let Some(ref w) = wal {
        w.cleanup()?;
    }
    locker.release()?;
    
    stats.total_time = total_start.elapsed();
    handler.on_finish();

    Ok(stats)
}

/// Detect if file is encrypted by checking for magic footer.
fn detect_file_state(file: &mut File) -> Result<bool> {
    let file_len = file.seek(SeekFrom::End(0))?;

    if file_len < FileFooter::SIZE as u64 {
        return Ok(false);
    }

    file.seek(SeekFrom::End(-(FileFooter::SIZE as i64)))?;
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic)?;

    file.seek(SeekFrom::Start(0))?;

    Ok(magic == FOOTER_MAGIC)
}

/// Validate that the operation matches the file state.
fn validate_state(is_encrypted: bool, operation: OperationMode) -> Result<()> {
    match (is_encrypted, operation) {
        (true, OperationMode::Encrypt) => Err(AppError::AlreadyEncrypted),
        (false, OperationMode::Decrypt) => Err(AppError::NotEncrypted),
        _ => Ok(()),
    }
}

/// Read the footer from an encrypted file.
fn read_footer(file: &mut File) -> Result<FileFooter> {
    file.seek(SeekFrom::End(-(FileFooter::SIZE as i64)))?;
    let mut footer_bytes = [0u8; FileFooter::SIZE];
    file.read_exact(&mut footer_bytes)?;
    FileFooter::from_bytes(&footer_bytes)
}

/// Append the encryption footer to the file.
fn append_footer(file: &mut File, salt: [u8; 16], nonce: [u8; 8]) -> Result<()> {
    let original_len = file.seek(SeekFrom::End(0))?;

    let mut checksum = [0u8; 32];
    file.seek(SeekFrom::Start(0))?;
    let _ = file.read(&mut checksum);

    let footer = FileFooter::new(salt, nonce, original_len, checksum);
    let footer_bytes = footer.to_bytes();

    file.seek(SeekFrom::End(0))?;
    file.write_all(&footer_bytes)?;
    file.sync_all()?;

    Ok(())
}

/// Remove the encryption footer from the file.
fn remove_footer(file: &mut File) -> Result<()> {
    let footer = read_footer(file)?;
    file.set_len(footer.original_len)?;
    file.sync_all()?;
    Ok(())
}

/// Split regions into batches of approximately the given size.
fn chunk_regions(regions: Vec<Region>, max_batch_size: usize) -> Vec<Vec<Region>> {
    let mut batches = Vec::new();
    let mut current_batch = Vec::new();
    let mut current_size = 0usize;

    for region in regions {
        if current_size + region.len > max_batch_size && !current_batch.is_empty() {
            batches.push(current_batch);
            current_batch = Vec::new();
            current_size = 0;
        }

        current_size += region.len;
        current_batch.push(region);
    }

    if !current_batch.is_empty() {
        batches.push(current_batch);
    }

    batches
}

/// Write processed data back to the file.
fn write_batch_data(file: &mut File, regions: &[Region], data: &[u8]) -> Result<()> {
    let mut data_offset = 0usize;

    for region in regions {
        file.seek(SeekFrom::Start(region.offset))?;
        file.write_all(&data[data_offset..data_offset + region.len])?;
        data_offset += region.len;
    }

    Ok(())
}

/// Read batch data from file (for no-WAL mode).
fn read_batch_data(file: &mut File, regions: &[Region]) -> Result<Vec<u8>> {
    let total_len: usize = regions.iter().map(|r| r.len).sum();
    let mut data = Vec::with_capacity(total_len);
    
    for region in regions {
        file.seek(SeekFrom::Start(region.offset))?;
        let mut buf = vec![0u8; region.len];
        file.read_exact(&mut buf)?;
        data.extend_from_slice(&buf);
    }
    
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_regions() {
        let regions = vec![
            Region { offset: 0, len: 100, kind: RegionKind::VideoIFrame },
            Region { offset: 100, len: 200, kind: RegionKind::VideoIFrame },
            Region { offset: 300, len: 150, kind: RegionKind::VideoIFrame },
            Region { offset: 450, len: 50, kind: RegionKind::VideoIFrame },
        ];

        let batches = chunk_regions(regions.clone(), 250);
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].len(), 1);
        assert_eq!(batches[1].len(), 1);
        assert_eq!(batches[2].len(), 2);

        let batches = chunk_regions(regions, 1000);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].len(), 4);
    }
}
