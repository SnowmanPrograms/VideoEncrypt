//! CLI entry point for media-lock.

use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use media_lock_core::{AppError, EncryptionTask, OperationMode, ProgressHandler};
use std::path::PathBuf;
use std::sync::Arc;
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
        } => {
            let files = collect_files(&path, recursive);

            if files.is_empty() {
                eprintln!("{} No media files found at: {}", t!("error_prefix"), path.display());
                std::process::exit(1);
            }

            for file_path in files {
                println!("Processing: {}", file_path.display());

                // Get password interactively if not provided
                let pwd = password.clone().unwrap_or_else(|| {
                    rpassword::prompt_password(t!("enter_password_prompt")).unwrap_or_default()
                });

                if pwd.is_empty() {
                    eprintln!("{} Password cannot be empty", t!("error_prefix"));
                    continue;
                }

                let handler = CliProgress::new();
                let task = EncryptionTask::new(file_path.clone(), OperationMode::Encrypt)
                    .with_password(pwd)
                    .with_audio(encrypt_audio)
                    .with_metadata_scrub(scrub_metadata)
                    .with_handler(handler);

                match task.run() {
                    Ok(_) => println!("{}", t!("success_msg")),
                    Err(e) => eprintln!("{} {}: {}", t!("fail_msg"), file_path.display(), e),
                }
            }
        }

        Commands::Decrypt {
            path,
            password,
            recursive,
        } => {
            let files = collect_files(&path, recursive);

            if files.is_empty() {
                eprintln!("{} No media files found at: {}", t!("error_prefix"), path.display());
                std::process::exit(1);
            }

            for file_path in files {
                println!("Processing: {}", file_path.display());

                let pwd = password.clone().unwrap_or_else(|| {
                    rpassword::prompt_password(t!("enter_password_prompt")).unwrap_or_default()
                });

                if pwd.is_empty() {
                    eprintln!("{} Password cannot be empty", t!("error_prefix"));
                    continue;
                }

                let handler = CliProgress::new();
                let task = EncryptionTask::new(file_path.clone(), OperationMode::Decrypt)
                    .with_password(pwd)
                    .with_handler(handler);

                match task.run() {
                    Ok(_) => println!("{}", t!("success_msg")),
                    Err(e) => eprintln!("{} {}: {}", t!("fail_msg"), file_path.display(), e),
                }
            }
        }

        Commands::Recover { path } => {
            println!("Recovering: {}", path.display());

            let handler = CliProgress::new();
            let task = EncryptionTask::new(path.clone(), OperationMode::Recover)
                .with_password("dummy".to_string()) // Password not needed for recovery
                .with_handler(handler);

            match task.run() {
                Ok(_) => println!("{}", t!("success_msg")),
                Err(e) => eprintln!("{} {}", t!("fail_msg"), e),
            }
        }
    }
}
