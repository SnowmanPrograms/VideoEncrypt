//! Integration tests for media_lock_core library.

use media_lock_core::{AppError, EncryptionTask, OperationMode, ProgressHandler};
use media_lock_core::common::{FileFooter, FOOTER_FLAG_AUDIO, FOOTER_FLAG_SCRUB_METADATA};
use media_lock_core::io::{LockManager, StreamingWal};
use media_lock_core::workflow::{execute_task_plan, plan_task, PlannedTask};
use std::sync::{Arc, Mutex};
use std::path::Path;
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

fn ebml_uint_1byte(value: u8) -> [u8; 1] {
    [value]
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
    bytes.extend_from_slice(&ebml_uint_1byte(1));
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
    bytes.extend_from_slice(&ebml_uint_1byte(2));
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

/// Minimal MKV fixture with both video and audio blocks.
fn minimal_mkv_fixture_with_audio() -> (Vec<u8>, u64, usize, u64, usize) {
    let mut bytes = Vec::new();

    // EBMLHeader
    bytes.extend_from_slice(&[0x1A, 0x45, 0xDF, 0xA3]);
    bytes.extend_from_slice(&ebml_vint_1byte(0));

    // Segment: content = Tracks (21) + Cluster (41) = 62 bytes
    bytes.extend_from_slice(&[0x18, 0x53, 0x80, 0x67]);
    bytes.extend_from_slice(&ebml_vint_1byte(62));

    // Tracks: 2 * TrackEntry(8) = 16 bytes
    bytes.extend_from_slice(&[0x16, 0x54, 0xAE, 0x6B]);
    bytes.extend_from_slice(&ebml_vint_1byte(16));

    // TrackEntry (video #1)
    bytes.push(0xAE);
    bytes.extend_from_slice(&ebml_vint_1byte(6));
    bytes.push(0xD7);
    bytes.extend_from_slice(&ebml_vint_1byte(1));
    bytes.extend_from_slice(&ebml_uint_1byte(1));
    bytes.push(0x83);
    bytes.extend_from_slice(&ebml_vint_1byte(1));
    bytes.push(1);

    // TrackEntry (audio #2)
    bytes.push(0xAE);
    bytes.extend_from_slice(&ebml_vint_1byte(6));
    bytes.push(0xD7);
    bytes.extend_from_slice(&ebml_vint_1byte(1));
    bytes.extend_from_slice(&ebml_uint_1byte(2));
    bytes.push(0x83);
    bytes.extend_from_slice(&ebml_vint_1byte(1));
    bytes.push(2);

    // Cluster: content = video SimpleBlock (22) + audio SimpleBlock (14) = 36 bytes
    bytes.extend_from_slice(&[0x1F, 0x43, 0xB6, 0x75]);
    bytes.extend_from_slice(&ebml_vint_1byte(36));

    // Video SimpleBlock: payload = 1 + 2 + 1 + 16 = 20 bytes
    bytes.push(0xA3);
    bytes.extend_from_slice(&ebml_vint_1byte(20));
    bytes.extend_from_slice(&ebml_vint_1byte(1)); // track #1
    bytes.extend_from_slice(&[0x00, 0x00]); // timecode
    bytes.push(0x80); // keyframe
    let video_offset = bytes.len() as u64;
    let video_len = 16usize;
    bytes.extend_from_slice(&[0x42u8; 16]);

    // Audio SimpleBlock: payload = 1 + 2 + 1 + 8 = 12 bytes
    bytes.push(0xA3);
    bytes.extend_from_slice(&ebml_vint_1byte(12));
    bytes.extend_from_slice(&ebml_vint_1byte(2)); // track #2
    bytes.extend_from_slice(&[0x00, 0x00]); // timecode
    bytes.push(0x00); // flags
    let audio_offset = bytes.len() as u64;
    let audio_len = 8usize;
    bytes.extend_from_slice(&[0x24u8; 8]);

    (bytes, video_offset, video_len, audio_offset, audio_len)
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

fn minimal_mp4_moov_with_metadata(chunk_offset: u32, ilst_payload: &[u8]) -> (Vec<u8>, u64, usize) {
    let trak = {
        // hdlr: version/flags (4) + pre_defined (4) + handler_type (4)
        let mut hdlr_payload = Vec::new();
        hdlr_payload.extend_from_slice(&0u32.to_be_bytes());
        hdlr_payload.extend_from_slice(&0u32.to_be_bytes());
        hdlr_payload.extend_from_slice(b"vide");
        let hdlr = mp4_box(b"hdlr", &hdlr_payload);

        // stss: entry_count = 1, sample #1 is sync sample
        let mut stss_payload = Vec::new();
        stss_payload.extend_from_slice(&0u32.to_be_bytes());
        stss_payload.extend_from_slice(&1u32.to_be_bytes());
        stss_payload.extend_from_slice(&1u32.to_be_bytes());
        let stss = mp4_box(b"stss", &stss_payload);

        // stsz: sample_size = 16, sample_count = 1
        let mut stsz_payload = Vec::new();
        stsz_payload.extend_from_slice(&0u32.to_be_bytes());
        stsz_payload.extend_from_slice(&16u32.to_be_bytes());
        stsz_payload.extend_from_slice(&1u32.to_be_bytes());
        let stsz = mp4_box(b"stsz", &stsz_payload);

        // stsc: 1 entry, 1 sample per chunk
        let mut stsc_payload = Vec::new();
        stsc_payload.extend_from_slice(&0u32.to_be_bytes());
        stsc_payload.extend_from_slice(&1u32.to_be_bytes());
        stsc_payload.extend_from_slice(&1u32.to_be_bytes());
        stsc_payload.extend_from_slice(&1u32.to_be_bytes());
        stsc_payload.extend_from_slice(&1u32.to_be_bytes());
        let stsc = mp4_box(b"stsc", &stsc_payload);

        // stco: 1 chunk offset pointing at mdat payload
        let mut stco_payload = Vec::new();
        stco_payload.extend_from_slice(&0u32.to_be_bytes());
        stco_payload.extend_from_slice(&1u32.to_be_bytes());
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

        mp4_box(b"trak", &mdia)
    };

    let ilst = mp4_box(b"ilst", ilst_payload);
    let meta = mp4_box(b"meta", &ilst);
    let udta = mp4_box(b"udta", &meta);

    let mut moov_payload = Vec::new();
    moov_payload.extend_from_slice(&trak);
    let udta_offset_in_moov_payload = moov_payload.len() as u64;
    moov_payload.extend_from_slice(&udta);

    // metadata region is the ilst payload (parser uses ilst offset + header size)
    let meta_payload_offset_in_moov = 8 + udta_offset_in_moov_payload + 8 + 8 + 8;
    (mp4_box(b"moov", &moov_payload), meta_payload_offset_in_moov, ilst_payload.len())
}

fn minimal_mp4_fixture_with_metadata() -> (Vec<u8>, u64, usize, u64, usize) {
    let sample = vec![0x55u8; 16];
    let ilst_payload = vec![0x77u8; 12];

    // ftyp: "isom" + minor_version 0
    let mut ftyp_payload = Vec::new();
    ftyp_payload.extend_from_slice(b"isom");
    ftyp_payload.extend_from_slice(&0u32.to_be_bytes());
    let ftyp = mp4_box(b"ftyp", &ftyp_payload);

    // Placeholder moov for sizing
    let (moov_placeholder, _meta_offset_in_moov, _meta_len) =
        minimal_mp4_moov_with_metadata(0, &ilst_payload);

    let mdat_offset = (ftyp.len() + moov_placeholder.len()) as u64;
    let sample_offset = mdat_offset + 8;

    let (moov, meta_offset_in_moov, meta_len) =
        minimal_mp4_moov_with_metadata(sample_offset as u32, &ilst_payload);

    let mdat = mp4_box(b"mdat", &sample);

    let mut bytes = Vec::new();
    bytes.extend_from_slice(&ftyp);
    bytes.extend_from_slice(&moov);
    bytes.extend_from_slice(&mdat);

    let meta_offset = ftyp.len() as u64 + meta_offset_in_moov;
    (bytes, sample_offset, sample.len(), meta_offset, meta_len)
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
fn test_roundtrip_encrypt_decrypt_mkv_with_audio_enabled() {
    let temp_dir = TempDir::new().unwrap();
    let (original, video_offset, video_len, audio_offset, audio_len) = minimal_mkv_fixture_with_audio();
    let path = temp_dir.path().join("fixture_audio.mkv");
    std::fs::write(&path, &original).unwrap();

    let password = "test_password_123";

    // Encrypt (with audio)
    EncryptionTask::new(path.clone(), OperationMode::Encrypt)
        .with_password(password.to_string())
        .with_audio(true)
        .run()
        .unwrap();

    assert_lock_and_wal_clean(&path);

    let encrypted = std::fs::read(&path).unwrap();
    assert_eq!(encrypted.len(), original.len() + FileFooter::SIZE);

    let footer = FileFooter::from_bytes(&encrypted[encrypted.len() - FileFooter::SIZE..]).unwrap();
    assert_eq!(footer.original_len, original.len() as u64);
    assert!(footer.encrypt_audio());
    assert!((footer.flags & FOOTER_FLAG_AUDIO) != 0);
    assert!(!footer.scrub_metadata());
    assert!((footer.flags & FOOTER_FLAG_SCRUB_METADATA) == 0);

    assert_ne!(
        &encrypted[video_offset as usize..video_offset as usize + video_len],
        &original[video_offset as usize..video_offset as usize + video_len],
        "Video region bytes should change after encryption"
    );

    assert_ne!(
        &encrypted[audio_offset as usize..audio_offset as usize + audio_len],
        &original[audio_offset as usize..audio_offset as usize + audio_len],
        "Audio region bytes should change after encryption"
    );

    // Decrypt (workflow reads flags from footer)
    EncryptionTask::new(path.clone(), OperationMode::Decrypt)
        .with_password(password.to_string())
        .run()
        .unwrap();

    assert_lock_and_wal_clean(&path);

    let decrypted = std::fs::read(&path).unwrap();
    assert_eq!(decrypted, original);
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
fn test_scrub_metadata_is_irreversible_but_safe() {
    let temp_dir = TempDir::new().unwrap();
    let (original, sample_offset, sample_len, meta_offset, meta_len) = minimal_mp4_fixture_with_metadata();
    let path = temp_dir.path().join("fixture_meta.mp4");
    std::fs::write(&path, &original).unwrap();

    let password = "test_password_123";

    // Encrypt with scrub_metadata enabled
    EncryptionTask::new(path.clone(), OperationMode::Encrypt)
        .with_password(password.to_string())
        .with_metadata_scrub(true)
        .run()
        .unwrap();

    assert_lock_and_wal_clean(&path);

    let encrypted = std::fs::read(&path).unwrap();
    let footer = FileFooter::from_bytes(&encrypted[encrypted.len() - FileFooter::SIZE..]).unwrap();
    assert!(footer.scrub_metadata());
    assert!((footer.flags & FOOTER_FLAG_SCRUB_METADATA) != 0);

    let meta_start = meta_offset as usize;
    let meta_end = meta_start + meta_len;
    assert!(encrypted[meta_start..meta_end].iter().all(|&b| b == 0x20));

    let sample_start = sample_offset as usize;
    let sample_end = sample_start + sample_len;
    assert_ne!(&encrypted[sample_start..sample_end], &original[sample_start..sample_end]);

    // Decrypt: video restores, metadata stays scrubbed (spaces)
    EncryptionTask::new(path.clone(), OperationMode::Decrypt)
        .with_password(password.to_string())
        .run()
        .unwrap();

    assert_lock_and_wal_clean(&path);

    let decrypted = std::fs::read(&path).unwrap();
    assert_eq!(decrypted.len(), original.len());
    assert_eq!(&decrypted[sample_start..sample_end], &original[sample_start..sample_end]);
    assert!(decrypted[meta_start..meta_end].iter().all(|&b| b == 0x20));
    assert_ne!(decrypted, original);
}

#[test]
fn test_decrypt_wrong_password_does_not_modify_file() {
    let temp_dir = TempDir::new().unwrap();
    let (original, _region_offset, _region_len) = minimal_mp4_fixture();
    let path = temp_dir.path().join("fixture_wrong_pwd.mp4");
    std::fs::write(&path, &original).unwrap();

    let correct = "correct_password";
    EncryptionTask::new(path.clone(), OperationMode::Encrypt)
        .with_password(correct.to_string())
        .run()
        .unwrap();

    let encrypted_before = std::fs::read(&path).unwrap();

    let err = EncryptionTask::new(path.clone(), OperationMode::Decrypt)
        .with_password("wrong_password".to_string())
        .run()
        .unwrap_err();

    assert!(matches!(err, AppError::AuthenticationFailed));
    assert_lock_and_wal_clean(&path);

    let encrypted_after = std::fs::read(&path).unwrap();
    assert_eq!(encrypted_after, encrypted_before);
}

#[test]
fn test_decrypt_wrong_password_no_wal_does_not_modify_file() {
    let temp_dir = TempDir::new().unwrap();
    let (original, _region_offset, _region_len) = minimal_mp4_fixture();
    let path = temp_dir.path().join("fixture_wrong_pwd_no_wal.mp4");
    std::fs::write(&path, &original).unwrap();

    let correct = "correct_password";
    EncryptionTask::new(path.clone(), OperationMode::Encrypt)
        .with_password(correct.to_string())
        .run()
        .unwrap();

    let encrypted_before = std::fs::read(&path).unwrap();

    let err = EncryptionTask::new(path.clone(), OperationMode::Decrypt)
        .with_password("wrong_password".to_string())
        .with_no_wal(true)
        .run()
        .unwrap_err();

    assert!(matches!(err, AppError::AuthenticationFailed));
    assert_lock_and_wal_clean(&path);

    let encrypted_after = std::fs::read(&path).unwrap();
    assert_eq!(encrypted_after, encrypted_before);
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
    let temp_dir = TempDir::new().unwrap();
    let missing = temp_dir.path().join("missing.mp4");

    let handler = MockHandler::new();
    let task = EncryptionTask::new(missing.clone(), OperationMode::Encrypt)
        .with_password("test".to_string())
        .with_handler(handler);

    let result = task.run();
    assert!(matches!(result, Err(AppError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound));
    assert_lock_and_wal_clean(&missing);
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

#[test]
fn test_plan_execute_allows_handler_attached_late() {
    let temp_dir = TempDir::new().unwrap();
    let (original, region_offset, region_len) = minimal_mp4_fixture();
    let path = temp_dir.path().join("fixture_plan_execute.mp4");
    std::fs::write(&path, &original).unwrap();

    let task = EncryptionTask::new(path.clone(), OperationMode::Encrypt)
        .with_password("test_password_123".to_string());

    let planned = plan_task(&task).unwrap();
    let plan = match planned {
        PlannedTask::Execute(plan) => plan,
        PlannedTask::Completed(_) => panic!("expected an executable plan"),
    };

    assert!(LockManager::is_locked(&path), "Planning should hold the file lock");

    // Handler is attached only for the execution stage (like CLI batch pipeline).
    let handler = MockHandler::new();
    let plan = plan.with_handler(handler.clone());

    execute_task_plan(plan).unwrap();

    assert_lock_and_wal_clean(&path);

    let log = handler.get_log();
    let start_count = log.iter().filter(|s| s.starts_with("on_start:")).count();
    let finish_count = log.iter().filter(|s| s.as_str() == "on_finish").count();
    assert_eq!(start_count, 1, "on_start should be called exactly once");
    assert_eq!(finish_count, 1, "on_finish should be called exactly once");
    assert!(
        log.iter().any(|s| s.starts_with("on_progress:")),
        "Expected progress callbacks during execution"
    );

    let encrypted = std::fs::read(&path).unwrap();
    let start = region_offset as usize;
    let end = start + region_len;
    assert_ne!(&encrypted[start..end], &original[start..end], "Region should be modified");

    // Footer should be present after encryption.
    let footer_start = encrypted.len() - FileFooter::SIZE;
    FileFooter::from_bytes(&encrypted[footer_start..]).unwrap();
}

#[test]
fn test_execute_rejects_file_size_change_since_planning() {
    use std::io::Write;

    let temp_dir = TempDir::new().unwrap();
    let (original, _region_offset, _region_len) = minimal_mp4_fixture();
    let path = temp_dir.path().join("fixture_size_change.mp4");
    std::fs::write(&path, &original).unwrap();

    let task = EncryptionTask::new(path.clone(), OperationMode::Encrypt)
        .with_password("test_password_123".to_string());

    let planned = plan_task(&task).unwrap();
    let plan = match planned {
        PlannedTask::Execute(plan) => plan,
        PlannedTask::Completed(_) => panic!("expected an executable plan"),
    };

    // Tamper with file size between plan and execution.
    let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    f.write_all(b"X").unwrap();
    f.sync_all().unwrap();

    let err = execute_task_plan(plan).unwrap_err();
    assert!(
        matches!(err, AppError::InvalidStructure(ref msg) if msg.contains("File size changed since planning")),
        "Expected stale plan error, got: {err:?}"
    );

    assert_lock_and_wal_clean(&path);
}

#[test]
fn test_execute_rejects_footer_change_since_planning_decrypt() {
    let temp_dir = TempDir::new().unwrap();
    let (original, _region_offset, _region_len) = minimal_mp4_fixture();
    let path = temp_dir.path().join("fixture_footer_change.mp4");
    std::fs::write(&path, &original).unwrap();

    let password = "test_password_123";

    EncryptionTask::new(path.clone(), OperationMode::Encrypt)
        .with_password(password.to_string())
        .run()
        .unwrap();

    let decrypt = EncryptionTask::new(path.clone(), OperationMode::Decrypt)
        .with_password(password.to_string());

    let planned = plan_task(&decrypt).unwrap();
    let plan = match planned {
        PlannedTask::Execute(plan) => plan,
        PlannedTask::Completed(_) => panic!("expected an executable plan"),
    };

    // Tamper with a checked footer field while the plan is pending.
    let mut bytes = std::fs::read(&path).unwrap();
    let footer_start = bytes.len() - FileFooter::SIZE;
    let flags_offset = footer_start + 9;
    bytes[flags_offset] ^= 0x01;
    std::fs::write(&path, &bytes).unwrap();

    let err = execute_task_plan(plan).unwrap_err();
    assert!(
        matches!(err, AppError::InvalidStructure(ref msg) if msg.contains("Footer changed since planning")),
        "Expected stale footer error, got: {err:?}"
    );

    assert_lock_and_wal_clean(&path);
}
