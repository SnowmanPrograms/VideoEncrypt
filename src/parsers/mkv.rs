//! MKV (EBML) container parser.
//!
//! Implements zero-DOM parsing of MKV/WebM files to extract I-frame locations.

use super::ContainerParser;
use crate::common::{Region, RegionKind};
use crate::error::{AppError, Result};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};

/// MKV parser implementation.
pub struct MkvParser;

impl MkvParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MkvParser {
    fn default() -> Self {
        Self::new()
    }
}

// EBML Element IDs
const EBML_HEADER_ID: u32 = 0x1A45DFA3;
const SEGMENT_ID: u32 = 0x18538067;
const TRACKS_ID: u32 = 0x1654AE6B;
const TRACK_ENTRY_ID: u32 = 0xAE;
const TRACK_NUMBER_ID: u32 = 0xD7;
const TRACK_TYPE_ID: u32 = 0x83;
const CLUSTER_ID: u32 = 0x1F43B675;
const SIMPLE_BLOCK_ID: u32 = 0xA3;
const BLOCK_GROUP_ID: u32 = 0xA0;
const BLOCK_ID: u32 = 0xA1;

/// Track types in MKV.
const TRACK_TYPE_VIDEO: u8 = 1;
const TRACK_TYPE_AUDIO: u8 = 2;

/// Information about a track.
#[derive(Debug, Clone)]
struct TrackInfo {
    number: u64,
    track_type: u8,
}

impl ContainerParser for MkvParser {
    fn name(&self) -> &'static str {
        "MKV"
    }

    fn scan_regions(
        &self,
        reader: &mut BufReader<File>,
        encrypt_audio: bool,
        _scrub_metadata: bool,
    ) -> Result<Vec<Region>> {
        let mut regions = Vec::new();
        let file_size = get_file_size(reader)?;

        reader.seek(SeekFrom::Start(0))?;

        // Verify EBML header
        let (id, _size) = read_ebml_element_header(reader)?;
        if id != EBML_HEADER_ID {
            return Err(AppError::InvalidStructure("Not a valid EBML file".to_string()));
        }

        // Find Segment
        let segment_start = find_element_after(reader, SEGMENT_ID, file_size)?
            .ok_or_else(|| AppError::InvalidStructure("No Segment found".to_string()))?;

        // Parse Tracks to identify video/audio track numbers
        let tracks = if let Some(tracks_pos) = find_element_in_segment(reader, TRACKS_ID, segment_start, file_size)? {
            parse_tracks(reader, tracks_pos)?
        } else {
            Vec::new()
        };

        let video_tracks: Vec<u64> = tracks
            .iter()
            .filter(|t| t.track_type == TRACK_TYPE_VIDEO)
            .map(|t| t.number)
            .collect();

        let audio_tracks: Vec<u64> = tracks
            .iter()
            .filter(|t| t.track_type == TRACK_TYPE_AUDIO)
            .map(|t| t.number)
            .collect();

        // Scan clusters for SimpleBlocks
        reader.seek(SeekFrom::Start(segment_start))?;

        while reader.stream_position()? < file_size {
            let pos = reader.stream_position()?;

            match read_ebml_element_header(reader) {
                Ok((id, size)) => {
                    if id == CLUSTER_ID {
                        // Parse cluster for blocks
                        let cluster_end = reader.stream_position()? + size;
                        scan_cluster(
                            reader,
                            cluster_end,
                            &video_tracks,
                            &audio_tracks,
                            encrypt_audio,
                            &mut regions,
                        )?;
                    } else {
                        // Skip this element
                        let next_pos = reader.stream_position()? + size;
                        if next_pos > file_size {
                            break;
                        }
                        reader.seek(SeekFrom::Start(next_pos))?;
                    }
                }
                Err(_) => break,
            }

            // Safety check to prevent infinite loop
            if reader.stream_position()? <= pos {
                break;
            }
        }

        // Sort by offset
        regions.sort_by_key(|r| r.offset);

        Ok(regions)
    }
}

/// Get file size.
fn get_file_size(reader: &mut BufReader<File>) -> Result<u64> {
    let current = reader.stream_position()?;
    let size = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(current))?;
    Ok(size)
}

