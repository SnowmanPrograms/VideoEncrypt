# 01. 核心领域模型与接口契约 (Shared Domain)

## AI 指令
本模块定义了系统中所有模块交互的“通用语言”。在编写具体实现代码前，必须严格遵守以下 Struct、Enum 和 Trait 的定义。

---

## 1. 基础数据结构

### 1.1 加密区域描述 (Region)
这是解析器（Parser）与加密引擎（Engine）交互的原子单位。
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionKind {
    VideoIFrame,    // 视频关键帧 (必须加密)
    AudioSample,    // 音频采样 (可选加密)
    Metadata,       // 敏感元数据 (覆盖或加密)
}

#[derive(Debug, Clone)]
pub struct Region {
    pub offset: u64,      // 物理文件绝对偏移量
    pub len: usize,       // 区域长度
    pub kind: RegionKind, // 区域类型
}
```

### 1.2 配置上下文 (Context)
```rust
#[derive(Debug, Clone)]
pub struct EncryptionConfig {
    pub key: [u8; 32],        // 派生后的主密钥
    pub nonce: [u8; 12],      // AES-CTR 初始 Nonce
    pub encrypt_audio: bool,  // 是否加密音频
    pub scrub_metadata: bool, // 是否清洗元数据
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OperationMode {
    Encrypt,
    Decrypt,
}
```

### 1.3 文件头/尾结构 (On-Disk Format)
追加在文件末尾 (EOF) 的标识块，用于识别文件状态。
```rust
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FileFooter {
    pub magic: [u8; 8],      // "RUST_ENC"
    pub version: u8,         // Version: 1
    pub salt: [u8; 16],      // 用于 KDF 的 Salt
    pub original_len: u64,   // 原始文件长度 (用于解密后截断)
    pub checksum: [u8; 32],  // 原始数据的采样 Hash (校验用)
}
// Note: 实际写入时需处理 Endianness (推荐 Little Endian)
```

---

## 2. 核心接口定义 (Traits)

### 2.1 容器解析器 (ContainerParser)
负责解析 MP4 或 MKV 结构，**惰性**生成加密区域。

```rust
use anyhow::Result;

pub trait ContainerParser {
    /// 快速检查文件 Magic Number，判断是否支持该格式
    fn probe(path: &std::path::Path) -> Result<bool>;

    /// 扫描并返回所有需要处理的区域
    /// 
    /// 性能要求:
    /// 1. 不应一次性加载整个 Region 列表到内存，推荐返回 Iterator 或 Channel。
    /// 2. 应当跳过无关的 Payload 数据。
    fn scan_regions(&self) -> Result<Vec<Region>>; 
    // 注：为了 Rayon 并行，Vec<Region> 是可接受的，因为 Region 结构很小。
    // 如果文件极大，可优化为 Box<dyn Iterator<Item = Region>>
}
```

### 2.2 预写日志管理器 (WalManager)
负责数据的灾难备份与恢复。

```rust
use anyhow::Result;

pub trait WalManager {
    /// 开始一个新的 Batch 事务
    /// 1. 将 regions 对应的原始数据写入 .wal 文件
    /// 2. 执行 fsync 落盘
    fn begin_batch(&mut self, file: &mut std::fs::File, regions: &[Region]) -> Result<()>;

    /// 提交事务
    /// 清空或标记当前 WAL 记录无效
    fn commit_batch(&mut self) -> Result<()>;

    /// 尝试恢复
    /// 如果发现 .lock 或 .wal，将原始数据回滚到 file 中
    fn recover_if_needed(file_path: &std::path::Path) -> Result<bool>;
}
```

### 2.3 加密转换引擎 (CryptoEngine)
负责具体的字节处理。

```rust
pub trait CryptoEngine {
    /// 原地处理数据块
    /// 
    /// offset: 该数据块在物理文件中的绝对偏移量 (用于计算 CTR Counter)
    /// data: 可变切片，原地修改
    fn process_block(&self, offset: u64, data: &mut [u8]);
}
```

---

## 3. 错误处理 (Error Types)

使用 `thiserror` 定义。

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Unknown or unsupported container format")]
    UnsupportedFormat,

    #[error("File is already encrypted")]
    AlreadyEncrypted,

    #[error("File is not encrypted or header missing")]
    NotEncrypted,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parsing error: {0}")]
    ParseError(String),
    
    #[error("Cryptography error")]
    CryptoError,
    
    #[error("Data integrity check failed during recovery")]
    IntegrityError,
}
```