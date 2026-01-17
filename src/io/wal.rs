//! Write-Ahead Logging (WAL) for crash safety.
//!
//! Implements a rolling batch WAL that backs up data before in-place modification.

use crate::common::Region;
use crate::error::{AppError, Result};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// WAL magic number: "WAL1" in big-endian.
const WAL_MAGIC: u32 = 0x57414C31;

/// WAL entry structure.
#[derive(Debug, Clone)]
pub struct WalEntry {
    /// Original file offset.
    pub offset: u64,
    /// Original data at that offset.
    pub data: Vec<u8>,
}

/// Write-Ahead Log manager.
///
/// Ensures data can be recovered if the process crashes during modification.
pub struct WalManager {
    /// Path to the WAL file.
    wal_path: PathBuf,
    /// Current batch entries (in memory).
    entries: Vec<WalEntry>,
}

impl WalManager {
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

    /// Check if recovery is needed (WAL file exists and has content).
    pub fn needs_recovery(target: &Path) -> bool {
        let wal_path = Self::wal_path_for(target);
        if let Ok(metadata) = wal_path.metadata() {
            metadata.len() > 0
        } else {
            false
        }
    }

    /// Create a new WAL manager for the target file.
    pub fn new(target: &Path) -> Self {
        Self {
            wal_path: Self::wal_path_for(target),
            entries: Vec::new(),
        }
    }

    /// Begin a new batch by reading original data from the file.
    ///
    /// This writes the original data to the WAL file before any modifications.
    pub fn begin_batch(&mut self, file: &mut File, regions: &[Region]) -> Result<()> {
        self.entries.clear();

        // Read original data from each region
        for region in regions {
            let mut data = vec![0u8; region.len];
            file.seek(SeekFrom::Start(region.offset))?;
            file.read_exact(&mut data)?;

            self.entries.push(WalEntry {
                offset: region.offset,
                data,
            });
        }

        // Write to WAL file
        self.write_wal()?;

        Ok(())
    }

    /// Write the current entries to the WAL file.
    fn write_wal(&self) -> Result<()> {
        let mut wal_file = File::create(&self.wal_path)?;

        // Write header
        wal_file.write_u32::<BigEndian>(WAL_MAGIC)?;
        wal_file.write_u32::<BigEndian>(self.entries.len() as u32)?;

        // Write entries
        for entry in &self.entries {
            wal_file.write_u64::<BigEndian>(entry.offset)?;
            wal_file.write_u32::<BigEndian>(entry.data.len() as u32)?;
            wal_file.write_all(&entry.data)?;
        }

        // Calculate and write CRC32
        let crc = self.calculate_crc();
        wal_file.write_u32::<BigEndian>(crc)?;

        // Critical: flush to disk
        wal_file.sync_all()?;

        Ok(())
    }

    /// Calculate CRC32 of the WAL content.
    fn calculate_crc(&self) -> u32 {
        let mut hasher = crc32fast::Hasher::new();

        // Hash the header
        hasher.update(&WAL_MAGIC.to_be_bytes());
        hasher.update(&(self.entries.len() as u32).to_be_bytes());

        // Hash entries
        for entry in &self.entries {
            hasher.update(&entry.offset.to_be_bytes());
            hasher.update(&(entry.data.len() as u32).to_be_bytes());
            hasher.update(&entry.data);
        }

        hasher.finalize()
    }

    /// Commit the batch (clear the WAL).
    ///
    /// Call this after the modified data has been successfully written to disk.
    pub fn commit_batch(&mut self) -> Result<()> {
        // Truncate the WAL file to mark completion
        let wal_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.wal_path)?;
        wal_file.sync_all()?;

