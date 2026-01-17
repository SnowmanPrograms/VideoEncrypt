# IO & Journaling Specification v2.0

本规范描述了 `media-lock` 的 I/O 安全机制，包括文件锁定和流式 WAL 日志。

## 1. File Locking

### Lock File Format (`.lock`)

```json
{
  "locked_at": "2024-01-15T10:30:00Z",
  "operation": "encrypt",
  "stage": "processing",
  "pid": 12345
}
```

### Lifecycle

1. **Acquire**: 检查 `.lock` 文件是否存在
2. **Update**: 更新 `stage` 字段 (`checking` → `processing` → `finalizing`)
3. **Release**: 删除 `.lock` 文件 (RAII 自动释放)

---

## 2. Streaming WAL v2.0

### 设计原则

- **2-Phase 处理**: Phase 1 顺序写入备份，Phase 2 原地加密
- **最少 Sync**: 仅 2 次 `sync_all()` (WAL 完成时 + 加密完成时)
- **流式写入**: 使用 8MB BufWriter 减少系统调用
- **尾部 CRC**: 确保 WAL 完整性

### WAL 文件格式 (`.wal`)

```
┌─────────────────────────────────┐
│ Header (12 bytes)               │
│  ├─ Magic: "WALV0002" (8B)      │
│  └─ Entry Count (4B BE)         │
├─────────────────────────────────┤
│ Entry 1                         │
│  ├─ Offset (8B BE)              │
│  ├─ Length (4B BE)              │
│  └─ Data (N bytes)              │
├─────────────────────────────────┤
│ Entry 2...N                     │
├─────────────────────────────────┤
│ Footer (4 bytes)                │
│  └─ CRC32 (全文件校验)          │
└─────────────────────────────────┘
```

### 处理流程

```
┌─ Phase 1: WAL Creation ─────────────────────────────┐
│ for region in sorted_regions:                       │
│     seek(region.offset)                             │
│     read original data                              │
│     append to WAL (buffered)                        │
│ flush buffer                                        │
│ write CRC footer                                    │
│ sync_all()  ← 唯一一次 WAL sync                     │
└────────────────────────────────────────────────────┘
                         ↓
┌─ Phase 2: In-place Encryption ──────────────────────┐
│ for region in sorted_regions:                       │
│     seek(region.offset)                             │
│     read → encrypt → seek_back → write              │
│     (Go-style pattern, 无 per-region sync)          │
│ sync_all()  ← 唯一一次文件 sync                     │
└────────────────────────────────────────────────────┘
                         ↓
┌─ Cleanup ───────────────────────────────────────────┐
│ delete WAL file                                     │
│ release lock                                        │
└────────────────────────────────────────────────────┘
```

### 崩溃恢复

| 崩溃时机 | WAL 状态 | 原文件状态 | 恢复动作 |
|---------|---------|-----------|---------|
| Phase 1 | 不完整 (CRC 失败) | 未修改 | 删除 WAL |
| Phase 2 | 完整 (CRC 通过) | 部分损坏 | 从 WAL 全量回滚 |

### 性能对比

| 指标 | v1.0 | v2.0 | 提升 |
|------|------|------|------|
| Sync 次数 | 22次 | **2次** | 91% ↓ |
| I/O 时间 | 985ms | **~570ms** | 42% ↓ |
| 总时间 | 1.26s | **~775ms** | 38% ↓ |

---

## 3. --no-wal 模式

当使用 `--no-wal` 标志时：
- 跳过 Phase 1 (无 WAL 备份)
- Phase 2 直接加密
- 仅最后一次 `sync_all()`
- ⚠️ 断电可能导致文件损坏