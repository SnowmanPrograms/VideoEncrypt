# 01. 核心领域模型与接口契约 (Shared Domain)

## AI 指令
本模块定义了系统的核心数据结构和交互接口。
**架构变更**: 代码将编译为 `lib` (Library)，供 CLI (`src/bin/cli`) 或未来的 GUI 调用。
**关键约束**:
1.  所有核心逻辑必须与 UI 解耦，通过 `ProgressHandler` Trait 反馈进度。
2.  必须支持编译时国际化 (I18n)。
3.  所有核心结构体必须是 `Send + Sync`，以便在后台线程运行。

---

## 1. 国际化与配置 (I18n & Config)

### 1.1 编译时国际化
使用 `rust-i18n` 或类似机制。在 `Cargo.toml` 中定义 features: `["en", "zh"]`。
代码中严禁硬编码字符串，必须使用宏。

```rust
// 示例宏定义 (伪代码)
#[macro_export]
macro_rules! t {
    ($key:expr) => { ... } // 根据编译 Feature 返回对应语言的 &str
}
```

### 1.2 加密配置 (Config)
传递给核心引擎的参数集合。

```rust
#[derive(Debug, Clone)]
pub struct EncryptionConfig {
    pub password: Option<String>, // 如果为 None，需在 Task 运行前通过交互获取或报错
    pub encrypt_audio: bool,      // 是否加密音频流
    pub scrub_metadata: bool,     // 是否清洗敏感元数据
    pub operation: OperationMode, // Encrypt | Decrypt
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationMode {
    Encrypt,
    Decrypt,
    Recover, // 仅用于灾难恢复模式
}
```

---

## 2. 基础数据结构

### 2.1 区域描述 (Region)
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionKind {
    VideoIFrame,
    AudioSample,
    Metadata,
}

#[derive(Debug, Clone)]
pub struct Region {
    pub offset: u64,
    pub len: usize,
    pub kind: RegionKind,
}
```

### 2.2 文件状态标识 (On-Disk)
```rust
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FileFooter {
    pub magic: [u8; 8],     // "RUST_ENC"
    pub version: u8,
    pub salt: [u8; 16],     // KDF Salt
    pub original_len: u64,  // 原始文件长度
    pub checksum: [u8; 32], // 原始数据采样校验
}
```

---

## 3. UI 交互接口 (关键解耦点)

CLI 和 GUI 必须实现此 Trait 来接收核心库的状态更新。

```rust
use std::sync::Arc;
use crate::error::AppError;

/// 进度回调接口 (必须是线程安全的)
pub trait ProgressHandler: Send + Sync {
    /// 任务开始
    /// total_bytes: 预计处理的总字节数 (用于计算百分比)
    /// message: 当前阶段描述 (支持 I18n key)
    fn on_start(&self, total_bytes: u64, message: &str);

    /// 进度更新 (增量)
    /// delta_bytes: 本次 Batch 处理的字节数
    fn on_progress(&self, delta_bytes: u64);

    /// 阶段变更 / 消息通知
    fn on_message(&self, message: &str);

    /// 任务完成
    fn on_finish(&self);

    /// 发生非致命错误 (如某个文件跳过)
    fn on_error(&self, err: &AppError);
}

/// 默认的空实现 (用于不需要 UI 的场景)
pub struct NoOpProgress;
impl ProgressHandler for NoOpProgress { ... }
```

---

## 4. 任务构建器 (Task Builder)

这是 Library 暴露给外部的主要入口点。

```rust
pub struct EncryptionTask {
    pub input_path: std::path::PathBuf,
    pub config: EncryptionConfig,
    // 使用 Arc<dyn> 存储回调，实现多态
    pub handler: Option<Arc<dyn ProgressHandler>>,
}

impl EncryptionTask {
    pub fn new(path: std::path::PathBuf, mode: OperationMode) -> Self { ... }
    
    pub fn with_password(mut self, pwd: String) -> Self { ... }
    pub fn with_audio(mut self, enable: bool) -> Self { ... }
    pub fn with_metadata_scrub(mut self, enable: bool) -> Self { ... }
    
    /// 设置进度回调
    pub fn with_handler(mut self, handler: Arc<dyn ProgressHandler>) -> Self { ... }

    /// 执行核心逻辑 (阻塞式，建议在单独线程调用)
    /// 该函数内部将串联: Lock -> Parser -> WAL -> Crypto -> Unlock
    pub fn run(self) -> crate::error::Result<()> {
        // 内部实现逻辑见 workflow.rs
    }
}
```

---

## 5. 内部核心接口 (Traits)

仅供库内部模块 (`parser`, `io`, `crypto`) 相互调用，不对外暴露。

*   `ContainerParser`: `scan_regions() -> Result<Vec<Region>>`
*   `WalManager`: `begin_batch()`, `commit_batch()`, `recover()`
*   `CryptoEngine`: `process_block()`

---

## 6. 错误处理

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("File is locked by another session")]
    FileLocked,
    
    #[error("Invalid password")]
    InvalidPassword,
    
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),
    
    // ... 其他错误
}
```