//! CLI entry point for media-lock.

use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use media_lock_core::{AppError, EncryptionTask, OperationMode, ProgressHandler, TaskStats, run_task_with_stats};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use walkdir::WalkDir;

// Import i18n macro
use media_lock_core::t;

#[derive(Parser)]
#[command(name = "media-lock", version, about = "In-place video encryption tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Encrypt media files
    Encrypt {
        /// Path to file or directory
        #[arg(value_name = "PATH")]
        path: PathBuf,

        /// Password for encryption
        #[arg(short, long)]
        password: Option<String>,

        /// Encrypt audio tracks (slower)
        #[arg(long)]
        encrypt_audio: bool,

        /// Scrub metadata (title, GPS, etc.)
        #[arg(long)]
        scrub_metadata: bool,

        /// Recursively process all files in directory
        #[arg(short, long)]
        recursive: bool,

        /// Disable WAL for faster (but unsafe) operation
        #[arg(long)]
        no_wal: bool,
    },

    /// Decrypt media files
    Decrypt {
        /// Path to file or directory
        #[arg(value_name = "PATH")]
        path: PathBuf,

        /// Password for decryption
        #[arg(short, long)]
        password: Option<String>,

        /// Recursively process all files in directory
        #[arg(short, long)]
        recursive: bool,

        /// Disable WAL for faster (but unsafe) operation
        #[arg(long)]
        no_wal: bool,
    },

    /// Recover from an interrupted session
    Recover {
        /// Path to the file to recover
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },
}

/// CLI progress handler using indicatif.
struct CliProgress {
    bar: ProgressBar,
}

impl CliProgress {
    fn new() -> Arc<Self> {
        let pb = ProgressBar::new(0);
        pb.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}) {msg}",
            )
            .unwrap()
            .progress_chars("#>-"),
        );
        Arc::new(Self { bar: pb })
    }
}

impl ProgressHandler for CliProgress {
    fn on_start(&self, total_bytes: u64, message: &str) {
        self.bar.set_length(total_bytes);
        self.bar.set_message(message.to_string());
    }

    fn on_progress(&self, delta_bytes: u64) {
        self.bar.inc(delta_bytes);
    }

    fn on_message(&self, message: &str) {
        self.bar.set_message(message.to_string());
    }

    fn on_finish(&self) {
        self.bar.finish_with_message(t!("done").to_string());
    }

    fn on_error(&self, err: &AppError) {
        self.bar.println(format!("{} {}", t!("error_prefix"), err));
    }
}

/// Collect files to process based on path and recursive flag.
fn collect_files(path: &PathBuf, recursive: bool) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.clone()];
    }

    if !path.is_dir() {
        return vec![];
    }

    let walker = if recursive {
        WalkDir::new(path).follow_links(false)
    } else {
        WalkDir::new(path).max_depth(1).follow_links(false)
    };

    walker
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            let ext = e.path().extension().and_then(|s| s.to_str()).unwrap_or("");
            matches!(ext.to_lowercase().as_str(), "mp4" | "m4v" | "mov" | "mkv" | "webm")
        })
        .map(|e| e.path().to_path_buf())
        .collect()
}

/// Format bytes in human-readable form.
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format duration in human-readable form.
fn format_duration(d: std::time::Duration) -> String {
    let ms = d.as_millis();
    if ms < 1000 {
        format!("{:.1}ms", ms as f64)
    } else {
        format!("{:.3}s", d.as_secs_f64())
    }
}

/// Print detailed performance statistics.
fn print_stats(stats: &TaskStats, file_path: &PathBuf, mode: &str) {
    println!();
    println!("================================================");
    println!("{}: {}", t!("bench_complete"), file_path.file_name().unwrap_or_default().to_string_lossy());
    println!("{}: {}", t!("bench_mode"), mode);
    println!("------------------------------------------------");
    println!("{}", t!("bench_perf"));
    println!("  1. {:24} {}", t!("bench_parse_time"), format_duration(stats.parse_time));
    println!("  2. {:24} {}", t!("bench_kdf_time"), format_duration(stats.kdf_time));
    println!("  3. {:24} {}", t!("bench_io_time"), format_duration(stats.io_time));
    println!("  4. {:24} {}", t!("bench_crypto_time"), format_duration(stats.crypto_time));
    println!("  5. {:24} {}", t!("bench_total_time"), format_duration(stats.total_time));
    println!();
    println!("{}", t!("bench_data_stats"));
    println!("  1. {:24} {}", t!("bench_file_size"), format_bytes(stats.file_size));
    println!("  2. {:24} {} ({:.1}%)", t!("bench_data_size"), 
             format_bytes(stats.data_size), stats.data_ratio_percent());
    println!("  3. {:24} {}", t!("bench_iframe_count"), stats.iframe_count);
    if stats.audio_count > 0 {
        println!("  4. {:24} {}", t!("bench_audio_count"), stats.audio_count);
    }
    println!();
    println!("{}", t!("bench_speed"));
    println!("  1. {:24} {:.2} MB/s", t!("bench_crypto_throughput"), stats.crypto_throughput_mbps());
    println!("  2. {:24} {:.2} MB/s", t!("bench_io_throughput"), stats.io_throughput_mbps());
    println!("  3. {:24} {:.2} MB/s", t!("bench_perceived_speed"), stats.perceived_speed_mbps());
    println!("================================================");
}

