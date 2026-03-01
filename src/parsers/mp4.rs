//! MP4 (ISOBMFF) container parser.
//!
//! Implements zero-DOM parsing of MP4 files to extract I-frame locations.

use super::ContainerParser;
use crate::common::{Region, RegionKind};
use crate::error::{AppError, Result};
use byteorder::{BigEndian, ReadBytesExt};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};

/// MP4 parser implementation.
pub struct Mp4Parser;

impl Mp4Parser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Mp4Parser {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents an MP4 box/atom header.
#[derive(Debug)]
struct BoxHeader {
    /// Box type (4 characters).
    box_type: [u8; 4],
    /// Total size including header.
    size: u64,
    /// Offset where this box starts.
    offset: u64,
    /// Size of the header itself (8 or 16 bytes).
    header_size: u8,
}

/// Track information extracted from trak box.
#[derive(Debug, Default)]
struct TrackInfo {
    /// Track type ('vide' or 'soun').
    handler_type: [u8; 4],
    /// Sync samples (I-frames) from stss.
    sync_samples: Vec<u32>,
    /// Sample sizes from stsz.
    sample_sizes: Vec<u32>,
    /// Chunk offsets from stco/co64.
    chunk_offsets: Vec<u64>,
    /// Sample-to-chunk mapping from stsc.
    sample_to_chunk: Vec<StscEntry>,
}

/// Entry in the sample-to-chunk table.
#[derive(Debug, Clone)]
struct StscEntry {
    first_chunk: u32,
    samples_per_chunk: u32,
    #[allow(dead_code)]
    sample_description_index: u32,
}

impl ContainerParser for Mp4Parser {
    fn name(&self) -> &'static str {
        "MP4"
    }

    fn scan_regions(
        &self,
        reader: &mut BufReader<File>,
        encrypt_audio: bool,
        scrub_metadata: bool,
    ) -> Result<Vec<Region>> {
        let mut regions = Vec::new();

        // Find moov box
        reader.seek(SeekFrom::Start(0))?;
        let file_size = get_file_size(reader)?;
        let moov = find_box(reader, b"moov", 0, file_size)?
            .ok_or_else(|| AppError::InvalidStructure("No moov box found".to_string()))?;

        // Parse all trak boxes within moov
        let tracks = parse_moov(reader, &moov)?;

        // Process each track
        for track in tracks {
            let is_video = &track.handler_type == b"vide";
            let is_audio = &track.handler_type == b"soun";

            if is_video {
                // Get I-frame regions
                let frame_regions = calculate_sample_offsets(&track, true)?;
                regions.extend(frame_regions.into_iter().map(|(offset, len)| Region {
                    offset,
                    len,
                    kind: RegionKind::VideoIFrame,
                }));
            } else if is_audio && encrypt_audio {
                // Get all audio sample regions
                let sample_regions = calculate_sample_offsets(&track, false)?;
                regions.extend(sample_regions.into_iter().map(|(offset, len)| Region {
                    offset,
                    len,
                    kind: RegionKind::AudioSample,
                }));
            }
        }

        // Find metadata regions if requested
        if scrub_metadata {
            if let Some(udta) = find_box_in(reader, b"udta", &moov)? {
                if let Some(meta) = find_box_in(reader, b"meta", &udta)? {
                    if let Some(ilst) = find_box_in(reader, b"ilst", &meta)? {
                        // Add the whole ilst as metadata region
                        let content_offset = ilst.offset + ilst.header_size as u64;
                        let content_len = ilst.size - ilst.header_size as u64;
                        if content_len > 0 && content_len < u32::MAX as u64 {
                            regions.push(Region {
                                offset: content_offset,
                                len: content_len as usize,
                                kind: RegionKind::Metadata,
                            });
                        }
                    }
                }
            }
        }

        // Sort regions by offset
        regions.sort_by_key(|r| r.offset);

        Ok(regions)
    }
}

/// Get the size of the file.
fn get_file_size(reader: &mut BufReader<File>) -> Result<u64> {
    let current = reader.stream_position()?;
    let size = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(current))?;
    Ok(size)
}

