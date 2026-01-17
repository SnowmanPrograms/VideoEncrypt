# 03. IO 安全与灾难恢复规范 (IO & Journaling Spec)

## AI 指令
本模块 (`io/`) 负责所有物理磁盘操作。
**核心原则**：绝不信任文件系统写入是原子的。必须通过 WAL (Write-Ahead Logging) 机制确保 `ACID` 特性。
**禁止**: 在没有 `.lock` 保护和 `.wal` 备份的情况下直接修改原文件。

---

## 1. 文件锁与状态机 (`io/locker.rs`)

### 1.1 锁文件定义
在目标文件同目录下创建 `.lock` 文件。内容为 JSON 格式，便于人工调试。
**文件名**: `{filename}.lock` (例如 `movie.mp4.lock`)

```rust
// 对应 src/io/locker.rs
#[derive(Serialize, Deserialize, Debug)]
pub struct LockState {
    pub session_id: String,       // UUID v4
    pub target_file: PathBuf,     // 绝对路径
    pub operation: OperationMode, // Encrypt/Decrypt
    pub timestamp: u64,           // UNIX timestamp
    pub stage: ProcessStage,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ProcessStage {
    Initializing,
    Processing { current_offset: u64 }, // 进度标记
    Finalizing,                         // 正在写入 Header/Footer
}
```

### 1.2 锁生命周期管理
1.  **Acquire**: 程序启动检查 -> 无锁 -> 创建锁 (Stage: Initializing)。
2.  **Update**: 每处理一个 Batch，更新 `current_offset` (可选，仅用于进度恢复，非强一致性要求，因为 WAL 才是数据真理)。
3.  **Release**: 任务完全成功 -> 删除锁。

---

## 2. 滚动预写日志 (`io/wal.rs`)

采用 **Rolling Batch WAL** 策略：只备份当前正在处理的 Batch，处理完即丢弃。

### 2.1 WAL 文件格式 (Binary)
**文件名**: `{filename}.wal`

| 偏移 (Byte) | 类型 | 描述 |
| :--- | :--- | :--- |
| 00-03 | `u32` (BE) | Magic: `0x57414C31` ("WAL1") |
| 04-07 | `u32` (BE) | Batch Entry Count (N) |
| **Header End** | | |
| **Entry 1** | | |
| +00 | `u64` (BE) | Original File Offset |
| +08 | `u32` (BE) | Data Length (L) |
| +12 | `[u8; L]` | **Original Raw Data** (Backup) |
| ... | ... | ... |
| **Entry N** | | |
| EOF | `u32` (BE) | Checksum (CRC32 of entire file) |

### 2.2 核心操作流程 (Atomic Write)
必须严格遵循顺序：

1.  **Prepare**: 内存中准备好一个 Batch 的 `Vec<Region>`。
2.  **Backup (Critical)**:
    *   读取原文件对应 Region 的数据。
    *   写入 `.wal` 文件。
    *   **Call `fsync` (必须!)**: 确保 WAL 落盘。
3.  **Payload Processing**:
    *   在内存中对数据进行 AES-CTR 处理。
4.  **In-Place Write**:
    *   通过 `mmap` 或 `File::seek+write` 将处理后的数据写回原文件。
    *   **Call `flush/fsync` (必须!)**: 确保修改落盘。
5.  **Commit**:
    *   `File::set_len(0)` 清空 `.wal` 文件（避免反复创建文件的开销）。

### 2.3 恢复逻辑 (Recovery)
系统启动时调用 `WalManager::recover_if_needed(path)`。

1.  **Check**: 是否存在 `.lock` 或 `.wal` (大小 > 0)？
2.  **Validate**: 读取 `.wal`，校验 CRC32。如果校验失败（WAL 写入未完成），则忽略（因为原文件还没被动过）。
3.  **Rollback**:
    *   解析 WAL 中的 `(Offset, Data)`。
    *   打开原文件，Seek 到 Offset，写入 Data。
    *   `fsync`。
4.  **Clean**: 删除 `.wal`，保留 `.lock`（以便上层应用决定是继续还是报错）。

---

## 3. 测试用例要求
1.  **`test_wal_format`**: 写入数据，断言二进制结构符合规范。
2.  **`test_crash_simulation`**:
    *   创建 Mock 文件 `[0, 0, 0, 0]`。
    *   Step 1: 写入 WAL `[0, 0, 0, 0]`。
    *   Step 2: 修改文件为 `[1, 1, 1, 1]`。
    *   Step 3: 模拟“重启”，运行 `recover()`。
    *   Assert: 文件变回 `[0, 0, 0, 0]`。
3.  **`test_corrupted_wal`**: 写入一半的 WAL (CRC 错误)，运行 recover，断言原文件未被修改。