/// Read an EBML variable-size integer (VINT).
fn read_vint(reader: &mut impl Read) -> Result<u64> {
    let mut first = [0u8; 1];
    reader.read_exact(&mut first)?;

    let first_byte = first[0];
    if first_byte == 0 {
        return Err(AppError::InvalidStructure("Invalid VINT: zero first byte".to_string()));
    }

    // Count leading zeros to determine length
    let len = first_byte.leading_zeros() as usize + 1;
    if len > 8 {
        return Err(AppError::InvalidStructure("Invalid VINT length".to_string()));
    }

    // Mask to extract value bits from first byte
    let mask = (1u8 << (8 - len)) - 1;
    let mut value = (first_byte & mask) as u64;

    // Read remaining bytes
    for _ in 1..len {
        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte)?;
        value = (value << 8) | byte[0] as u64;
    }

    Ok(value)
}

/// Read an EBML element ID (variable-length).
fn read_element_id(reader: &mut impl Read) -> Result<u32> {
    let mut first = [0u8; 1];
    reader.read_exact(&mut first)?;

    let first_byte = first[0];
    if first_byte == 0 {
        return Err(AppError::InvalidStructure("Invalid element ID".to_string()));
    }

    // Count leading zeros to determine length
    let len = first_byte.leading_zeros() as usize + 1;
    if len > 4 {
        return Err(AppError::InvalidStructure("Element ID too long".to_string()));
    }

    let mut value = first_byte as u32;

    for _ in 1..len {
        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte)?;
        value = (value << 8) | byte[0] as u32;
    }

    Ok(value)
}

/// Read an EBML element header (ID + Size).
fn read_ebml_element_header(reader: &mut BufReader<File>) -> Result<(u32, u64)> {
    let id = read_element_id(reader)?;
    let size = read_vint(reader)?;
    Ok((id, size))
}

/// Find an element starting after current position.
fn find_element_after(reader: &mut BufReader<File>, target_id: u32, limit: u64) -> Result<Option<u64>> {
    while reader.stream_position()? < limit {
        let pos = reader.stream_position()?;

        match read_ebml_element_header(reader) {
            Ok((id, size)) => {
                if id == target_id {
                    return Ok(Some(reader.stream_position()?));
                }
                let next = reader.stream_position()? + size;
                if next > limit {
                    break;
                }
                reader.seek(SeekFrom::Start(next))?;
            }
            Err(_) => break,
        }

        if reader.stream_position()? <= pos {
            break;
        }
    }
    Ok(None)
}

/// Find an element within the segment.
fn find_element_in_segment(
    reader: &mut BufReader<File>,
    target_id: u32,
    segment_start: u64,
    file_size: u64,
) -> Result<Option<u64>> {
    reader.seek(SeekFrom::Start(segment_start))?;
    find_element_after(reader, target_id, file_size)
}

/// Parse the Tracks element to get track information.
fn parse_tracks(reader: &mut BufReader<File>, tracks_start: u64) -> Result<Vec<TrackInfo>> {
    let mut tracks = Vec::new();

    // Read Tracks element size
    reader.seek(SeekFrom::Start(tracks_start))?;

    // We're already past the header, need to figure out the end
    // For simplicity, we'll scan for TrackEntry elements
    let file_size = get_file_size(reader)?;

    while reader.stream_position()? < file_size {
        let pos = reader.stream_position()?;

        match read_ebml_element_header(reader) {
            Ok((id, size)) => {
                if id == TRACK_ENTRY_ID {
                    if let Some(track) = parse_track_entry(reader, size)? {
                        tracks.push(track);
                    }
                } else if id == CLUSTER_ID || id == SEGMENT_ID {
                    // We've gone past the Tracks section
                    break;
                } else {
                    let next = reader.stream_position()? + size;
                    if next > file_size {
                        break;
                    }
                    reader.seek(SeekFrom::Start(next))?;
                }
            }
            Err(_) => break,
        }

        if reader.stream_position()? <= pos {
            break;
        }
    }

    Ok(tracks)
}

