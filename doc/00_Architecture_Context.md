# 00. 架构设计与技术约束 (Architecture Context)

## 1. 项目概述
本项目旨在实现一个**原地（In-Place）、高性能、抗断电**的媒体文件加密系统。
核心目标是在不破坏文件容器结构（MP4/MKV）、不增加文件体积的前提下，对视频关键帧（I-Frame）及敏感元数据进行流式加密。

## 2. 核心设计原则
1.  **原地操作 (In-Place)**: 直接修改物理文件，避免产生临时副本（解决磁盘空间不足问题）。
2.  **零拷贝 (Zero-Copy)**: 利用 `mmap` 技术，在文件系统与加密引擎间传输数据时避免内存复制。
3.  **灾难恢复 (Crash Safety)**: 采用 **Rolling Batch WAL (预写日志)** 机制，保证在任何时刻断电，文件均可恢复至一致状态。
4.  **接口抽象 (Trait-Based)**: 解析层（Parser）与执行层（Engine）解耦，支持 MP4、MKV 等多种容器格式。

## 3. 技术栈选型 (Rust)
所有生成的代码必须遵循以下依赖版本与规范：

*   **语言版本**: Rust 2021 Edition (Stable)
*   **IO 层**:
    *   `memmap2`: 用于高性能内存映射读写。
    *   `std::fs`: 基础文件操作。
*   **加密层**:
    *   `aes`, `ctr`: AES-256-CTR 实现。
    *   `argon2`: 密码哈希与密钥派生 (KDF)。
    *   `rand`: 生成 Salt 和 Nonce。
*   **解析层 (Parser)**:
    *   **不使用** 通用 DOM 解析库（如 `mp4parse`），避免全量加载。
    *   使用 `byteorder` 或 `binrw` 手写高性能流式解析器。
*   **并发层**:
    *   `rayon`: 用于并行处理数据块（如果 IO 允许）。
*   **交互层**:
    *   `clap`: 命令行参数解析 (Derived pattern)。
    *   `indicatif`: 进度条显示。
    *   `tracing` / `tracing-subscriber`: 结构化日志。
*   **错误处理**:
    *   `thiserror`: 库级别的错误定义。
    *   `anyhow`: 应用级别的错误传递。
*   **序列化**:
    *   `serde`, `serde_json`: 用于 WAL 头部或 Lock 文件的序列化。
*   **I18n**: 使用 `rust-i18n` 或简单的 `lazy_static` 配合 `cfg` 宏实现编译时语言选择。
*   **Features**: 在 `Cargo.toml` 中定义 `features = ["zh", "en", "gui-support"]`。   

## 4. 目录结构规范
```text
src/
├── lib.rs           # 核心，暴露给 GUI 和 CLI 调用
├── workflow.rs      # 核心调度逻辑
├── i18n.rs          # 国际化文本常量定义
├── common.rs        # 核心领域模型 (Domain) -> 对应 01 文档
├── error.rs         # 统一错误定义
├── crypto/          # 加密引擎模块
│   ├── mod.rs
│   ├── engine.rs    # AES-CTR 逻辑
│   └── key_deriv.rs # Argon2 KDF
├── io/              # IO 安全模块
│   ├── mod.rs
│   ├── wal.rs       # Rolling WAL 实现
│   └── locker.rs    # 文件锁与恢复逻辑
├── parsers/         # 容器解析模块
│   ├── mod.rs
│   ├── mp4.rs       # MP4 ISOBMFF 实现
│   └── mkv.rs       # MKV EBML 实现
└── main.rs          # CLI 入口，只负责解析参数和调用 lib
```

## 5. 关键流程约束
1.  **启动检查**: 必须优先检查 `.lock` 文件。如果存在，进入 Recovery 模式。
2.  **原子写入**: `Read Original -> Write WAL -> Sync -> Encrypt RAM -> Write File -> Flush -> Clear WAL`。
3.  **音频处理**: 默认**不**加密音频，除非显式配置。
4.  **MKV 支持**: MKV 解析需处理 EBML 变长整数 (VINT) 并识别 `SimpleBlock` 的 Keyframe 标志。