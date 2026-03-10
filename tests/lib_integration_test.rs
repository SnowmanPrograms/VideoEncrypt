//! Integration tests for media_lock_core library.

use media_lock_core::{AppError, EncryptionTask, OperationMode, ProgressHandler};
use media_lock_core::common::FileFooter;
use media_lock_core::io::{LockManager, StreamingWal};
use std::sync::{Arc, Mutex};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Mock progress handler for testing.
struct MockHandler {
    log: Arc<Mutex<Vec<String>>>,
}

impl MockHandler {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            log: Arc::new(Mutex::new(Vec::new())),
        })
    }

    fn get_log(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
}

impl ProgressHandler for MockHandler {
    fn on_start(&self, total_bytes: u64, message: &str) {
        self.log.lock().unwrap().push(format!("on_start: {} bytes, {}", total_bytes, message));
    }

    fn on_progress(&self, delta_bytes: u64) {
        self.log.lock().unwrap().push(format!("on_progress: {} bytes", delta_bytes));
    }

    fn on_message(&self, message: &str) {
        self.log.lock().unwrap().push(format!("on_message: {}", message));
    }

    fn on_finish(&self) {
        self.log.lock().unwrap().push("on_finish".to_string());
    }

    fn on_error(&self, err: &AppError) {
        self.log.lock().unwrap().push(format!("on_error: {}", err));
    }
}

fn assert_lock_and_wal_clean(target: &Path) {
    assert!(
        !LockManager::lock_path_for(target).exists(),
        "Lock file should be cleaned up"
    );
    assert!(
        !StreamingWal::wal_path_for(target).exists(),
        "WAL file should be cleaned up"
    );
}

fn ebml_vint_1byte(value: u64) -> [u8; 1] {
    assert!(value <= 0x7F, "fixture only supports 1-byte VINT");
    [(0x80u8 | (value as u8))]
}

/// Minimal MKV fixture that the bundled parser can scan.
///
/// Layout:
/// - EBMLHeader (size 0)
/// - Segment
///   - Tracks (2 entries: video #1, audio #2)
///   - Cluster
///     - SimpleBlock (track #1, keyframe, 16 bytes payload)
fn minimal_mkv_fixture() -> (Vec<u8>, u64, usize) {
    let mut bytes = Vec::new();

    // EBMLHeader
    bytes.extend_from_slice(&[0x1A, 0x45, 0xDF, 0xA3]);
    bytes.extend_from_slice(&ebml_vint_1byte(0));

    // Segment: content = Tracks (21) + Cluster (27) = 48 bytes
    bytes.extend_from_slice(&[0x18, 0x53, 0x80, 0x67]);
    bytes.extend_from_slice(&ebml_vint_1byte(48));

    // Tracks: 2 * TrackEntry(8) = 16 bytes
    bytes.extend_from_slice(&[0x16, 0x54, 0xAE, 0x6B]);
    bytes.extend_from_slice(&ebml_vint_1byte(16));

    // TrackEntry (video)
    bytes.push(0xAE);
    bytes.extend_from_slice(&ebml_vint_1byte(6));
    // TrackNumber = 1
    bytes.push(0xD7);
    bytes.extend_from_slice(&ebml_vint_1byte(1));
    bytes.extend_from_slice(&ebml_vint_1byte(1));
    // TrackType = video(1)
    bytes.push(0x83);
    bytes.extend_from_slice(&ebml_vint_1byte(1));
    bytes.push(1);

    // TrackEntry (audio)
    bytes.push(0xAE);
    bytes.extend_from_slice(&ebml_vint_1byte(6));
    // TrackNumber = 2
    bytes.push(0xD7);
    bytes.extend_from_slice(&ebml_vint_1byte(1));
    bytes.extend_from_slice(&ebml_vint_1byte(2));
    // TrackType = audio(2)
    bytes.push(0x83);
    bytes.extend_from_slice(&ebml_vint_1byte(1));
    bytes.push(2);

    // Cluster: content = SimpleBlock (22 bytes)
    bytes.extend_from_slice(&[0x1F, 0x43, 0xB6, 0x75]);
    bytes.extend_from_slice(&ebml_vint_1byte(22));

    // SimpleBlock: payload = track(1) + timecode(2) + flags(1) + data(16) = 20 bytes
    bytes.push(0xA3);
    bytes.extend_from_slice(&ebml_vint_1byte(20));
    bytes.extend_from_slice(&ebml_vint_1byte(1)); // track #1
    bytes.extend_from_slice(&[0x00, 0x00]); // timecode
    bytes.push(0x80); // keyframe flag

    let region_offset = bytes.len() as u64;
    let region_len = 16usize;
    bytes.extend_from_slice(&[0x42u8; 16]);

    (bytes, region_offset, region_len)
}

fn mp4_box(box_type: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let size: u32 = (8 + payload.len()) as u32;
    let mut out = Vec::with_capacity(size as usize);
    out.extend_from_slice(&size.to_be_bytes());
    out.extend_from_slice(box_type);
    out.extend_from_slice(payload);
    out
}