/// Parse a TrackEntry element.
fn parse_track_entry(reader: &mut BufReader<File>, entry_size: u64) -> Result<Option<TrackInfo>> {
    let entry_end = reader.stream_position()? + entry_size;
    let mut track_number: Option<u64> = None;
    let mut track_type: Option<u8> = None;

    while reader.stream_position()? < entry_end {
        let pos = reader.stream_position()?;

        match read_ebml_element_header(reader) {
            Ok((id, size)) => {
                if id == TRACK_NUMBER_ID {
                    track_number = Some(read_vint(reader)?);
                } else if id == TRACK_TYPE_ID {
                    let mut buf = [0u8; 1];
                    reader.read_exact(&mut buf)?;
                    track_type = Some(buf[0]);
                } else {
                    let next = reader.stream_position()? + size;
                    reader.seek(SeekFrom::Start(next.min(entry_end)))?;
                }
            }
            Err(_) => break,
        }

        if reader.stream_position()? <= pos {
            break;
        }
    }

    if let (Some(number), Some(ttype)) = (track_number, track_type) {
        Ok(Some(TrackInfo {
            number,
            track_type: ttype,
        }))
    } else {
        Ok(None)
    }
}

/// Scan a cluster for SimpleBlocks with keyframes.
fn scan_cluster(
    reader: &mut BufReader<File>,
    cluster_end: u64,
    video_tracks: &[u64],
    audio_tracks: &[u64],
    encrypt_audio: bool,
    regions: &mut Vec<Region>,
) -> Result<()> {
    while reader.stream_position()? < cluster_end {
        let pos = reader.stream_position()?;

        match read_ebml_element_header(reader) {
            Ok((id, size)) => {
                if id == SIMPLE_BLOCK_ID {
                    parse_simple_block(reader, size, video_tracks, audio_tracks, encrypt_audio, regions)?;
                } else if id == BLOCK_GROUP_ID {
                    // Scan inside BlockGroup for Block elements
                    let group_end = reader.stream_position()? + size;
                    scan_block_group(reader, group_end, video_tracks, audio_tracks, encrypt_audio, regions)?;
                } else {
                    let next = reader.stream_position()? + size;
                    reader.seek(SeekFrom::Start(next.min(cluster_end)))?;
                }
            }
            Err(_) => break,
        }

        if reader.stream_position()? <= pos {
            break;
        }
    }

    Ok(())
}

/// Scan a BlockGroup for Block elements.
fn scan_block_group(
    reader: &mut BufReader<File>,
    group_end: u64,
    video_tracks: &[u64],
    audio_tracks: &[u64],
    encrypt_audio: bool,
    regions: &mut Vec<Region>,
) -> Result<()> {
    while reader.stream_position()? < group_end {
        let pos = reader.stream_position()?;

        match read_ebml_element_header(reader) {
            Ok((id, size)) => {
                if id == BLOCK_ID {
                    // Block has the same format as SimpleBlock but no keyframe flag
                    // For BlockGroup, we need ReferenceBlock to determine keyframe
                    // For simplicity, we treat all video blocks in BlockGroup as potential keyframes
                    parse_block(reader, size, video_tracks, audio_tracks, encrypt_audio, regions)?;
                } else {
                    let next = reader.stream_position()? + size;
                    reader.seek(SeekFrom::Start(next.min(group_end)))?;
                }
            }
            Err(_) => break,
        }

        if reader.stream_position()? <= pos {
            break;
        }
    }

    Ok(())
}

