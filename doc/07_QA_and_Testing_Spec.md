# 07. 测试与质量保证计划 (QA & Testing Spec)

## AI 指令
本模块定义了项目的测试策略。
请在编写代码时，同步创建 `tests/` 目录下的集成测试文件。

---

## 1. 测试分层策略

### 1.1 单元测试 (Unit Tests)
*   **位置**: 各个 `src/` 模块内部 (`#[cfg(test)]`).
*   **职责**: 测试函数级的逻辑（如 AES-CTR 计数器计算、Region 切分算法）。
*   **覆盖率目标**: 核心算法 > 90%。

### 1.2 库集成测试 (Library Integration Tests)
*   **位置**: `tests/lib_integration_test.rs`
*   **职责**: 调用 `media_lock_core` 的 API，不经过 CLI。
*   **Mocking**: 需要实现一个 `MockProgressHandler` 来验证回调是否被正确触发。
    ```rust
    struct MockHandler {
        log: Arc<Mutex<Vec<String>>>,
    }
    // Assert: log 包含 "on_start", "on_progress" ...
    ```

### 1.3 端到端黑盒测试 (E2E System Tests)
*   **位置**: `tests/cli_e2e_test.rs` 或外部脚本。
*   **职责**: 编译出 Binary，通过 Shell 调用，验证最终文件结果。

---

## 2. 端到端验证流程 (E2E Workflow)

这是验证系统可用性的最终标准。开发完成后需编写 Python 或 Shell 脚本自动执行。

**测试脚本逻辑**:

1.  **环境准备**:
    *   `cargo build --release --features en`
    *   准备测试目录 `temp_test/`

2.  **生成源文件**:
    *   创建 10MB 随机数据文件 `source.bin`。
    *   **伪造容器**: 在 `source.bin` 的头部写入伪造的 MP4 Atom (`moov`, `trak`...)。无需真实视频编码，只要符合 Parser 的 `box_type` 检查即可。
    *   计算原始 Hash: `HASH_ORIG = sha256sum(source.bin)`

3.  **加密测试**:
    *   执行: `./target/release/media-lock-cli encrypt source.bin -p 123456`
    *   **验证 1 (Magic)**: 检查文件末尾是否包含 "RUST_ENC"。
    *   **验证 2 (Hash)**: `sha256sum(source.bin)` 必须 **不等于** `HASH_ORIG`。
    *   **验证 3 (Size)**: 文件大小应增加 (Footer 长度)。

4.  **灾难恢复测试 (模拟)**:
    *   *注: 这需要特殊的 Test Flag 或 Mock，难以在纯黑盒中测试，建议在库集成测试中覆盖。*

5.  **解密测试**:
    *   执行: `./target/release/media-lock-cli decrypt source.bin -p 123456`
    *   **验证 4 (还原)**: `sha256sum(source.bin)` 必须 **等于** `HASH_ORIG`。
    *   **验证 5 (Size)**: 文件大小必须完全恢复。

6.  **错误密码测试**:
    *   加密后，使用 `-p wrongpass` 解密。
    *   预期: 进程退出码非 0，stderr 输出包含错误信息。

---

## 3. 性能基准测试 (Benchmarks)
使用 `criterion` crate。
*   **Parser Bench**: 解析 1GB MP4 索引的时间。
*   **Crypto Bench**: AES-NI 加密吞吐量 (MB/s)。