# 05. CLI 适配层规范 (CLI Orchestrator Spec)

## AI 指令
本模块 (`src/bin/main.rs`) 是 Core Library 的**消费者**。
**职责**:
1.  解析命令行参数。
2.  实现 `ProgressHandler` Trait (适配 `indicatif`)。
3.  调用 `EncryptionTask` 执行业务逻辑。
4.  **不包含** 文件解析、加密运算等具体业务逻辑。

---

## 1. 命令行参数定义

利用 Feature Flag 决定默认语言。

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "media-lock", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Encrypt {
        #[arg(value_name = "PATH")]
        path: PathBuf,
        
        #[arg(short, long)]
        password: Option<String>,
        
        /// Encrypt audio tracks (slower)
        #[arg(long)]
        encrypt_audio: bool,
        
        /// Scrub metadata (title, gps, etc.)
        #[arg(long)]
        scrub_metadata: bool,
        
        #[arg(short, long)]
        recursive: bool,
    },
    Decrypt {
        #[arg(value_name = "PATH")]
        path: PathBuf,
        
        #[arg(short, long)]
        password: Option<String>,

        #[arg(short, long)]
        recursive: bool,
    },
    Recover {
        #[arg(value_name = "PATH")]
        path: PathBuf,
    }
}
```

---

## 2. 进度条适配器 (CliProgress)

实现 Core Library 定义的 `ProgressHandler` 接口。

```rust
use indicatif::{ProgressBar, ProgressStyle};
use my_core_lib::ProgressHandler; // 引用 lib
use std::sync::{Arc, Mutex};

struct CliProgress {
    bar: ProgressBar,
}

impl CliProgress {
    fn new() -> Arc<Self> {
        let pb = ProgressBar::new(0);
        pb.set_style(ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
        ).unwrap());
        Arc::new(Self { bar: pb })
    }
}

impl ProgressHandler for CliProgress {
    fn on_start(&self, total_bytes: u64, message: &str) {
        self.bar.set_length(total_bytes);
        self.bar.set_message(message.to_string());
        // 使用 i18n 宏: self.bar.set_message(t!("processing_msg"));
    }

    fn on_progress(&self, delta_bytes: u64) {
        self.bar.inc(delta_bytes);
    }

    fn on_message(&self, message: &str) {
        self.bar.set_message(message.to_string());
    }

    fn on_finish(&self) {
        self.bar.finish_with_message(t!("done"));
    }

    fn on_error(&self, err: &AppError) {
        // 在 CLI 中，错误通常打印到 stderr
        self.bar.println(format!("{} {}", t!("error_prefix"), err));
    }
}
```

---

## 3. 主流程逻辑 (main.rs)

```rust
fn main() {
    let cli = Cli::parse();
    
    // 1. 处理递归遍历 (WalkDir)
    // 如果是目录，收集所有文件；如果是文件，直接处理。
    let files = collect_files(&cli.command);

    for file_path in files {
        // 2. 准备配置
        let (mode, pwd, audio, meta) = match &cli.command {
             Commands::Encrypt { password, encrypt_audio, scrub_metadata, .. } => 
                 (OperationMode::Encrypt, password, *encrypt_audio, *scrub_metadata),
             Commands::Decrypt { password, .. } => 
                 (OperationMode::Decrypt, password, false, false),
             // ...
        };

        // 3. 密码交互 (如果 CLI 参数未提供)
        let password = pwd.clone().unwrap_or_else(|| {
            rpassword::prompt_password(t!("enter_password_prompt")).unwrap()
        });

        // 4. 构建任务
        let handler = CliProgress::new();
        let task = EncryptionTask::new(file_path, mode)
            .with_password(password)
            .with_audio(audio)
            .with_metadata_scrub(meta)
            .with_handler(handler); // 注入 CLI 进度条实现

        // 5. 执行任务
        match task.run() {
            Ok(_) => println!("{}", t!("success_msg")),
            Err(e) => eprintln!("{} {}", t!("fail_msg"), e),
        }
    }
}
```

---

## 4. 编译与构建说明

### 4.1 Cargo.toml 配置
```toml
[package]
name = "media-lock"
version = "0.1.0"

[lib]
name = "media_lock_core"
path = "src/lib.rs"

[[bin]]
name = "media-lock-cli"
path = "src/bin/main.rs"

[features]
default = ["en"]
en = []
zh = []
# 用于未来 GUI 依赖隔离
gui-support = [] 
```

### 4.2 编译指令
*   **英文版**: `cargo build --release --features en`
*   **中文版**: `cargo build --release --features zh`

---

## 5. 测试与验证
1.  **I18n 测试**: 编译 zh 版，运行 `--help` 或执行操作，验证输出是否为中文。
2.  **UI 解耦测试**: 编写一个 Mock 的 `ProgressHandler` (只记录日志不显示进度条)，在单元测试中传入 `EncryptionTask`，验证核心逻辑是否能在无终端环境下运行。