/// Parse a SimpleBlock element.
fn parse_simple_block(
    reader: &mut BufReader<File>,
    size: u64,
    video_tracks: &[u64],
    audio_tracks: &[u64],
    encrypt_audio: bool,
    regions: &mut Vec<Region>,
) -> Result<()> {
    let block_start = reader.stream_position()?;
    let block_end = block_start + size;

    // Read track number (VINT)
    let track_number = read_vint(reader)?;

    // Read timecode (i16)
    let mut timecode_buf = [0u8; 2];
    reader.read_exact(&mut timecode_buf)?;

    // Read flags (u8)
    let mut flags_buf = [0u8; 1];
    reader.read_exact(&mut flags_buf)?;
    let flags = flags_buf[0];

    // Calculate data offset and length
    let header_len = reader.stream_position()? - block_start;
    let data_offset = reader.stream_position()?;
    let data_len = size.saturating_sub(header_len);

    // Check keyframe flag (bit 7)
    let is_keyframe = (flags & 0x80) != 0;

    let is_video = video_tracks.contains(&track_number);
    let is_audio = audio_tracks.contains(&track_number);

    if is_video && is_keyframe {
        regions.push(Region {
            offset: data_offset,
            len: data_len as usize,
            kind: RegionKind::VideoIFrame,
        });
    } else if is_audio && encrypt_audio {
        regions.push(Region {
            offset: data_offset,
            len: data_len as usize,
            kind: RegionKind::AudioSample,
        });
    }

    // Seek to end of block
    reader.seek(SeekFrom::Start(block_end))?;

    Ok(())
}

/// Parse a Block element (from BlockGroup).
fn parse_block(
    reader: &mut BufReader<File>,
    size: u64,
    video_tracks: &[u64],
    audio_tracks: &[u64],
    encrypt_audio: bool,
    regions: &mut Vec<Region>,
) -> Result<()> {
    let block_start = reader.stream_position()?;
    let block_end = block_start + size;

    // Read track number (VINT)
    let track_number = read_vint(reader)?;

    // Read timecode (i16)
    let mut timecode_buf = [0u8; 2];
    reader.read_exact(&mut timecode_buf)?;

    // Read flags (u8)
    let mut flags_buf = [0u8; 1];
    reader.read_exact(&mut flags_buf)?;

    // Calculate data offset and length
    let header_len = reader.stream_position()? - block_start;
    let data_offset = reader.stream_position()?;
    let data_len = size.saturating_sub(header_len);

    let is_video = video_tracks.contains(&track_number);
    let is_audio = audio_tracks.contains(&track_number);

    // For Block in BlockGroup, we treat video as keyframe (conservative approach)
    if is_video {
        regions.push(Region {
            offset: data_offset,
            len: data_len as usize,
            kind: RegionKind::VideoIFrame,
        });
    } else if is_audio && encrypt_audio {
        regions.push(Region {
            offset: data_offset,
            len: data_len as usize,
            kind: RegionKind::AudioSample,
        });
    }

    // Seek to end of block
    reader.seek(SeekFrom::Start(block_end))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vint_decoding() {
        // Test cases for VINT decoding
        // 1-byte: 0x81 = 1
        let mut cursor = std::io::Cursor::new(vec![0x81u8]);
        assert_eq!(read_vint(&mut cursor).unwrap(), 1);

        // 1-byte: 0x9F = 31
        let mut cursor = std::io::Cursor::new(vec![0x9Fu8]);
        assert_eq!(read_vint(&mut cursor).unwrap(), 31);

        // 2-byte: 0x40 0x01 = 1
        let mut cursor = std::io::Cursor::new(vec![0x40u8, 0x01]);
        assert_eq!(read_vint(&mut cursor).unwrap(), 1);

        // 2-byte: 0x40 0xFF = 255
        let mut cursor = std::io::Cursor::new(vec![0x40u8, 0xFF]);
        assert_eq!(read_vint(&mut cursor).unwrap(), 255);
    }

    #[test]
    fn test_element_id_reading() {
        // 1-byte ID: 0xA3 (SimpleBlock)
        let mut cursor = std::io::Cursor::new(vec![0xA3u8]);
        assert_eq!(read_element_id(&mut cursor).unwrap(), 0xA3);

        // 4-byte ID: EBML header
        let mut cursor = std::io::Cursor::new(vec![0x1A, 0x45, 0xDF, 0xA3]);
        assert_eq!(read_element_id(&mut cursor).unwrap(), 0x1A45DFA3);
    }

    #[test]
    fn test_simple_block_keyframe_flag() {
        // Keyframe flag is bit 7 of flags byte
        let keyframe_flags = 0x80u8;
        let non_keyframe_flags = 0x00u8;

        assert!((keyframe_flags & 0x80) != 0);
        assert!((non_keyframe_flags & 0x80) == 0);
    }
}