fn minimal_mp4_moov(chunk_offset: u32) -> Vec<u8> {
    // hdlr: version/flags (4) + pre_defined (4) + handler_type (4)
    let mut hdlr_payload = Vec::new();
    hdlr_payload.extend_from_slice(&0u32.to_be_bytes());
    hdlr_payload.extend_from_slice(&0u32.to_be_bytes());
    hdlr_payload.extend_from_slice(b"vide");
    let hdlr = mp4_box(b"hdlr", &hdlr_payload);

    // stss: entry_count = 1, sample #1 is sync sample
    let mut stss_payload = Vec::new();
    stss_payload.extend_from_slice(&0u32.to_be_bytes()); // version/flags
    stss_payload.extend_from_slice(&1u32.to_be_bytes()); // entry_count
    stss_payload.extend_from_slice(&1u32.to_be_bytes()); // sample_number
    let stss = mp4_box(b"stss", &stss_payload);

    // stsz: sample_size = 16, sample_count = 1
    let mut stsz_payload = Vec::new();
    stsz_payload.extend_from_slice(&0u32.to_be_bytes()); // version/flags
    stsz_payload.extend_from_slice(&16u32.to_be_bytes()); // sample_size
    stsz_payload.extend_from_slice(&1u32.to_be_bytes()); // sample_count
    let stsz = mp4_box(b"stsz", &stsz_payload);

    // stsc: 1 entry, 1 sample per chunk
    let mut stsc_payload = Vec::new();
    stsc_payload.extend_from_slice(&0u32.to_be_bytes()); // version/flags
    stsc_payload.extend_from_slice(&1u32.to_be_bytes()); // entry_count
    stsc_payload.extend_from_slice(&1u32.to_be_bytes()); // first_chunk
    stsc_payload.extend_from_slice(&1u32.to_be_bytes()); // samples_per_chunk
    stsc_payload.extend_from_slice(&1u32.to_be_bytes()); // sample_description_index
    let stsc = mp4_box(b"stsc", &stsc_payload);

    // stco: 1 chunk offset pointing at mdat payload
    let mut stco_payload = Vec::new();
    stco_payload.extend_from_slice(&0u32.to_be_bytes()); // version/flags
    stco_payload.extend_from_slice(&1u32.to_be_bytes()); // entry_count
    stco_payload.extend_from_slice(&chunk_offset.to_be_bytes());
    let stco = mp4_box(b"stco", &stco_payload);

    let mut stbl_payload = Vec::new();
    stbl_payload.extend_from_slice(&stss);
    stbl_payload.extend_from_slice(&stsz);
    stbl_payload.extend_from_slice(&stsc);
    stbl_payload.extend_from_slice(&stco);
    let stbl = mp4_box(b"stbl", &stbl_payload);

    let minf = mp4_box(b"minf", &stbl);

    let mut mdia_payload = Vec::new();
    mdia_payload.extend_from_slice(&hdlr);
    mdia_payload.extend_from_slice(&minf);
    let mdia = mp4_box(b"mdia", &mdia_payload);

    let trak = mp4_box(b"trak", &mdia);
    mp4_box(b"moov", &trak)
}

/// Minimal MP4 fixture that the bundled parser can scan.
///
/// Layout:
/// - ftyp
/// - moov(trak(mdia(hdlr+minf(stbl(stss+stsz+stsc+stco)))))
/// - mdat(16 bytes sample)
fn minimal_mp4_fixture() -> (Vec<u8>, u64, usize) {
    let sample = vec![0x55u8; 16];

    // ftyp: "isom" + minor_version 0
    let mut ftyp_payload = Vec::new();
    ftyp_payload.extend_from_slice(b"isom");
    ftyp_payload.extend_from_slice(&0u32.to_be_bytes());
    let ftyp = mp4_box(b"ftyp", &ftyp_payload);

    // Build moov once to get its length; chunk offset value itself won't change box sizes.
    let moov_placeholder = minimal_mp4_moov(0);
    let mdat_offset = (ftyp.len() + moov_placeholder.len()) as u64;
    let sample_offset = mdat_offset + 8; // mdat header

    let moov = minimal_mp4_moov(sample_offset as u32);
    let mdat = mp4_box(b"mdat", &sample);

    let mut bytes = Vec::new();
    bytes.extend_from_slice(&ftyp);
    bytes.extend_from_slice(&moov);
    bytes.extend_from_slice(&mdat);

    (bytes, sample_offset, sample.len())
}

