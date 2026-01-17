//! Internationalization (i18n) support.
//!
//! Provides compile-time language selection via Cargo features.

/// Macro for internationalized strings.
/// Returns the appropriate string based on the compile-time language feature.
#[macro_export]
macro_rules! t {
    // Status messages
    ("status_checking") => {
        if cfg!(feature = "zh") {
            "正在检查文件状态..."
        } else {
            "Checking file status..."
        }
    };
    ("status_recovering") => {
        if cfg!(feature = "zh") {
            "正在恢复上次中断的会话..."
        } else {
            "Recovering from previous session..."
        }
    };
    ("status_analyzing") => {
        if cfg!(feature = "zh") {
            "正在分析文件结构..."
        } else {
            "Analyzing file structure..."
        }
    };
    ("status_processing") => {
        if cfg!(feature = "zh") {
            "正在处理数据..."
        } else {
            "Processing data..."
        }
    };
    ("status_finalizing") => {
        if cfg!(feature = "zh") {
            "正在完成..."
        } else {
            "Finalizing..."
        }
    };

    // User prompts
    ("enter_password_prompt") => {
        if cfg!(feature = "zh") {
            "请输入密码: "
        } else {
            "Enter password: "
        }
    };
    ("confirm_password_prompt") => {
        if cfg!(feature = "zh") {
            "请确认密码: "
        } else {
            "Confirm password: "
        }
    };

    // Result messages
    ("done") => {
        if cfg!(feature = "zh") {
            "完成"
        } else {
            "Done"
        }
    };
    ("success_msg") => {
        if cfg!(feature = "zh") {
            "操作成功完成!"
        } else {
            "Operation completed successfully!"
        }
    };
    ("fail_msg") => {
        if cfg!(feature = "zh") {
            "操作失败:"
        } else {
            "Operation failed:"
        }
    };
    ("error_prefix") => {
        if cfg!(feature = "zh") {
            "[错误]"
        } else {
            "[Error]"
        }
    };

    // Error messages
    ("err_password_mismatch") => {
        if cfg!(feature = "zh") {
            "密码不匹配"
        } else {
            "Passwords do not match"
        }
    };
    ("err_file_not_found") => {
        if cfg!(feature = "zh") {
            "文件未找到"
        } else {
            "File not found"
        }
    };

    // Help text
    ("help_encrypt") => {
        if cfg!(feature = "zh") {
            "加密媒体文件"
        } else {
            "Encrypt media files"
        }
    };
    ("help_decrypt") => {
        if cfg!(feature = "zh") {
            "解密媒体文件"
        } else {
            "Decrypt media files"
        }
    };
    ("help_recover") => {
        if cfg!(feature = "zh") {
            "从中断的会话中恢复"
        } else {
            "Recover from an interrupted session"
        }
    };
    ("help_password") => {
        if cfg!(feature = "zh") {
            "加密/解密密码"
        } else {
            "Password for encryption/decryption"
        }
    };
    ("help_encrypt_audio") => {
        if cfg!(feature = "zh") {
            "同时加密音频轨道（较慢）"
        } else {
            "Also encrypt audio tracks (slower)"
        }
    };
    ("help_scrub_metadata") => {
        if cfg!(feature = "zh") {
            "清除敏感元数据（标题、GPS等）"
        } else {
            "Scrub sensitive metadata (title, GPS, etc.)"
        }
    };
    ("help_recursive") => {
        if cfg!(feature = "zh") {
            "递归处理目录中的所有文件"
        } else {
            "Recursively process all files in directory"
        }
    };

    // Benchmark output
    ("bench_complete") => {
        if cfg!(feature = "zh") {
            "Benchmark 完成"
        } else {
            "Benchmark Complete"
        }
    };
    ("bench_mode") => {
        if cfg!(feature = "zh") {
            "模式"
        } else {
            "Mode"
        }
    };
    ("bench_perf") => {
        if cfg!(feature = "zh") {
            "[性能指标]"
        } else {
            "[Performance Metrics]"
        }
    };
    ("bench_parse_time") => {
        if cfg!(feature = "zh") {
            "解析耗时 (Parse)"
        } else {
            "Parse Time"
        }
    };
    ("bench_kdf_time") => {
        if cfg!(feature = "zh") {
            "密钥派生 (KDF)"
        } else {
            "Key Derivation (KDF)"
        }
    };
    ("bench_io_time") => {
        if cfg!(feature = "zh") {
            "I/O 耗时 (Read+Write)"
        } else {
            "I/O Time (Read+Write)"
        }
    };
    ("bench_crypto_time") => {
        if cfg!(feature = "zh") {
            "加密耗时 (Crypto)"
        } else {
            "Crypto Time"
        }
    };
    ("bench_total_time") => {
        if cfg!(feature = "zh") {
            "总耗时"
        } else {
            "Total Time"
        }
    };
    ("bench_data_stats") => {
        if cfg!(feature = "zh") {
            "[数据统计]"
        } else {
            "[Data Statistics]"
        }
    };
    ("bench_file_size") => {
        if cfg!(feature = "zh") {
            "文件总大小"
        } else {
            "File Size"
        }
    };
    ("bench_data_size") => {
        if cfg!(feature = "zh") {
            "实际加密数据量"
        } else {
            "Encrypted Data"
        }
    };
    ("bench_iframe_count") => {
        if cfg!(feature = "zh") {
            "I帧(关键帧)数量"
        } else {
            "I-Frame Count"
        }
    };
    ("bench_audio_count") => {
        if cfg!(feature = "zh") {
            "音频样本数量"
        } else {
            "Audio Sample Count"
        }
    };
    ("bench_speed") => {
        if cfg!(feature = "zh") {
            "[速度分析]"
        } else {
            "[Speed Analysis]"
        }
    };
    ("bench_crypto_throughput") => {
        if cfg!(feature = "zh") {
            "加密吞吐量"
        } else {
            "Crypto Throughput"
        }
    };
    ("bench_io_throughput") => {
        if cfg!(feature = "zh") {
            "I/O 吞吐量"
        } else {
            "I/O Throughput"
        }
    };
    ("bench_perceived_speed") => {
        if cfg!(feature = "zh") {
            "用户感知速度"
        } else {
            "Perceived Speed"
        }
    };

    // Default fallback
    ($key:expr) => {
        $key
    };
}

pub use t;
