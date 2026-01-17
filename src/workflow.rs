//! Core workflow orchestration.
//!
//! This module implements the main encryption/decryption workflow.

use crate::common::{
    EncryptionTask, FileFooter, NoOpProgress, OperationMode, ProgressHandler,
    Region, FOOTER_MAGIC,
};
use crate::crypto::{derive_key, generate_nonce, generate_salt, CryptoEngine};
use crate::error::{AppError, Result};
use crate::io::{LockManager, ProcessStage, WalManager};
use crate::parsers::detect_parser;
use crate::t;

use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom, Write};

/// Default batch size for processing (16 MB).
const BATCH_SIZE: usize = 16 * 1024 * 1024;

/// Execute the encryption/decryption task.
pub fn run_task(task: &EncryptionTask) -> Result<()> {
    let path = &task.input_path;
    let handler: &dyn ProgressHandler = task
        .handler
        .as_ref()
        .map(|h| h.as_ref())
        .unwrap_or(&NoOpProgress);

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
        // After recovery, we're done if in Recover mode
        if task.config.operation == OperationMode::Recover {
            locker.release()?;
            handler.on_finish();
            return Ok(());
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

    let file_for_parsing = File::open(path)?;
    let mut reader = BufReader::new(file_for_parsing);
    let parser = detect_parser(path)?;
    let regions = parser.scan_regions(
        &mut reader,
        task.config.encrypt_audio,
        task.config.scrub_metadata,
    )?;

    if regions.is_empty() {
        // No regions to process
        handler.on_message("No regions found to process");
        locker.release()?;
        handler.on_finish();
        return Ok(());
    }

    // 4. Calculate total work
    let total_bytes: u64 = regions.iter().map(|r| r.len as u64).sum();
    handler.on_start(total_bytes, t!("status_processing"));

    // 5. Setup crypto
    let (salt, nonce, engine) = match task.config.operation {
        OperationMode::Encrypt => {
            let salt = generate_salt();
            let nonce = generate_nonce();
            let key = derive_key(password, &salt)?;
            (salt, nonce, CryptoEngine::new(key, nonce))
        }
        OperationMode::Decrypt => {
            // Read footer to get salt and nonce
            let footer = read_footer(&mut file)?;
            let key = derive_key(password, &footer.salt)?;
            (footer.salt, footer.nonce, CryptoEngine::new(key, footer.nonce))
        }
        OperationMode::Recover => {
            // Already handled above
            unreachable!()
        }
    };

    // 6. Batch processing
    let batches = chunk_regions(regions, BATCH_SIZE);
    let mut wal = WalManager::new(path);

    locker.update_stage(ProcessStage::Processing { current_offset: 0 })?;

    for batch in batches {
        // A. WAL Write (Critical)
        wal.begin_batch(&mut file, &batch)?;

        // B. Read & Process in RAM
        let mut data = wal.get_batch_data();
        engine.process_regions(&batch, &mut data, task.config.scrub_metadata);

        // C. Write Back
        write_batch_data(&mut file, &batch, &data)?;
        file.sync_all()?;

        // D. Commit
        wal.commit_batch()?;

        // E. Update progress
        let batch_bytes: u64 = batch.iter().map(|r| r.len as u64).sum();
        handler.on_progress(batch_bytes);
    }

    // 7. Finalize
    handler.on_message(t!("status_finalizing"));
    locker.update_stage(ProcessStage::Finalizing)?;

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
    wal.cleanup()?;
    locker.release()?;
    handler.on_finish();

    Ok(())
}

/// Detect if file is encrypted by checking for magic footer.
fn detect_file_state(file: &mut File) -> Result<bool> {
    let file_len = file.seek(SeekFrom::End(0))?;

    if file_len < FileFooter::SIZE as u64 {
        return Ok(false);
    }

    // Read last bytes for magic check
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

    // Generate a simple checksum (first 32 bytes sampled from file)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_regions() {
        use crate::common::RegionKind;

        let regions = vec![
            Region { offset: 0, len: 100, kind: RegionKind::VideoIFrame },
            Region { offset: 100, len: 200, kind: RegionKind::VideoIFrame },
            Region { offset: 300, len: 150, kind: RegionKind::VideoIFrame },
            Region { offset: 450, len: 50, kind: RegionKind::VideoIFrame },
        ];

        // Batch size of 250:
        // Batch 1: region 0 (100 bytes)
        // Batch 2: region 1 (200 bytes) - 100+200=300 > 250, new batch
        // Batch 3: region 2 + region 3 (150+50=200 bytes) - 200+150=350 > 250, new batch
        let batches = chunk_regions(regions.clone(), 250);
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].len(), 1); // First region (100 bytes)
        assert_eq!(batches[1].len(), 1); // Second region (200 bytes)
        assert_eq!(batches[2].len(), 2); // Third + Fourth regions (150+50=200 bytes)

        // Batch size of 1000 should create one batch
        let batches = chunk_regions(regions, 1000);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].len(), 4);
    }
}