#[test]
fn test_roundtrip_encrypt_decrypt_mkv_minimal_fixture() {
    let temp_dir = TempDir::new().unwrap();
    let (original, region_offset, region_len) = minimal_mkv_fixture();
    let path = temp_dir.path().join("fixture.mkv");
    std::fs::write(&path, &original).unwrap();

    let password = "test_password_123";

    // Encrypt
    let handler = MockHandler::new();
    EncryptionTask::new(path.clone(), OperationMode::Encrypt)
        .with_password(password.to_string())
        .with_handler(handler.clone())
        .run()
        .unwrap();

    assert_lock_and_wal_clean(&path);

    let encrypted = std::fs::read(&path).unwrap();
    assert_eq!(encrypted.len(), original.len() + FileFooter::SIZE);

    let footer = FileFooter::from_bytes(&encrypted[encrypted.len() - FileFooter::SIZE..]).unwrap();
    assert_eq!(footer.original_len, original.len() as u64);

    assert_ne!(
        &encrypted[region_offset as usize..region_offset as usize + region_len],
        &original[region_offset as usize..region_offset as usize + region_len],
        "Region bytes should change after encryption"
    );

    // Decrypt
    EncryptionTask::new(path.clone(), OperationMode::Decrypt)
        .with_password(password.to_string())
        .run()
        .unwrap();

    assert_lock_and_wal_clean(&path);

    let decrypted = std::fs::read(&path).unwrap();
    assert_eq!(decrypted, original);
    println!("Handler log: {:?}", handler.get_log());
}

#[test]
fn test_roundtrip_encrypt_decrypt_mp4_minimal_fixture() {
    let temp_dir = TempDir::new().unwrap();
    let (original, region_offset, region_len) = minimal_mp4_fixture();
    let path = temp_dir.path().join("fixture.mp4");
    std::fs::write(&path, &original).unwrap();

    let password = "test_password_123";

    // Encrypt
    EncryptionTask::new(path.clone(), OperationMode::Encrypt)
        .with_password(password.to_string())
        .run()
        .unwrap();

    assert_lock_and_wal_clean(&path);

    let encrypted = std::fs::read(&path).unwrap();
    assert_eq!(encrypted.len(), original.len() + FileFooter::SIZE);

    let footer = FileFooter::from_bytes(&encrypted[encrypted.len() - FileFooter::SIZE..]).unwrap();
    assert_eq!(footer.original_len, original.len() as u64);

    assert_ne!(
        &encrypted[region_offset as usize..region_offset as usize + region_len],
        &original[region_offset as usize..region_offset as usize + region_len],
        "Region bytes should change after encryption"
    );

    // Decrypt
    EncryptionTask::new(path.clone(), OperationMode::Decrypt)
        .with_password(password.to_string())
        .run()
        .unwrap();

    assert_lock_and_wal_clean(&path);

    let decrypted = std::fs::read(&path).unwrap();
    assert_eq!(decrypted, original);
}

#[test]
fn test_encrypt_twice_fails_with_already_encrypted() {
    let temp_dir = TempDir::new().unwrap();
    let (original, _region_offset, _region_len) = minimal_mkv_fixture();
    let path = temp_dir.path().join("fixture.mkv");
    std::fs::write(&path, &original).unwrap();

    let password = "test_password_123";

    EncryptionTask::new(path.clone(), OperationMode::Encrypt)
        .with_password(password.to_string())
        .run()
        .unwrap();

    let err = EncryptionTask::new(path.clone(), OperationMode::Encrypt)
        .with_password(password.to_string())
        .run()
        .unwrap_err();

    assert!(matches!(err, AppError::AlreadyEncrypted));
    assert_lock_and_wal_clean(&path);
}

#[test]
fn test_decrypt_unencrypted_fails_with_not_encrypted() {
    let temp_dir = TempDir::new().unwrap();
    let (original, _region_offset, _region_len) = minimal_mkv_fixture();
    let path = temp_dir.path().join("fixture.mkv");
    std::fs::write(&path, &original).unwrap();

    let err = EncryptionTask::new(path.clone(), OperationMode::Decrypt)
        .with_password("test_password_123".to_string())
        .run()
        .unwrap_err();

    assert!(matches!(err, AppError::NotEncrypted));
    assert_lock_and_wal_clean(&path);
}

#[test]
fn test_encrypt_nonexistent_file() {
    let handler = MockHandler::new();
    let task = EncryptionTask::new(PathBuf::from("nonexistent_file.mp4"), OperationMode::Encrypt)
        .with_password("test".to_string())
        .with_handler(handler);

    let result = task.run();
    assert!(matches!(result, Err(AppError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound));
}

#[test]
fn test_encrypt_without_password() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.mp4");
    std::fs::write(&test_file, b"fake mp4 content").unwrap();

    let task = EncryptionTask::new(test_file, OperationMode::Encrypt);

    let result = task.run();
    assert!(matches!(result, Err(AppError::InvalidPassword)));
}

#[test]
fn test_handler_callbacks() {
    // Test that the mock handler works correctly
    let handler = MockHandler::new();
    
    handler.on_start(1000, "testing");
    handler.on_progress(500);
    handler.on_message("phase 2");
    handler.on_finish();
    
    let log = handler.get_log();
    assert_eq!(log.len(), 4);
    assert!(log[0].contains("on_start"));
    assert!(log[1].contains("on_progress"));
    assert!(log[2].contains("on_message"));
    assert!(log[3].contains("on_finish"));
}