fn process_file(file_path: PathBuf, mode: OperationMode, password: String, encrypt_audio: bool, scrub_metadata: bool, no_wal: bool) {
    println!("Processing: {}", file_path.display());
    if no_wal {
        println!("  [WARNING] WAL disabled - unsafe mode");
    }

    let start_time = Instant::now();
    let handler = CliProgress::new();
    
    let mut task = EncryptionTask::new(file_path.clone(), mode)
        .with_password(password)
        .with_handler(handler)
        .with_no_wal(no_wal);

    if mode == OperationMode::Encrypt {
        task = task.with_audio(encrypt_audio).with_metadata_scrub(scrub_metadata);
    }

    match run_task_with_stats(&task) {
        Ok(stats) => {
            let mode_str = if no_wal {
                match mode {
                    OperationMode::Encrypt => "Encrypt (I-Frame Only, No-WAL)",
                    OperationMode::Decrypt => "Decrypt (No-WAL)",
                    OperationMode::Recover => "Recover",
                }
            } else {
                match mode {
                    OperationMode::Encrypt => "Encrypt (I-Frame Only)",
                    OperationMode::Decrypt => "Decrypt",
                    OperationMode::Recover => "Recover",
                }
            };
            print_stats(&stats, &file_path, mode_str);
            println!("{} (Time: {})", t!("success_msg"), format_duration(start_time.elapsed()));
        }
        Err(e) => eprintln!("{} {}: {}", t!("fail_msg"), file_path.display(), e),
    }
}

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Encrypt {
            path,
            password,
            encrypt_audio,
            scrub_metadata,
            recursive,
            no_wal,
        } => {
            let files = collect_files(&path, recursive);

            if files.is_empty() {
                eprintln!("{} No media files found at: {}", t!("error_prefix"), path.display());
                std::process::exit(1);
            }

            for file_path in files {
                let pwd = password.clone().unwrap_or_else(|| {
                    rpassword::prompt_password(t!("enter_password_prompt")).unwrap_or_default()
                });

                if pwd.is_empty() {
                    eprintln!("{} Password cannot be empty", t!("error_prefix"));
                    continue;
                }

                process_file(file_path, OperationMode::Encrypt, pwd, encrypt_audio, scrub_metadata, no_wal);
            }
        }

        Commands::Decrypt {
            path,
            password,
            recursive,
            no_wal,
        } => {
            let files = collect_files(&path, recursive);

            if files.is_empty() {
                eprintln!("{} No media files found at: {}", t!("error_prefix"), path.display());
                std::process::exit(1);
            }

            for file_path in files {
                let pwd = password.clone().unwrap_or_else(|| {
                    rpassword::prompt_password(t!("enter_password_prompt")).unwrap_or_default()
                });

                if pwd.is_empty() {
                    eprintln!("{} Password cannot be empty", t!("error_prefix"));
                    continue;
                }

                process_file(file_path, OperationMode::Decrypt, pwd, false, false, no_wal);
            }
        }

        Commands::Recover { path } => {
            println!("Recovering: {}", path.display());

            let start_time = Instant::now();
            let handler = CliProgress::new();
            let task = EncryptionTask::new(path.clone(), OperationMode::Recover)
                .with_password("dummy".to_string())
                .with_handler(handler);

            match run_task_with_stats(&task) {
                Ok(stats) => {
                    print_stats(&stats, &path, "Recover");
                    println!("{} (Time: {})", t!("success_msg"), format_duration(start_time.elapsed()));
                }
                Err(e) => eprintln!("{} {}", t!("fail_msg"), e),
            }
        }
    }
}