        self.entries.clear();
        Ok(())
    }

    /// Recover from a crash by restoring original data from the WAL.
    pub fn recover(target: &Path) -> Result<()> {
        let wal_path = Self::wal_path_for(target);

        if !wal_path.exists() {
            return Ok(());
        }

        // Read and parse WAL
        let mut wal_file = File::open(&wal_path)?;
        let entries = Self::read_wal(&mut wal_file)?;

        // Restore original data
        let mut target_file = OpenOptions::new()
            .write(true)
            .open(target)?;

        for entry in entries {
            target_file.seek(SeekFrom::Start(entry.offset))?;
            target_file.write_all(&entry.data)?;
        }

        target_file.sync_all()?;

        // Clear WAL
        let wal_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&wal_path)?;
        wal_file.sync_all()?;

        Ok(())
    }

    /// Read and validate a WAL file.
    fn read_wal(file: &mut File) -> Result<Vec<WalEntry>> {
        // Read header
        let magic = file.read_u32::<BigEndian>()?;
        if magic != WAL_MAGIC {
            return Err(AppError::WalChecksumError);
        }

        let entry_count = file.read_u32::<BigEndian>()? as usize;
        let mut entries = Vec::with_capacity(entry_count);

        // Read entries
        for _ in 0..entry_count {
            let offset = file.read_u64::<BigEndian>()?;
            let data_len = file.read_u32::<BigEndian>()? as usize;
            let mut data = vec![0u8; data_len];
            file.read_exact(&mut data)?;

            entries.push(WalEntry { offset, data });
        }

        // Read and verify CRC
        let stored_crc = file.read_u32::<BigEndian>()?;

        // Recalculate CRC
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&WAL_MAGIC.to_be_bytes());
        hasher.update(&(entries.len() as u32).to_be_bytes());
        for entry in &entries {
            hasher.update(&entry.offset.to_be_bytes());
            hasher.update(&(entry.data.len() as u32).to_be_bytes());
            hasher.update(&entry.data);
        }
        let calculated_crc = hasher.finalize();

        if stored_crc != calculated_crc {
            // CRC mismatch means WAL was not fully written
            // In this case, we assume original file is intact
            return Err(AppError::WalChecksumError);
        }

        Ok(entries)
    }

    /// Get the data for the current batch (for processing).
    pub fn get_batch_data(&self) -> Vec<u8> {
        let total_len: usize = self.entries.iter().map(|e| e.data.len()).sum();
        let mut data = Vec::with_capacity(total_len);
        for entry in &self.entries {
            data.extend_from_slice(&entry.data);
        }
        data
    }

    /// Clean up the WAL file.
    pub fn cleanup(&self) -> Result<()> {
        if self.wal_path.exists() {
            std::fs::remove_file(&self.wal_path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_wal_format() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("test.mp4");
        
        // Create test file
        let mut file = File::create(&target).unwrap();
        file.write_all(&[0u8; 100]).unwrap();
        file.sync_all().unwrap();

        // Create WAL
        let mut file = OpenOptions::new().read(true).write(true).open(&target).unwrap();
        let mut wal = WalManager::new(&target);
        
        let regions = vec![
            Region { offset: 0, len: 10, kind: crate::common::RegionKind::VideoIFrame },
            Region { offset: 50, len: 20, kind: crate::common::RegionKind::VideoIFrame },
        ];

        wal.begin_batch(&mut file, &regions).unwrap();

        // Verify WAL file exists and has content
        let wal_path = WalManager::wal_path_for(&target);
        assert!(wal_path.exists());
        assert!(wal_path.metadata().unwrap().len() > 0);

        // Commit should clear WAL
        wal.commit_batch().unwrap();
        assert_eq!(wal_path.metadata().unwrap().len(), 0);
    }

    #[test]
    fn test_crash_simulation() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("test.bin");

        // Create test file with known content
        let original_data = vec![0u8; 4];
        std::fs::write(&target, &original_data).unwrap();

        // Begin batch (write WAL)
        let mut file = OpenOptions::new().read(true).write(true).open(&target).unwrap();
        let mut wal = WalManager::new(&target);
        
        let regions = vec![
            Region { offset: 0, len: 4, kind: crate::common::RegionKind::VideoIFrame },
        ];
        wal.begin_batch(&mut file, &regions).unwrap();

        // Modify file (simulating encryption)
        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(&[1, 1, 1, 1]).unwrap();
        file.sync_all().unwrap();
        drop(file);

        // Simulate crash - don't commit, just recover
        WalManager::recover(&target).unwrap();

        // Verify file is restored
        let restored_data = std::fs::read(&target).unwrap();
        assert_eq!(restored_data, original_data);
    }

    #[test]
    fn test_corrupted_wal() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("test.bin");
        let wal_path = WalManager::wal_path_for(&target);

        // Create test file
        std::fs::write(&target, &[0u8; 4]).unwrap();

        // Write a corrupted WAL (wrong CRC)
        let mut wal_file = File::create(&wal_path).unwrap();
        wal_file.write_u32::<BigEndian>(WAL_MAGIC).unwrap();
        wal_file.write_u32::<BigEndian>(1).unwrap(); // 1 entry
        wal_file.write_u64::<BigEndian>(0).unwrap(); // offset
        wal_file.write_u32::<BigEndian>(4).unwrap(); // length
        wal_file.write_all(&[0, 0, 0, 0]).unwrap(); // data
        wal_file.write_u32::<BigEndian>(0xDEADBEEF).unwrap(); // wrong CRC
        wal_file.sync_all().unwrap();
        drop(wal_file);

        // Recovery should fail with checksum error
        let result = WalManager::recover(&target);
        assert!(matches!(result, Err(AppError::WalChecksumError)));

        // Original file should be unchanged
        let data = std::fs::read(&target).unwrap();
        assert_eq!(data, vec![0u8; 4]);
    }
}
