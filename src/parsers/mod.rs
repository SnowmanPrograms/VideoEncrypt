//! Media container parsers module.
//!
//! Provides parsers for MP4 (ISOBMFF) and MKV (EBML) container formats.

mod mkv;
mod mp4;

pub use mkv::MkvParser;
pub use mp4::Mp4Parser;

use crate::common::Region;
use crate::error::{AppError, Result};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// Trait for container format parsers.
pub trait ContainerParser {
    /// Scan the file and return regions to encrypt.
    ///
    /// # Arguments
    /// * `file` - Open file handle with read access.
    /// * `encrypt_audio` - Whether to include audio samples.
    /// * `scrub_metadata` - Whether to include metadata regions.
    fn scan_regions(
        &self,
        file: &mut BufReader<File>,
        encrypt_audio: bool,
        scrub_metadata: bool,
    ) -> Result<Vec<Region>>;

    /// Get the name of this parser.
    fn name(&self) -> &'static str;
}

/// Detect the container format and return the appropriate parser.
pub fn detect_parser(path: &Path) -> Result<Box<dyn ContainerParser>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    // Read first 12 bytes for detection
    let mut header = [0u8; 12];
    if reader.read_exact(&mut header).is_err() {
        return Err(AppError::UnsupportedFormat("File too short".to_string()));
    }

    // Reset to beginning
    reader.seek(SeekFrom::Start(0))?;

    // Check for MP4/MOV
    // MP4 files start with a box, commonly 'ftyp' at offset 4
    if &header[4..8] == b"ftyp" || &header[4..8] == b"moov" || &header[4..8] == b"mdat" {
        return Ok(Box::new(Mp4Parser::new()));
    }

    // Check for MKV/WebM (EBML header)
    // EBML documents start with 0x1A 0x45 0xDF 0xA3
    if header[0..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        return Ok(Box::new(MkvParser::new()));
    }

    // Try to detect by extension as fallback
    if let Some(ext) = path.extension() {
        let ext = ext.to_string_lossy().to_lowercase();
        match ext.as_str() {
            "mp4" | "m4v" | "m4a" | "mov" => return Ok(Box::new(Mp4Parser::new())),
            "mkv" | "webm" | "mka" => return Ok(Box::new(MkvParser::new())),
            _ => {}
        }
    }

    Err(AppError::UnsupportedFormat(format!(
        "Unknown format: {}",
        path.display()
    )))
}
