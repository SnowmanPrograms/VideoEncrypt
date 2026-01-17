//! AES-256-CTR encryption engine.

use aes::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
use aes::Aes256;
use ctr::Ctr64BE;

use crate::common::{Region, RegionKind};

/// Type alias for AES-256-CTR cipher with 64-bit big-endian counter.
type Aes256Ctr = Ctr64BE<Aes256>;

/// The crypto engine for in-place encryption/decryption.
pub struct CryptoEngine {
    /// The 256-bit AES key.
    key: [u8; 32],
    /// The 8-byte nonce (fixed per file).
    nonce: [u8; 8],
}

impl CryptoEngine {
    /// Create a new crypto engine with the given key and nonce.
    pub fn new(key: [u8; 32], nonce: [u8; 8]) -> Self {
        Self { key, nonce }
    }

    /// Build the 16-byte IV for a given block index.
    ///
    /// Format: `[8-byte nonce] || [8-byte counter (BE)]`
    fn build_iv(&self, block_idx: u64) -> [u8; 16] {
        let mut iv = [0u8; 16];
        iv[0..8].copy_from_slice(&self.nonce);
        iv[8..16].copy_from_slice(&block_idx.to_be_bytes());
        iv
    }

    /// Process a block of data at the given global offset.
    ///
    /// This function handles both encryption and decryption (XOR operation).
    /// It correctly handles unaligned offsets by seeking the cipher.
    ///
    /// # Arguments
    /// * `global_offset` - Byte offset from the start of the file.
    /// * `data` - Mutable slice of data to process in-place.
    pub fn process_block(&self, global_offset: u64, data: &mut [u8]) {
        if data.is_empty() {
            return;
        }

        // Calculate the starting block index and offset within that block
        let block_idx = global_offset / 16;
        let offset_in_block = (global_offset % 16) as usize;

        // Build the IV for this block
        let iv = self.build_iv(block_idx);

        // Create the cipher
        let mut cipher = Aes256Ctr::new(&self.key.into(), &iv.into());

        // If we're not aligned to a block boundary, we need to seek
        if offset_in_block > 0 {
            // Seek the cipher to the correct position within the keystream
            cipher.seek(offset_in_block);
        }

        // Apply the keystream to the data
        cipher.apply_keystream(data);
    }

    /// Process multiple regions of data.
    ///
    /// # Arguments
    /// * `regions` - List of regions to process.
    /// * `data` - Concatenated data from all regions (in order).
    /// * `scrub_metadata` - If true, scrub metadata regions instead of encrypting.
    pub fn process_regions(&self, regions: &[Region], data: &mut [u8], scrub_metadata: bool) {
        let mut data_offset = 0usize;

        for region in regions {
            let region_data = &mut data[data_offset..data_offset + region.len];

            match region.kind {
                RegionKind::Metadata if scrub_metadata => {
                    // Scrub metadata by replacing with spaces (UTF-8 safe)
                    region_data.fill(0x20);
                }
                RegionKind::Metadata => {
                    // Encrypt metadata normally
                    self.process_block(region.offset, region_data);
                }
                RegionKind::VideoIFrame | RegionKind::AudioSample => {
                    // Encrypt video/audio data
                    self.process_block(region.offset, region_data);
                }
            }

            data_offset += region.len;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        for (i, byte) in key.iter_mut().enumerate() {
            *byte = i as u8;
        }
        key
    }

    fn test_nonce() -> [u8; 8] {
        [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let engine = CryptoEngine::new(test_key(), test_nonce());

        let original = b"Hello, World! This is a test message for encryption.";
        let mut data = original.to_vec();

        // Encrypt
        engine.process_block(0, &mut data);
        assert_ne!(&data[..], &original[..], "Data should be encrypted");

        // Decrypt (XOR again)
        engine.process_block(0, &mut data);
        assert_eq!(&data[..], &original[..], "Data should be decrypted");
    }

    #[test]
    fn test_ctr_random_access() {
        let engine = CryptoEngine::new(test_key(), test_nonce());

        // Encrypt a larger buffer
        let mut full_data: Vec<u8> = (0..100).collect();
        let original = full_data.clone();
        engine.process_block(0, &mut full_data);

        // Now decrypt just a portion (offset 50, length 10)
        let mut partial = full_data[50..60].to_vec();
        engine.process_block(50, &mut partial);

        // Verify it matches the original
        assert_eq!(&partial[..], &original[50..60], "Random access decryption should work");
    }

    #[test]
    fn test_alignment_boundary() {
        let engine = CryptoEngine::new(test_key(), test_nonce());

        // Test data that crosses a 16-byte boundary
        // Offset 15, length 3 means bytes 15, 16, 17
        let mut data = [0xAA, 0xBB, 0xCC];
        let original = data;

        // Encrypt
        engine.process_block(15, &mut data);
        assert_ne!(data, original, "Data should be encrypted");

        // Decrypt
        engine.process_block(15, &mut data);
        assert_eq!(data, original, "Data should be decrypted correctly across boundary");
    }

    #[test]
    fn test_large_offset() {
        let engine = CryptoEngine::new(test_key(), test_nonce());

        // Test with a very large offset (100 GB)
        let large_offset: u64 = 100 * 1024 * 1024 * 1024;
        let mut data = [0x42u8; 32];
        let original = data;

        // This should not panic
        engine.process_block(large_offset, &mut data);
        assert_ne!(data, original, "Should encrypt at large offset");

        engine.process_block(large_offset, &mut data);
        assert_eq!(data, original, "Should decrypt at large offset");
    }

    #[test]
    fn test_process_regions_with_scrub() {
        let engine = CryptoEngine::new(test_key(), test_nonce());

        let regions = vec![
            Region { offset: 0, len: 16, kind: RegionKind::VideoIFrame },
            Region { offset: 16, len: 8, kind: RegionKind::Metadata },
        ];

        let mut data = vec![0x42u8; 24];

        engine.process_regions(&regions, &mut data, true);

        // First 16 bytes should be encrypted
        assert_ne!(&data[0..16], &[0x42u8; 16][..], "Video should be encrypted");

        // Last 8 bytes should be scrubbed (spaces)
        assert_eq!(&data[16..24], &[0x20u8; 8][..], "Metadata should be scrubbed");
    }
}
