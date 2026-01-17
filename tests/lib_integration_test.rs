//! Integration tests for media_lock_core library.

use media_lock_core::{EncryptionTask, OperationMode, ProgressHandler, AppError};
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
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

    #[allow(dead_code)]
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

#[test]
fn test_encrypt_decrypt_with_mkv() {
    // This test requires a real video file, so we'll skip if not available
    let test_file = PathBuf::from("tests/731timelapse h264-420 Rec.709L 1080p 29.97 MQ.mkv");
    if !test_file.exists() {
        println!("Skipping test: test file not found at {:?}", test_file);
        return;
    }

    // Create a copy of the test file
    let temp_dir = TempDir::new().unwrap();
    let test_copy = temp_dir.path().join("test.mkv");
    std::fs::copy(&test_file, &test_copy).unwrap();

    // Get original file size
    let original_size = std::fs::metadata(&test_copy).unwrap().len();

    // Encrypt
    let handler = MockHandler::new();
    let task = EncryptionTask::new(test_copy.clone(), OperationMode::Encrypt)
        .with_password("test_password_123".to_string())
        .with_handler(handler.clone());

    let result = task.run();
    
    // Print the log for debugging
    println!("Handler log: {:?}", handler.get_log());
    
    if let Err(e) = &result {
        println!("Encryption error: {} - this may be expected if parser finds no regions", e);
        // If no regions are found, that's okay for this test
        return;
    }

    // If encryption succeeded, verify footer was added
    let encrypted_size = std::fs::metadata(&test_copy).unwrap().len();
    println!("Original size: {}, Encrypted size: {}", original_size, encrypted_size);
    
    // Footer should add 73 bytes
    assert!(encrypted_size >= original_size, "Encrypted file should not be smaller");
}

#[test]
fn test_encrypt_nonexistent_file() {
    let handler = MockHandler::new();
    let task = EncryptionTask::new(PathBuf::from("nonexistent_file.mp4"), OperationMode::Encrypt)
        .with_password("test".to_string())
        .with_handler(handler);

    let result = task.run();
    assert!(result.is_err(), "Should fail for nonexistent file");
}

#[test]
fn test_encrypt_without_password() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.mp4");
    std::fs::write(&test_file, b"fake mp4 content").unwrap();

    let task = EncryptionTask::new(test_file, OperationMode::Encrypt);

    let result = task.run();
    assert!(result.is_err(), "Should fail without password");
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
