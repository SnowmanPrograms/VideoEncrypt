# Media Lock

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-2021-edition-orange.svg)](https://www.rust-lang.org)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)](https://github.com/anomalyco/opencode)

**Media Lock** 是一个高性能、原位操作、崩溃安全的视频文件加密系统，支持 MP4、MKV 等格式。它直接在磁盘上加密视频文件，无需创建临时副本，使用 AES-256-CTR 加密和 Argon2id 密钥派生算法。

## 特性

- **原位加密**：无需临时文件，直接在原文件上加密
- **I-Frame 优先策略**：默认只加密 I 帧（关键帧），保持视频可预览
- **崩溃安全**：两阶段预写日志（WAL）机制确保原子性操作
- **高性能**：优化的 I/O 模式，顺序 WAL 写入和直接原位加密
- **强加密**：AES-256-CTR 加密配合 Argon2id 密钥派生（内存硬化的 KDF）
- **格式支持**：MP4、M4V、MOV、MKV、WebM 容器
- **进度跟踪**：详细的统计信息，包括解析时间、KDF 时间、I/O 吞吐量、加密吞吐量
- **文件锁定**：防止对同一文件的并发访问
- **恢复支持**：自动从中断会话中恢复
- **元数据清除**：可选择清除敏感元数据（标题、GPS 等）
- **国际化**：编译时语言选择（英文/中文）

## 支持的格式

| 容器格式 | 扩展名 | 备注 |
|---------|--------|------|
| MP4/ISOBMFF | .mp4, .m4v, .mov, .m4a | 完整支持 |
| Matroska | .mkv, .webm, .mka | 完整支持 |

## 安装

### 从源码构建

```bash
# 克隆仓库
git clone https://github.com/your-org/media-lock.git
cd media-lock

# 构建发布版本
cargo build --release

# 全局安装
cargo install --path .
```

二进制文件将作为 `media-lock` 可用。

### 构建并指定语言支持

```bash
# 英文（默认）
cargo build --release

# 中文
cargo build --release --features zh
```

## 使用方法

### 命令行界面

#### 加密单个文件

```bash
media-lock encrypt video.mp4 --password yourpassword
```

#### 带选项加密

```bash
# 仅加密 I 帧（默认），清除元数据，启用 WAL
media-lock encrypt video.mp4 -p yourpassword --scrub-metadata

# 同时加密音频轨道（较慢）
media-lock encrypt video.mp4 -p yourpassword --encrypt-audio

# 禁用 WAL 以获得更快但不安全的操作
media-lock encrypt video.mp4 -p yourpassword --no-wal
```

#### 解密文件

```bash
media-lock decrypt video.mp4 --password yourpassword
```

#### 加密多个文件

```bash
# 加密目录中的所有媒体文件
media-lock encrypt /path/to/videos -p yourpassword

# 递归加密所有媒体文件
media-lock encrypt /path/to/videos -p yourpassword --recursive

# 调整批处理流水线（解析+KDF vs I/O）
media-lock encrypt /path/to/videos -p yourpassword --recursive --jobs 4 --queue 5
```

#### 从中断的会话中恢复

```bash
media-lock recover video.mp4
```

#### 交互式密码输入

如果不指定 `--password`，系统会提示您输入：

```bash
media-lock encrypt video.mp4
Enter password: ******
```

### 库使用

#### 基本加密

```rust
use media_lock_core::{EncryptionTask, OperationMode};

let task = EncryptionTask::new("video.mp4".into(), OperationMode::Encrypt)
    .with_password("yourpassword".to_string());

task.run()?;
```

#### 自定义进度处理器

```rust
use media_lock_core::{EncryptionTask, OperationMode, ProgressHandler};
use std::sync::Arc;

struct MyProgress;

impl ProgressHandler for MyProgress {
    fn on_start(&self, total_bytes: u64, message: &str) {
        println!("开始: {} ({} 字节)", message, total_bytes);
    }

    fn on_progress(&self, delta_bytes: u64) {
        println!("已处理 {} 字节", delta_bytes);
    }

    fn on_message(&self, message: &str) {
        println!("状态: {}", message);
    }

    fn on_finish(&self) {
        println!("完成！");
    }

    fn on_error(&self, err: &media_lock_core::AppError) {
        println!("错误: {}", err);
    }
}

let task = EncryptionTask::new("video.mp4".into(), OperationMode::Encrypt)
    .with_password("yourpassword".to_string())
    .with_handler(Arc::new(MyProgress));

task.run()?;
```

#### 高级配置

```rust
use media_lock_core::{EncryptionTask, OperationMode};

let task = EncryptionTask::new("video.mp4".into(), OperationMode::Encrypt)
    .with_password("yourpassword".to_string())
    .with_audio(true)               // 加密音频轨道
    .with_metadata_scrub(true)      // 清除元数据
    .with_no_wal(false);            // 启用 WAL（默认）

task.run()?;
```

#### 获取性能统计

```rust
use media_lock_core::{EncryptionTask, OperationMode, run_task_with_stats};

let task = EncryptionTask::new("video.mp4".into(), OperationMode::Encrypt)
    .with_password("yourpassword".to_string());

let stats = run_task_with_stats(&task)?;

println!("总时间: {:?}", stats.total_time);
println!("加密吞吐量: {:.2} MB/s", stats.crypto_throughput_mbps());
println!("I/O 吞吐量: {:.2} MB/s", stats.io_throughput_mbps());
println!("I 帧数量: {}", stats.iframe_count);
```

## 加密策略

### I-Frame 优先方法

Media Lock 默认采用 I-Frame 优先的加密策略：

| 内容类型 | 默认加密 | 描述 |
|---------|---------|------|
| I 帧（关键帧） | 始终加密 | 完整的图像数据 |
| P 帧/B 帧 | 不加密 | 与关键帧的差异 |
| 音频 | 可选（默认关闭） | 音频轨道数据 |

此策略提供：
- **可预览性**：加密的视频仍然可以部分查看（P/B 帧可见）
- **高性能**：比加密整个文件快得多
- **平衡性**：对最重要的数据提供良好的安全性，性能影响最小

要加密所有视频和音频数据：

```bash
media-lock encrypt video.mp4 -p yourpassword --encrypt-audio
```

### 原位操作

Media Lock 直接在原文件上加密，无需创建临时副本。这种方法：
- 节省磁盘空间（无需重复文件）
- 减少 I/O 开销
- 适用于大文件

加密过程对每个区域使用 Go 风格的模式：
```
读取 -> 加密 -> 回退定位 -> 写入
```

## 崩溃恢复

### WAL 机制

Media Lock 使用两阶段预写日志（WAL）来保证崩溃安全：

```
┌─────────────────────────────────────────────────────────────┐
│ 阶段 1：备份（在任何修改之前）                             │
├─────────────────────────────────────────────────────────────┤
│ 1. 创建 WAL 文件                                             │
│ 2. 将所有区域流式写入 WAL（顺序写入）                       │
│ 3. 写入条目计数和 CRC                                       │
│ 4. 同步 WAL（对所有数据进行单次 fsync）                      │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ 阶段 2：加密（原位）                                        │
├─────────────────────────────────────────────────────────────┤
│ 对每个区域（按偏移量排序）：                                 │
│   读取 -> 加密 -> 回退定位 -> 写入                           │
│ 5. 同步加密数据（单次 fsync）                                │
│ 6. 追加/删除 footer                                         │
│ 7. 清理 WAL                                                 │
└─────────────────────────────────────────────────────────────┘
```

如果发生崩溃：
- **阶段 1 完成前**：原文件完好，WAL 被忽略
- **阶段 2 期间**：WAL 包含所有原始数据，自动恢复

### 恢复过程

如果之前的会话失败（WAL 存在），下一个操作将：
1. 检测不完整的会话
2. 从 WAL 恢复原始数据
3. 继续请求的操作

手动恢复：

```bash
media-lock recover video.mp4
```

## 性能

Media Lock 针对高性能加密进行了优化：

- **流式 WAL**：使用大缓冲区（8MB）的顺序写入
- **最小化同步**：每个文件仅 2 次 fsync 操作（WAL + 加密数据）
- **直接 I/O**：使用 4MB 缓冲区的原位加密
- **单次解析**：高效的容器扫描
- **I-Frame 优先**：减少 70-90% 的加密数据量

性能特点：
- **吞吐量随磁盘速度扩展**：大多数操作为 I/O 限制
- **内存高效**：无论文件大小如何，内存使用恒定
- **可扩展**：高效处理多 GB 文件
- **批处理流水线**：解析+KDF（`--jobs`）与 I/O 重叠，使用 `--queue` 限制缓冲

## 安全设计

### 密码学组件

- **加密**：AES-256-CTR 模式
  - 使用 8 字节 nonce 支持大文件
  - 可对任何区域进行随机访问解密
  - 无状态，可并行化

- **密钥派生**：Argon2id
  - 内存硬化 KDF（64 MB，3 次迭代，4 个并行通道）
  - OWASP 推荐参数
  - 每个文件 16 字节随机盐值

- **随机数**：用于盐值和 nonce 的加密安全随机数生成器

### 文件格式

加密的文件包含 73 字节的 footer：

```
偏移量  大小    字段
------  ------  -----
0       8       魔数: "RUST_ENC"
8       1       版本
9       16      盐值（用于密钥派生）
25      8       Nonce（用于 AES-CTR）
33      8       原始文件长度
41      32      校验和（原始数据采样）
```

### 安全考虑

- **无密钥存储**：密码永不存储，只保存盐值
- **暴力破解防护**：Argon2id 内存硬化 KDF
- **元数据清除**：可选择删除敏感元数据
- **重放防护**：每次加密使用唯一的盐值和 nonce

## 配置选项

| 选项 | 简写 | 描述 | 默认值 |
|------|------|------|--------|
| `--password` | `-p` | 加密/解密密码 | 如果省略则提示输入 |
| `--encrypt-audio` | | 加密音频轨道 | false |
| `--scrub-metadata` | | 清除敏感元数据 | false |
| `--recursive` | `-r` | 递归处理目录 | false |
| `--no-wal` | | 禁用 WAL（更快但不安全） | false |
| `--jobs` | | 并行规划线程数（解析 + KDF，仅批处理生效） | 自动（≤4） |
| `--queue` | | I/O 前可领先的规划任务数（仅批处理生效） | 5 |

### 性能与安全

- **使用 WAL**（默认）：崩溃安全，稍慢（约 2 次同步）
- **不使用 WAL**（`--no-wal`）：更快，但崩溃时文件可能损坏

仅在以下情况下使用 `--no-wal`：
- 您有原始文件的备份
- 系统稳定且配备 UPS
- 您接受崩溃时数据丢失的风险

## 开发

### 构建

```bash
# 调试构建
cargo build

# 发布构建
cargo build --release

# 运行测试
cargo test

# 使用语言特性构建
cargo build --features en  # 英文（默认）
cargo build --features zh  # 中文
cargo build --features gui-support  # 未来 GUI 支持
```

### 项目结构

```
media-lock/
├── src/
│   ├── bin/main.rs       # CLI 入口
│   ├── lib.rs            # 库入口
│   ├── common.rs         # 领域模型和类型
│   ├── workflow.rs       # 核心编排
│   ├── crypto/
│   │   ├── mod.rs
│   │   ├── engine.rs     # AES-256-CTR 实现
│   │   └── key_deriv.rs  # Argon2id 密钥派生
│   ├── parsers/
│   │   ├── mod.rs
│   │   ├── mp4.rs        # MP4/MOV 解析器
│   │   └── mkv.rs        # MKV/WebM 解析器
│   ├── io/
│   │   ├── mod.rs
│   │   ├── wal.rs        # 预写日志
│   │   └── locker.rs     # 文件锁定
│   ├── error.rs          # 错误类型
│   └── i18n.rs           # 国际化
├── tests/
│   └── lib_integration_test.rs
├── doc/                  # 开发文档（设计前）
├── Cargo.toml
└── README.md
```

### 测试

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test test_encrypt_decrypt_roundtrip

# 运行并显示输出
cargo test -- --nocapture

# 运行集成测试
cargo test --test lib_integration_test
```

## 许可证

MIT 许可证 - 详见 LICENSE 文件

## 贡献

欢迎贡献！请确保：
- 代码通过 `cargo clippy` 和 `cargo fmt`
- 为新功能添加测试
- 更新文档

## 致谢

由 VideoEncrypt 团队开发
