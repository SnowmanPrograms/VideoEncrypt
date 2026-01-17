//! Write-Ahead Logging (WAL) v2.0 for crash safety.
//!
//! Implements a streaming WAL that writes all backup data sequentially
//! before any modifications, enabling full recovery on crash.

use crate::common::Region;
use crate::error::{AppError, Result};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// WAL magic number v2: "WALV0002" in ASCII.
const WAL_MAGIC_V2: [u8; 8] = *b"WALV0002";

/// WAL entry for streaming write.
#[derive(Debug, Clone)]
struct WalEntry {
    offset: u64,
    data: Vec<u8>,
}

/// Streaming WAL writer.
///
/// Usage:
/// 1. `StreamingWal::create()` - Create new WAL file
/// 2. `append_region()` - Stream backup data for each region  
/// 3. `finish()` - Write CRC footer and sync
pub struct StreamingWal {
    writer: std::io::BufWriter<File>,
    entry_count: u32,
    total_bytes: u64,
}

impl StreamingWal {
    /// Get the WAL file path for a given target file.
    pub fn wal_path_for(target: &Path) -> PathBuf {
        let mut wal_path = target.to_path_buf();
        let file_name = wal_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        wal_path.set_file_name(format!("{}.wal", file_name));
        wal_path
    }

    /// Check if recovery is needed (WAL file exists and is valid).
    pub fn needs_recovery(target: &Path) -> bool {
        let wal_path = Self::wal_path_for(target);
        if let Ok(metadata) = wal_path.metadata() {
            // Must have at least header (8 magic + 4 count) + footer (4 CRC)
            metadata.len() > 16
        } else {
            false
        }
    }

    /// Create a new streaming WAL for the target file.
    pub fn create(target: &Path) -> Result<Self> {
        let wal_path = Self::wal_path_for(target);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&wal_path)?;

        // Use BufWriter to reduce syscalls (8MB buffer)
        let mut writer = std::io::BufWriter::with_capacity(8 * 1024 * 1024, file);

        // Write header: magic + placeholder for entry count (will be updated in finish)
        writer.write_all(&WAL_MAGIC_V2)?;
        writer.write_u32::<BigEndian>(0)?; // placeholder

