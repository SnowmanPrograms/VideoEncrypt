# 04. 加密引擎规范 (Crypto Engine Spec)

## AI 指令
本模块 (`crypto/`) 负责核心密码学运算。
**核心算法**: AES-256-CTR (Counter Mode)。
**特点**:
1.  **流式 (Streaming)**: 密文长度 === 明文长度（无 Padding）。
2.  **随机访问 (Random Access)**: 支持通过物理 Offset 计算 IV。

---

## 1. 密钥派生 (KDF)

使用 **Argon2id** 将用户密码转换为 32字节主密钥。

*   **Params**:
    *   Algorithm: Argon2id
    *   Salt: 16 bytes (随机生成，存储于文件 Header/Footer)
    *   T_Cost: 3 (Iterations)
    *   M_Cost: 64MB (Memory)
    *   P_Cost: 4 (Parallelism)
    *   Output Len: 32 bytes (AES-256 Key)

---

## 2. AES-CTR 计数器计算逻辑

这是实现“原地随机读写”的关键。标准 AES-CTR 使用 `Nonce (12B) || Counter (4B)` 构成 16字节 IV。

### 2.1 状态公式
对于物理文件中的任意字节偏移量 `Global_Offset`：

1.  **Block Index**: `Block_Idx = Global_Offset / 16`
2.  **Intra-Block Offset**: `Rem = Global_Offset % 16`
3.  **IV (Initial Vector)**:
    *   `Nonce`: 12字节 (文件创建时随机生成，存 Header)。
    *   `Counter`: 4字节 (Big Endian)。
    *   `Current_IV = Nonce || (Block_Idx as u32_be)`
    *   *注意*: 标准 CTR 模式在 Counter 溢出时不会进位到 Nonce，本项目文件最大支持 `2^32 * 16字节 = 64GB`。
    *   **扩展支持 (64GB+)**: 如果需要支持超大文件，需将 Nonce 缩减为 8字节，Counter 扩展为 8字节。**本项目建议采用 8B Nonce + 8B Counter** 方案以支持任意大小文件。

### 2.2 8B+8B 方案实现细节
*   `Fixed_Nonce`: `[u8; 8]` (From Header)
*   `Dynamic_Counter`: `u64` (Little Endian or Big Endian, typically BE for CTR standard) = `(Initial_Counter + Block_Idx)`

### 2.3 `process_block` 实现
```rust
fn process_block(&self, global_offset: u64, data: &mut [u8]) {
    // 1. 计算起始 Counter
    let block_idx = global_offset / 16;
    let offset_in_block = (global_offset % 16) as usize;

    // 2. 初始化 AES-CTR
    // 必须从 block 边界开始计算 keystream，即使 global_offset 未对齐
    let mut cipher = Aes256Ctr::new(&self.key, &base_iv(block_idx));
    
    // 3. 处理未对齐的头部 (如果有)
    if offset_in_block > 0 {
        // 生成 keystream 并丢弃前 offset_in_block 个字节
        // 或者：更高效的做法是生成 keystream block，手动异或
        // Rust `ctr` crate 通常会自动处理 seek_ctr，需查阅文档确认
        cipher.seek_ctr(block_idx); 
    }

    // 4. 原地加密/解密 (XOR)
    // 此时 cipher 内部游标可能需要调整
    // 建议：手动生成 Keystream 与 data 进行 XOR，确保完全掌控对齐逻辑
    apply_xor_keystream(data, key, iv, offset_in_block);
}
```

---

## 3. 隐私元数据清洗 (Metadata Scrubbing)

仅针对 `RegionKind::Metadata`。

*   **策略**:
    *   **Scrub (默认)**: 将内容字节全部替换为 `0x00`。这会破坏文本可读性，但保留 Atom 结构，不影响 MP4 解析。
    *   **Obfuscate (可选)**: 使用 AES-CTR 加密。但需注意，如果播放器解析元数据时遇到乱码可能会崩溃（虽不常见）。
    *   **建议**: 针对 `©nam` 等文本字段，替换为 **空格 (0x20)**，以保持 UTF-8 兼容性；针对二进制字段，使用 `0x00`。

---

## 4. 测试用例要求
1.  **`test_kdf_consistency`**: 给定相同 Password + Salt，必须输出相同 Key。
2.  **`test_ctr_random_access`**:
    *   加密数据 `[0..100]`。
    *   独立解密 `data[50..60]`。
    *   验证结果是否等于原始 `data[50..60]`。
3.  **`test_alignment_boundary`**:
    *   测试跨越 16字节边界的数据块处理（例如 Offset=15, Len=3）。
4.  **`test_large_offset`**: 模拟 Offset = 100GB，验证 Counter 计算不溢出/Panic。