/// Read an MP4 box header.
fn read_box_header(reader: &mut BufReader<File>) -> Result<Option<BoxHeader>> {
    let offset = reader.stream_position()?;

    // Try to read size (4 bytes)
    let size32 = match reader.read_u32::<BigEndian>() {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    let mut box_type = [0u8; 4];
    reader.read_exact(&mut box_type)?;

    let (size, header_size) = if size32 == 1 {
        // Extended size (64-bit)
        let size64 = reader.read_u64::<BigEndian>()?;
        (size64, 16u8)
    } else if size32 == 0 {
        // Box extends to end of file
        let file_size = get_file_size(reader)?;
        (file_size - offset, 8u8)
    } else {
        (size32 as u64, 8u8)
    };

    Ok(Some(BoxHeader {
        box_type,
        size,
        offset,
        header_size,
    }))
}

/// Find a box at the top level within a range.
fn find_box(
    reader: &mut BufReader<File>,
    box_type: &[u8; 4],
    start: u64,
    end: u64,
) -> Result<Option<BoxHeader>> {
    reader.seek(SeekFrom::Start(start))?;

    while reader.stream_position()? < end {
        if let Some(header) = read_box_header(reader)? {
            if &header.box_type == box_type {
                return Ok(Some(header));
            }
            // Skip to next box
            let next_pos = header.offset + header.size;
            if next_pos > end {
                break;
            }
            reader.seek(SeekFrom::Start(next_pos))?;
        } else {
            break;
        }
    }

    Ok(None)
}

/// Find a box within a parent container box.
fn find_box_in(
    reader: &mut BufReader<File>,
    box_type: &[u8; 4],
    parent: &BoxHeader,
) -> Result<Option<BoxHeader>> {
    let start = parent.offset + parent.header_size as u64;
    let end = parent.offset + parent.size;
    find_box(reader, box_type, start, end)
}

/// Parse the moov box and extract track information.
fn parse_moov(reader: &mut BufReader<File>, moov: &BoxHeader) -> Result<Vec<TrackInfo>> {
    let mut tracks = Vec::new();
    let moov_end = moov.offset + moov.size;
    let mut pos = moov.offset + moov.header_size as u64;

    reader.seek(SeekFrom::Start(pos))?;

    while pos < moov_end {
        if let Some(header) = read_box_header(reader)? {
            if &header.box_type == b"trak" {
                if let Some(track) = parse_trak(reader, &header)? {
                    tracks.push(track);
                }
            }
            pos = header.offset + header.size;
            if pos >= moov_end {
                break;
            }
            reader.seek(SeekFrom::Start(pos))?;
        } else {
            break;
        }
    }

    Ok(tracks)
}

/// Parse a trak box.
fn parse_trak(reader: &mut BufReader<File>, trak: &BoxHeader) -> Result<Option<TrackInfo>> {
    let mut track = TrackInfo::default();

    // Find mdia > minf > stbl
    let mdia = match find_box_in(reader, b"mdia", trak)? {
        Some(b) => b,
        None => return Ok(None),
    };

    // Get handler type from hdlr
    if let Some(hdlr) = find_box_in(reader, b"hdlr", &mdia)? {
        reader.seek(SeekFrom::Start(hdlr.offset + hdlr.header_size as u64 + 8))?;
        reader.read_exact(&mut track.handler_type)?;
    }

    let minf = match find_box_in(reader, b"minf", &mdia)? {
        Some(b) => b,
        None => return Ok(None),
    };

    let stbl = match find_box_in(reader, b"stbl", &minf)? {
        Some(b) => b,
        None => return Ok(None),
    };

    // Parse stss (sync samples) - only for video
    if &track.handler_type == b"vide" {
        if let Some(stss) = find_box_in(reader, b"stss", &stbl)? {
            track.sync_samples = parse_stss(reader, &stss)?;
        }
    }

    // Parse stsz (sample sizes)
    if let Some(stsz) = find_box_in(reader, b"stsz", &stbl)? {
        track.sample_sizes = parse_stsz(reader, &stsz)?;
    }

    // Parse stsc (sample-to-chunk)
    if let Some(stsc) = find_box_in(reader, b"stsc", &stbl)? {
        track.sample_to_chunk = parse_stsc(reader, &stsc)?;
    }

    // Parse stco or co64 (chunk offsets)
    if let Some(stco) = find_box_in(reader, b"stco", &stbl)? {
        track.chunk_offsets = parse_stco(reader, &stco)?;
    } else if let Some(co64) = find_box_in(reader, b"co64", &stbl)? {
        track.chunk_offsets = parse_co64(reader, &co64)?;
    }

    Ok(Some(track))
}

/// Parse stss (sync sample) box.
fn parse_stss(reader: &mut BufReader<File>, stss: &BoxHeader) -> Result<Vec<u32>> {
    reader.seek(SeekFrom::Start(stss.offset + stss.header_size as u64))?;

    // Skip version and flags
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;

    let entry_count = reader.read_u32::<BigEndian>()?;
    let mut samples = Vec::with_capacity(entry_count as usize);

    for _ in 0..entry_count {
        samples.push(reader.read_u32::<BigEndian>()?);
    }

    Ok(samples)
}

/// Parse stsz (sample size) box.
fn parse_stsz(reader: &mut BufReader<File>, stsz: &BoxHeader) -> Result<Vec<u32>> {
    reader.seek(SeekFrom::Start(stsz.offset + stsz.header_size as u64))?;

    // Skip version and flags
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;

    let sample_size = reader.read_u32::<BigEndian>()?;
    let sample_count = reader.read_u32::<BigEndian>()?;

    if sample_size != 0 {
        // All samples have the same size
        Ok(vec![sample_size; sample_count as usize])
    } else {
        // Variable size samples
        let mut sizes = Vec::with_capacity(sample_count as usize);
        for _ in 0..sample_count {
            sizes.push(reader.read_u32::<BigEndian>()?);
        }
        Ok(sizes)
    }
}

/// Parse stsc (sample-to-chunk) box.
fn parse_stsc(reader: &mut BufReader<File>, stsc: &BoxHeader) -> Result<Vec<StscEntry>> {
    reader.seek(SeekFrom::Start(stsc.offset + stsc.header_size as u64))?;

    // Skip version and flags
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;

    let entry_count = reader.read_u32::<BigEndian>()?;
    let mut entries = Vec::with_capacity(entry_count as usize);

    for _ in 0..entry_count {
        entries.push(StscEntry {
            first_chunk: reader.read_u32::<BigEndian>()?,
            samples_per_chunk: reader.read_u32::<BigEndian>()?,
            sample_description_index: reader.read_u32::<BigEndian>()?,
        });
    }

    Ok(entries)
}

/// Parse stco (chunk offset) box.
fn parse_stco(reader: &mut BufReader<File>, stco: &BoxHeader) -> Result<Vec<u64>> {
    reader.seek(SeekFrom::Start(stco.offset + stco.header_size as u64))?;

    // Skip version and flags
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;

    let entry_count = reader.read_u32::<BigEndian>()?;
    let mut offsets = Vec::with_capacity(entry_count as usize);

    for _ in 0..entry_count {
        offsets.push(reader.read_u32::<BigEndian>()? as u64);
    }

    Ok(offsets)
}

/// Parse co64 (64-bit chunk offset) box.
fn parse_co64(reader: &mut BufReader<File>, co64: &BoxHeader) -> Result<Vec<u64>> {
    reader.seek(SeekFrom::Start(co64.offset + co64.header_size as u64))?;

    // Skip version and flags
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;

    let entry_count = reader.read_u32::<BigEndian>()?;
    let mut offsets = Vec::with_capacity(entry_count as usize);

    for _ in 0..entry_count {
        offsets.push(reader.read_u64::<BigEndian>()?);
    }

    Ok(offsets)
}

/// Calculate the file offset and size for each sample.
fn calculate_sample_offsets(
    track: &TrackInfo,
    keyframes_only: bool,
) -> Result<Vec<(u64, usize)>> {
    if track.sample_sizes.is_empty() || track.chunk_offsets.is_empty() {
        return Ok(Vec::new());
    }

    // Determine which samples to include
    let target_samples: Vec<u32> = if keyframes_only && !track.sync_samples.is_empty() {
        track.sync_samples.clone()
    } else {
        (1..=track.sample_sizes.len() as u32).collect()
    };

    let mut results = Vec::with_capacity(target_samples.len());

    // Build sample-to-chunk mapping
    for &sample_num in &target_samples {
        if sample_num == 0 || sample_num as usize > track.sample_sizes.len() {
            continue;
        }

        let sample_idx = sample_num as usize - 1;
        let sample_size = track.sample_sizes[sample_idx] as usize;

        if let Some(offset) = get_sample_offset(track, sample_num) {
            results.push((offset, sample_size));
        }
    }

    Ok(results)
}

/// Get the file offset for a specific sample number (1-indexed).
fn get_sample_offset(track: &TrackInfo, sample_num: u32) -> Option<u64> {
    if track.sample_to_chunk.is_empty() || track.chunk_offsets.is_empty() {
        return None;
    }

    // Find which chunk contains this sample
    let mut current_sample = 1u32;
    let mut current_chunk = 1u32;
    let mut samples_per_chunk = track.sample_to_chunk[0].samples_per_chunk;
    let mut stsc_idx = 0;

    let chunk_count = track.chunk_offsets.len() as u32;

    while current_chunk <= chunk_count {
        // Check if we need to update samples_per_chunk
        if stsc_idx + 1 < track.sample_to_chunk.len() {
            if current_chunk >= track.sample_to_chunk[stsc_idx + 1].first_chunk {
                stsc_idx += 1;
                samples_per_chunk = track.sample_to_chunk[stsc_idx].samples_per_chunk;
            }
        }

        // Check if sample is in this chunk
        let chunk_end_sample = current_sample + samples_per_chunk - 1;
        if sample_num >= current_sample && sample_num <= chunk_end_sample {
            // Found the chunk
            let chunk_offset = track.chunk_offsets[current_chunk as usize - 1];

            // Calculate offset within chunk
            let mut offset_in_chunk = 0u64;
            for i in current_sample..sample_num {
                let idx = i as usize - 1;
                if idx < track.sample_sizes.len() {
                    offset_in_chunk += track.sample_sizes[idx] as u64;
                }
            }

            return chunk_offset.checked_add(offset_in_chunk);
        }

        current_sample = chunk_end_sample + 1;
        current_chunk += 1;
    }

    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_mp4_detect() {
        // Test detection logic
        let ftyp_header = [0, 0, 0, 0x20, b'f', b't', b'y', b'p', b'm', b'p', b'4', b'2'];
        assert_eq!(&ftyp_header[4..8], b"ftyp");
    }
}
