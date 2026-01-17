# 05. 核心业务调度规范 (Core Workflow Spec)

## AI 指令
本模块 (`src/workflow.rs`) 负责实现 `EncryptionTask::run()` 的具体逻辑。
它是整个 Library 的心脏，负责将 Parser, IO, Crypto 模块串联起来。
**核心原则**: 
1.  **阻塞式执行**: 可以在任何线程调用，但会阻塞直到完成。
2.  **UI 无关**: 只通过 `ProgressHandler` 回调通知状态，不直接打印日志。
3.  **Panic Free**: 所有错误必须转换为 `AppError` 返回。

---

## 1. 主要执行流程 (`run_task`)

该函数是 `EncryptionTask::run` 的底层实现。

```rust
// 伪代码逻辑
pub fn run_task(task: &EncryptionTask) -> Result<()> {
    let path = &task.input_path;
    let handler = task.handler.as_deref().unwrap_or(&NoOpProgress); // 获取回调
    
    // 1. 初始化检查
    handler.on_message(t!("status_checking")); // I18n: "正在检查文件状态..."
    
    // Check 1: 文件锁
    let mut locker = LockManager::acquire(path)?; 
    
    // Check 2: 灾难恢复
    if WalManager::needs_recovery(path) {
        // 如果处于 Recover 模式，则继续；否则报错提示用户
        if task.config.operation != OperationMode::Recover {
            return Err(AppError::PreviousSessionFailed);
        }
        handler.on_message(t!("status_recovering"));
        WalManager::recover(path)?;
    }
    
    // 2. 准备文件与解析
    let mut file = OpenOptions::new().read(true).write(true).open(path)?;
    
    // Check 3: Magic Header 检查 (避免重复加密/解密)
    let file_state = detect_file_state(&file)?;
    validate_state(file_state, task.config.operation)?;

    // 3. 解析结构 (Parser)
    handler.on_message(t!("status_analyzing"));
    let parser = detect_parser(path)?;
    let regions = parser.scan_regions(&file)?;
    
    // 4. 计算总工作量并通知 UI
    let total_bytes: u64 = regions.iter().map(|r| r.len as u64).sum();
    handler.on_start(total_bytes, t!("status_processing"));

    // 5. 分块处理 (Batching)
    // 将 Region 列表切分为约 16MB 的 Batch，以平衡内存和 WAL 性能
    let batches = chunk_regions(regions, 16 * 1024 * 1024);
    
    let mut wal = WalManager::new(path);
    let mut engine = CryptoEngine::new(task.config.key);

    // 6. 核心循环 (The Loop)
    for batch in batches {
        // A. WAL Write (Critical)
        wal.begin_batch(&mut file, &batch)?;
        
        // B. Read & Encrypt in RAM
        let mut data = read_batch_data(&file, &batch)?;
        // 根据 RegionKind 决定是 Encrypt 还是 Scrub
        engine.process_regions(&batch, &mut data, task.config.scrub_metadata);
        
        // C. Write Back
        write_batch_data(&mut file, &batch, &data)?;
        file.sync_all()?; // 物理落盘
        
        // D. Commit
        wal.commit_batch()?;
        
        // E. Update UI
        let batch_bytes = batch.iter().map(|r| r.len as u64).sum();
        handler.on_progress(batch_bytes);
    }
    
    // 7. 终结状态 (Finalize)
    handler.on_message(t!("status_finalizing"));
    match task.config.operation {
        OperationMode::Encrypt => append_footer(&mut file, &task.config)?,
        OperationMode::Decrypt => remove_footer(&mut file)?,
        _ => {}
    }

    // 8. 释放锁
    locker.release()?;
    handler.on_finish();
    
    Ok(())
}
```

---

## 2. 辅助逻辑
*   **`detect_file_state`**: 读取文件末尾 32 字节，检查 Magic Number。
*   **`chunk_regions`**: 算法函数。输入 `Vec<Region>`，输出 `Vec<Vec<Region>>`。确保每个 Batch 的总字节数不超过阈值，且不拆分单个 Region。