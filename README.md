# Media Lock

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-2021-edition-orange.svg)](https://www.rust-lang.org)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)](https://github.com/anomalyco/opencode)

**Media Lock** is an in-place, high-performance, crash-safe media file encryption system for video files (MP4, MKV, etc.). It encrypts videos directly on disk without creating temporary copies, using AES-256-CTR encryption with Argon2id key derivation.

## Features

- **In-place encryption**: No temporary files required, encrypts directly on the original file
- **I-Frame first strategy**: By default, only encrypts I-frames (keyframes), keeping videos previewable
- **Crash-safe**: Two-phase Write-Ahead Log (WAL) mechanism for atomic operations
- **High performance**: Optimized I/O patterns with sequential WAL writes and direct in-place encryption
- **Strong cryptography**: AES-256-CTR with Argon2id key derivation (memory-hard KDF)
- **Format support**: MP4, M4V, MOV, MKV, WebM containers
- **Progress tracking**: Detailed statistics including parse time, KDF time, I/O throughput, crypto throughput
- **File locking**: Prevents concurrent access to the same file
- **Recovery support**: Automatic recovery from interrupted sessions
- **Metadata scrubbing**: Option to scrub sensitive metadata (titles, GPS, etc.)
- **Internationalization**: Compile-time language selection (English/Chinese)

## Supported Formats

| Container | Extensions | Notes |
|-----------|-----------|-------|
| MP4/ISOBMFF | .mp4, .m4v, .mov, .m4a | Full support |
| Matroska | .mkv, .webm, .mka | Full support |

## Installation

### Build from Source

```bash
# Clone the repository
git clone https://github.com/your-org/media-lock.git
cd media-lock

# Build release binary
cargo build --release

# Install globally
cargo install --path .
```

The binary will be available as `media-lock`.

### Build with Language Support

```bash
# English (default)
cargo build --release

# Chinese
cargo build --release --features zh
```

## Usage

### Command Line Interface

#### Encrypt a Single File

```bash
media-lock encrypt video.mp4 --password yourpassword
```

#### Encrypt with Options

```bash
# Encrypt I-frames only (default), scrub metadata, enable WAL
media-lock encrypt video.mp4 -p yourpassword --scrub-metadata

# Encrypt with audio tracks (slower)
media-lock encrypt video.mp4 -p yourpassword --encrypt-audio

# Disable WAL for faster but unsafe operation
media-lock encrypt video.mp4 -p yourpassword --no-wal
```

#### Decrypt a File

```bash
media-lock decrypt video.mp4 --password yourpassword
```

#### Encrypt Multiple Files

```bash
# Encrypt all media files in a directory
media-lock encrypt /path/to/videos -p yourpassword

# Recursively encrypt all media files
media-lock encrypt /path/to/videos -p yourpassword --recursive
```

#### Recover from Interrupted Session

```bash
media-lock recover video.mp4
```

#### Interactive Password Input

If you don't specify `--password`, you'll be prompted interactively:

```bash
media-lock encrypt video.mp4
Enter password: ******
```

### Library Usage

#### Basic Encryption

```rust
use media_lock_core::{EncryptionTask, OperationMode};

let task = EncryptionTask::new("video.mp4".into(), OperationMode::Encrypt)
    .with_password("yourpassword".to_string());

task.run()?;
```

#### Custom Progress Handler

```rust
use media_lock_core::{EncryptionTask, OperationMode, ProgressHandler};
use std::sync::Arc;

struct MyProgress;

impl ProgressHandler for MyProgress {
    fn on_start(&self, total_bytes: u64, message: &str) {
        println!("Starting: {} ({} bytes)", message, total_bytes);
    }

    fn on_progress(&self, delta_bytes: u64) {
        println!("Processed {} bytes", delta_bytes);
    }

    fn on_message(&self, message: &str) {
        println!("Status: {}", message);
    }

    fn on_finish(&self) {
        println!("Completed!");
    }

    fn on_error(&self, err: &media_lock_core::AppError) {
        println!("Error: {}", err);
    }
}

let task = EncryptionTask::new("video.mp4".into(), OperationMode::Encrypt)
    .with_password("yourpassword".to_string())
    .with_handler(Arc::new(MyProgress));

task.run()?;
```

#### Advanced Configuration

```rust
use media_lock_core::{EncryptionTask, OperationMode};

let task = EncryptionTask::new("video.mp4".into(), OperationMode::Encrypt)
    .with_password("yourpassword".to_string())
    .with_audio(true)               // Encrypt audio tracks
    .with_metadata_scrub(true)      // Scrub metadata
    .with_no_wal(false);            // Enable WAL (default)

task.run()?;
```

#### Get Performance Statistics

```rust
use media_lock_core::{EncryptionTask, OperationMode, run_task_with_stats};

let task = EncryptionTask::new("video.mp4".into(), OperationMode::Encrypt)
    .with_password("yourpassword".to_string());

let stats = run_task_with_stats(&task)?;

println!("Total time: {:?}", stats.total_time);
println!("Crypto throughput: {:.2} MB/s", stats.crypto_throughput_mbps());
println!("I/O throughput: {:.2} MB/s", stats.io_throughput_mbps());
println!("I-Frame count: {}", stats.iframe_count);
```

## Encryption Strategy

### I-Frame First Approach

Media Lock uses an I-Frame-first encryption strategy by default:

| Content Type | Encryption Default | Description |
|--------------|-------------------|-------------|
| I-Frames (Keyframes) | Always encrypted | Complete image data |
| P-Frames/B-Frames | Not encrypted | Differences from keyframes |
| Audio | Optional (off by default) | Audio track data |

This strategy provides:
- **Previewability**: Encrypted videos can still be partially viewed (P/B frames are visible)
- **Performance**: Significantly faster than encrypting the entire file
- **Balance**: Good security for the most important data with minimal performance impact

To encrypt all video and audio data:

```bash
media-lock encrypt video.mp4 -p yourpassword --encrypt-audio
```

### In-place Operation

Media Lock encrypts files directly in-place without creating temporary copies. This approach:

- Saves disk space (no duplicate files needed)
- Reduces I/O overhead
- Works well with large files

The encryption process uses a Go-style pattern for each region:
```
read -> encrypt -> seek_back -> write
```

## Crash Recovery

### WAL Mechanism

Media Lock uses a two-phase Write-Ahead Log (WAL) for crash safety:

```
┌─────────────────────────────────────────────────────────────┐
│ Phase 1: Backup (Before any modification)                   │
├─────────────────────────────────────────────────────────────┤
│ 1. Create WAL file                                           │
│ 2. Stream all regions to WAL (sequential write)             │
│ 3. Write entry count and CRC                                │
│ 4. Sync WAL (single fsync for all data)                     │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Phase 2: Encrypt (In-place)                                │
├─────────────────────────────────────────────────────────────┤
│ For each region (sorted by offset):                         │
│   read -> encrypt -> seek_back -> write                      │
│ 5. Sync encrypted data (single fsync)                       │
│ 6. Append/remove footer                                     │
│ 7. Cleanup WAL                                               │
└─────────────────────────────────────────────────────────────┘
```

If a crash occurs:
- **Before Phase 1 completion**: Original file is intact, WAL is ignored
- **During Phase 2**: WAL contains all original data, recovery is automatic

### Recovery Process

If a previous session failed (WAL exists), the next operation will:
1. Detect the incomplete session
2. Restore original data from WAL
3. Continue with the requested operation

Manual recovery:

```bash
media-lock recover video.mp4
```

## Performance

Media Lock is optimized for high-performance encryption:

- **Streaming WAL**: Sequential writes with large buffer (8MB)
- **Minimal syncs**: Only 2 fsync operations per file (WAL + encrypted data)
- **Direct I/O**: In-place encryption with 4MB buffer
- **Single-pass parsing**: Efficient container scanning
- **I-Frame first**: Reduces data to encrypt by 70-90%

Performance characteristics:
- **Throughput scales with disk speed**: I/O bound for most operations
- **Memory efficient**: Constant memory usage regardless of file size
- **Scalable**: Works efficiently with multi-GB files

## Security Design

### Cryptographic Components

- **Encryption**: AES-256 in CTR mode
  - Supports large files with 8-byte nonce
  - Random access decryption for any region
  - Stateless, parallelizable

- **Key Derivation**: Argon2id
  - Memory-hard KDF (64 MB, 3 iterations, 4 parallel lanes)
  - OWASP-recommended parameters
  - 16-byte random salt per file

- **Random Numbers**: Cryptographically secure RNG for salts and nonces

### File Format

Encrypted files include a 73-byte footer:

```
Offset  Size    Field
------  ------  -----
0       8       Magic: "RUST_ENC"
8       1       Version
9       16      Salt (for key derivation)
25      8       Nonce (for AES-CTR)
33      8       Original file length
41      32      Checksum (sample of original data)
```

### Security Considerations

- **No key storage**: Password is never stored, only the salt is saved
- **Brute-force protection**: Argon2id memory-hard KDF
- **Metadata scrubbing**: Option to remove sensitive metadata
- **Replay protection**: Unique salt and nonce per encryption

## Configuration Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--password` | `-p` | Password for encryption/decryption | Prompt if omitted |
| `--encrypt-audio` | | Encrypt audio tracks | false |
| `--scrub-metadata` | | Scrub sensitive metadata | false |
| `--recursive` | `-r` | Process directory recursively | false |
| `--no-wal` | | Disable WAL (faster but unsafe) | false |

### Performance vs Safety

- **With WAL** (default): Crash-safe, slightly slower (~2 syncs)
- **Without WAL** (`--no-wal`): Faster, but file may be corrupted on crash

Only use `--no-wal` if:
- You have backups of the original files
- The system is stable with UPS
- You accept the risk of data loss on crash

## Development

### Build

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with language features
cargo build --features en  # English (default)
cargo build --features zh  # Chinese
cargo build --features gui-support  # Future GUI support
```

### Project Structure

```
media-lock/
├── src/
│   ├── bin/main.rs       # CLI entry point
│   ├── lib.rs            # Library entry point
│   ├── common.rs         # Domain models and types
│   ├── workflow.rs       # Core orchestration
│   ├── crypto/
│   │   ├── mod.rs
│   │   ├── engine.rs     # AES-256-CTR implementation
│   │   └── key_deriv.rs  # Argon2id key derivation
│   ├── parsers/
│   │   ├── mod.rs
│   │   ├── mp4.rs        # MP4/MOV parser
│   │   └── mkv.rs        # MKV/WebM parser
│   ├── io/
│   │   ├── mod.rs
│   │   ├── wal.rs        # Write-Ahead Log
│   │   └── locker.rs     # File locking
│   ├── error.rs          # Error types
│   └── i18n.rs           # Internationalization
├── tests/
│   └── lib_integration_test.rs
├── doc/                  # Development docs (pre-design)
├── Cargo.toml
└── README.md
```

### Testing

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_encrypt_decrypt_roundtrip

# Run with output
cargo test -- --nocapture

# Run integration tests
cargo test --test lib_integration_test
```

## License

MIT License - see LICENSE file for details

## Contributing

Contributions are welcome! Please ensure:
- Code passes `cargo clippy` and `cargo fmt`
- Tests are added for new features
- Documentation is updated

## Credits

Developed by the VideoEncrypt Team