        Ok(Self {
            writer,
            entry_count: 0,
            total_bytes: 0,
        })
    }

    /// Append a region's backup data to the WAL.
    ///
    /// Reads the original data from `source_file` and writes it to WAL.
    pub fn append_region(&mut self, source_file: &mut File, region: &Region) -> Result<()> {
        // Read original data from source file
        source_file.seek(SeekFrom::Start(region.offset))?;
        let mut data = vec![0u8; region.len];
        source_file.read_exact(&mut data)?;

        // Write entry: offset + length + data
        self.writer.write_u64::<BigEndian>(region.offset)?;
        self.writer.write_u32::<BigEndian>(region.len as u32)?;
        self.writer.write_all(&data)?;

        self.entry_count += 1;
        self.total_bytes += region.len as u64;

        Ok(())
    }

    /// Finish the WAL: update entry count, write CRC, and sync.
    ///
    /// This is the ONLY sync operation for Phase 1.
    pub fn finish(mut self) -> Result<()> {
        // Flush the buffer first
        self.writer.flush()?;
        
        // Get the underlying file for seeking
        let mut file = self.writer.into_inner().map_err(|e| e.into_error())?;
        
        // Update entry count at offset 8
        file.seek(SeekFrom::Start(8))?;
        file.write_u32::<BigEndian>(self.entry_count)?;
        
        // Calculate CRC over the complete file
        file.seek(SeekFrom::Start(0))?;
        let file_len = file.seek(SeekFrom::End(0))?;
        file.seek(SeekFrom::Start(0))?;
        
        let mut hasher = crc32fast::Hasher::new();
        let mut buf = vec![0u8; file_len as usize];
        file.read_exact(&mut buf)?;
        hasher.update(&buf);
        let crc = hasher.finalize();
        
        // Append CRC at the end
        file.seek(SeekFrom::End(0))?;
        file.write_u32::<BigEndian>(crc)?;

        // Critical: single sync for all WAL data
        file.sync_all()?;

        Ok(())
    }

    /// Get total bytes written to WAL.
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    /// Validate a WAL file and return entries if valid.
    fn read_and_validate(wal_path: &Path) -> Result<Vec<WalEntry>> {
        let mut file = File::open(wal_path)?;
        let file_len = file.seek(SeekFrom::End(0))?;
        file.seek(SeekFrom::Start(0))?;

        // Read and verify magic
        let mut magic = [0u8; 8];
        file.read_exact(&mut magic)?;
        if magic != WAL_MAGIC_V2 {
            return Err(AppError::WalChecksumError);
        }

        let entry_count = file.read_u32::<BigEndian>()? as usize;

        // Read all entries
        let mut entries = Vec::with_capacity(entry_count);
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&magic);
        hasher.update(&(entry_count as u32).to_be_bytes());

        for _ in 0..entry_count {
            let offset = file.read_u64::<BigEndian>()?;
            hasher.update(&offset.to_be_bytes());

            let len = file.read_u32::<BigEndian>()? as usize;
            hasher.update(&(len as u32).to_be_bytes());

            let mut data = vec![0u8; len];
            file.read_exact(&mut data)?;
            hasher.update(&data);

            entries.push(WalEntry { offset, data });
        }

        // Read and verify CRC
        let stored_crc = file.read_u32::<BigEndian>()?;
        let calculated_crc = hasher.finalize();

        if stored_crc != calculated_crc {
            return Err(AppError::WalChecksumError);
        }

        // Verify we're at the expected position
        let current_pos = file.stream_position()?;
        if current_pos != file_len {
            return Err(AppError::WalChecksumError);
        }

        Ok(entries)
    }

    /// Recover from a crash by restoring original data from the WAL.
    pub fn recover(target: &Path) -> Result<()> {
        let wal_path = Self::wal_path_for(target);

        if !wal_path.exists() {
            return Ok(());
        }

        // Try to validate WAL
        let entries = match Self::read_and_validate(&wal_path) {
            Ok(entries) => entries,
            Err(AppError::WalChecksumError) | Err(AppError::Io(_)) => {
                // WAL is incomplete/corrupted - assume original file is intact
                // Just delete the WAL and continue
                let _ = std::fs::remove_file(&wal_path);
                return Ok(());
            }
            Err(e) => return Err(e),
        };

        // WAL is valid - restore all regions
        let mut target_file = OpenOptions::new().write(true).open(target)?;

        for entry in entries {
            target_file.seek(SeekFrom::Start(entry.offset))?;
            target_file.write_all(&entry.data)?;
        }

        target_file.sync_all()?;

        // Cleanup WAL
        std::fs::remove_file(&wal_path)?;

        Ok(())
    }

    /// Delete WAL file if it exists.
    pub fn cleanup(target: &Path) -> Result<()> {
        let wal_path = Self::wal_path_for(target);
        if wal_path.exists() {
            std::fs::remove_file(&wal_path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::RegionKind;
    use tempfile::TempDir;

    #[test]
    fn test_streaming_wal_basic() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("test.mp4");

        // Create test file with known content
        let original_data = vec![0xAA; 100];
        std::fs::write(&target, &original_data).unwrap();

        // Create streaming WAL
        let mut source = OpenOptions::new().read(true).open(&target).unwrap();
        let mut wal = StreamingWal::create(&target).unwrap();

        let regions = vec![
            Region { offset: 0, len: 10, kind: RegionKind::VideoIFrame },
            Region { offset: 50, len: 20, kind: RegionKind::VideoIFrame },
        ];

        for region in &regions {
            wal.append_region(&mut source, region).unwrap();
        }
        wal.finish().unwrap();

        // Verify WAL file exists
        let wal_path = StreamingWal::wal_path_for(&target);
        assert!(wal_path.exists());

        // Verify WAL can be validated
        let entries = StreamingWal::read_and_validate(&wal_path).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].offset, 0);
        assert_eq!(entries[0].data.len(), 10);
        assert_eq!(entries[1].offset, 50);
        assert_eq!(entries[1].data.len(), 20);

        // Cleanup
        StreamingWal::cleanup(&target).unwrap();
        assert!(!wal_path.exists());
    }

    #[test]
    fn test_crash_recovery_phase2() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("test.bin");

        // Create test file
        let original_data = vec![0u8; 100];
        std::fs::write(&target, &original_data).unwrap();

        // Create WAL (simulating Phase 1 complete)
        let mut source = OpenOptions::new().read(true).open(&target).unwrap();
        let mut wal = StreamingWal::create(&target).unwrap();
        let region = Region { offset: 0, len: 10, kind: RegionKind::VideoIFrame };
        wal.append_region(&mut source, &region).unwrap();
        wal.finish().unwrap();

        // Simulate Phase 2 partial corruption
        let mut file = OpenOptions::new().write(true).open(&target).unwrap();
        file.write_all(&[0xFF; 10]).unwrap();
        file.sync_all().unwrap();
        drop(file);

        // Verify file is "corrupted"
        let corrupted = std::fs::read(&target).unwrap();
        assert_eq!(&corrupted[0..10], &[0xFF; 10]);

        // Recover should restore original data
        StreamingWal::recover(&target).unwrap();

        // Verify recovery
        let restored = std::fs::read(&target).unwrap();
        assert_eq!(&restored[0..10], &[0u8; 10]);
    }

    #[test]
    fn test_incomplete_wal_handling() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("test.bin");
        let wal_path = StreamingWal::wal_path_for(&target);

        // Create test file
        std::fs::write(&target, &[0u8; 100]).unwrap();

        // Write incomplete WAL (missing CRC)
        let mut wal_file = File::create(&wal_path).unwrap();
        wal_file.write_all(&WAL_MAGIC_V2).unwrap();
        wal_file.write_u32::<BigEndian>(1).unwrap(); // 1 entry
        wal_file.write_u64::<BigEndian>(0).unwrap(); // offset
        wal_file.write_u32::<BigEndian>(4).unwrap(); // length
        wal_file.write_all(&[0, 0, 0, 0]).unwrap(); // data
        // Missing CRC!
        drop(wal_file);

        // Recovery should detect incomplete WAL and delete it
        StreamingWal::recover(&target).unwrap();

        // WAL should be deleted
        assert!(!wal_path.exists());

        // Original file should be unchanged
        let data = std::fs::read(&target).unwrap();
        assert_eq!(data, vec![0u8; 100]);
    }
}
