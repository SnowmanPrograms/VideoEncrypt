# 05. CLI 调度与交互规范 (CLI Orchestrator Spec)

## AI 指令
本模块 (`main.rs` 及相关逻辑) 是应用程序的入口。
**职责**:
1.  解析用户命令行参数 (`clap`)。
2.  管理用户交互（密码输入、确认提示、进度条）。
3.  调度核心流程：文件发现 -> 锁检查 -> 格式解析 -> 分批加密 -> 状态终结。
4.  **关键约束**: 必须优雅处理 `Ctrl+C` 中断（尽管我们有 WAL，但能优雅退出更好）。

---

## 1. 命令行接口定义 (`clap`)

使用 `clap` 的 Derive 模式定义参数结构。

```rust
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "media-lock", version, about = "High-performance in-place media encryption")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// 启用详细日志
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 加密文件或目录
    Encrypt {
        /// 目标路径 (文件或文件夹)
        #[arg(value_name = "PATH")]
        path: PathBuf,

        /// 递归处理子目录
        #[arg(short, long)]
        recursive: bool,

        /// 密码 (如果不提供则交互式输入)
        #[arg(short, long)]
        password: Option<String>,
        
        /// 包含音频流加密 (默认不加密)
        #[arg(long)]
        encrypt_audio: bool,
    },

    /// 解密文件或目录
    Decrypt {
        #[arg(value_name = "PATH")]
        path: PathBuf,

        #[arg(short, long)]
        recursive: bool,

        #[arg(short, long)]
        password: Option<String>,
    },
    
    /// 尝试恢复中断的任务
    Recover {
        #[arg(value_name = "PATH")]
        path: PathBuf,
    }
}
```

---

## 2. 核心调度流程 (`process_file`)

这是程序的心脏。对每个目标文件执行此函数。

### 2.1 状态检测与决策
在执行任何 IO 之前：
1.  **Check Lock**: 检查是否存在 `.lock`。
    *   若存在 -> 抛出 `Error::PreviousSessionFailed`，提示用户运行 `recover` 命令或自动恢复。
2.  **Check Magic**: 读取 EOF Footer。
    *   若 Command == Encrypt && Magic 存在 -> Skip (已加密)。
    *   若 Command == Decrypt && Magic 不存在 -> Skip (非加密文件)。

### 2.2 密码获取 (KDF 准备)
*   如果 CLI 参数未提供密码，使用 `rpassword` 库提示用户输入。
*   **Encrypt 模式**: 生成随机 Salt，调用 `Argon2` 生成 Key。
*   **Decrypt 模式**: 从 EOF Footer 读取 Salt，结合用户输入计算 Key，**必须校验 Checksum** (尝试解密 Footer 中的 Checksum 字段验证密码正确性)。

### 2.3 执行循环 (The Loop)
```rust
// 伪代码逻辑
fn process_single_file(ctx: Context, file_path: PathBuf) -> Result<()> {
    // 1. Setup
    let mut file = OpenOptions::new().read(true).write(true).open(&file_path)?;
    let file_len = file.metadata()?.len();
    
    // 2. Lock
    let mut locker = LockManager::acquire(&file_path)?;
    
    // 3. Parse (Lazy/Streamed)
    let parser = detect_parser(&file_path)?; // MP4 or MKV
    let regions = parser.scan_regions(&file)?; // 获取所有需加密区域
    
    // 4. Batch Processing strategy
    // 将 Region 列表按物理 Offset 排序，并切分为 16MB 左右的 Batches
    // 这是为了控制 WAL 大小和内存占用
    let batches = chunk_regions(regions, batch_size = 16 * 1024 * 1024);
    
    let pb = ProgressBar::new(batches.len()); // indicatif
    
    // 5. Execution
    let mut wal = WalManager::new(&file_path);
    let engine = CryptoEngine::new(ctx.key);
    
    for batch in batches {
        // A. WAL Write (Crash Safety Point)
        wal.begin_batch(&mut file, &batch)?;
        
        // B. Memory Encrypt
        // 读取数据 -> AES-CTR -> 内存中修改
        let mut data = read_batch_data(&file, &batch)?;
        engine.process_regions(&batch, &mut data);
        
        // C. Disk Write
        write_batch_data(&mut file, &batch, &data)?;
        file.sync_all()?; // 物理落盘
        
        // D. Commit
        wal.commit_batch()?;
        
        pb.inc(1);
    }
    
    // 6. Finalize
    if ctx.mode == Encrypt {
        write_footer(&mut file, ctx.salt, original_hash)?;
    } else {
        remove_footer(&mut file)?;
    }
    
    // 7. Unlock
    locker.release()?;
    
    Ok(())
}
```

---

## 3. 目录遍历与并发策略
*   **遍历库**: 使用 `walkdir` crate。
*   **并发模型**:
    *   **建议**: **串行处理文件**。
    *   *理由*: 机械硬盘 (HDD) 对随机读写非常敏感。如果是多线程同时加密 4 个视频文件，磁头会疯狂跳跃 (Thrashing)，导致总吞吐量暴跌。
    *   *优化*: 可以在文件内部使用 `Rayon` 并行计算 AES（如果 CPU 是瓶颈），但在 IO 密集型场景下，单线程顺序 IO 往往是最快的。

---

## 4. UI 交互体验 (`indicatif`)
设计双层进度条：
1.  **总进度**: `[====>......] 3/10 Files` (处理文件夹时)。
2.  **当前文件**: `[===>.......] 45% | 120MB/s | ETA: 5s` (处理单个大文件时)。
    *   使用 `ProgressBar::set_style` 定制模板。
    *   对于小文件（<10MB），不显示文件级进度条，避免闪烁。

---

## 5. 测试与 E2E 验证计划

### 5.1 端到端测试 (E2E Test)
这是验证系统可用性的最终测试。

**测试脚本逻辑**:
1.  **生成源文件**: 创建 10MB 随机数据文件。
2.  **伪造容器**: 在特定 Offset 写入 MP4 Box Header (`moov`, `trak`...) 甚至不需要真实 Payload，只要解析器能跑通即可。
3.  **计算 Hash**: `sha256sum source.mp4`.
4.  **加密**: `cargo run -- encrypt source.mp4 -p 123456`.
5.  **验证加密**:
    *   文件大小是否增加了 (Header Size)?
    *   文件 Hash 是否改变?
    *   再次计算 Hash。
6.  **解密**: `cargo run -- decrypt source.mp4 -p 123456`.
7.  **验证还原**:
    *   解密后的 Hash 必须 === 步骤 3 的原始 Hash。
    *   文件大小必须复原。