# 02. 媒体容器解析规范 (Parser Spec)

## AI 指令
本模块负责实现 `parsers/` 下的具体逻辑。
目标是**精准**且**高效**地提取视频文件中的加密目标区域（主要是 I 帧）。
请严格遵循 "Zero-DOM" 原则：**绝不**构建完整的文件对象树，仅读取必要的索引 Atom/Element。

---

## 1. 通用技术要求
1.  **Trait 实现**: 所有解析器必须实现 `crate::common::ContainerParser`。
2.  **IO 模式**: 使用 `std::io::BufReader` 配合 `Seek`，严禁一次性 `read_to_end`。
3.  **大端序 (Big-Endian)**: MP4 和 MKV (EBML) 默认均为大端序，请使用 `byteorder::BigEndian` 或 `binrw` 处理。
4.  **性能约束**: 解析 10GB 文件的索引耗时应在 **1秒** 以内。

---

## 2. MP4 解析器规范 (`parsers/mp4.rs`)

MP4 (ISOBMFF) 将索引与数据分离。核心任务是定位 `moov` -> `trak` 并计算物理偏移。

### 2.1 关键 Atom/Box 路径
需按顺序解析以下 Box，忽略其他所有 Box (特别是 `mdat`)：
1.  Root -> `moov` (Movie)
2.  `moov` -> `trak` (Track, 可能有多个)
3.  `trak` -> `mdia` -> `minf` -> `stbl` (Sample Table)
4.  **核心索引表** (位于 `stbl` 下):
    *   `stss` (Sync Sample Box): **关键帧**列表 (Sample ID)。
    *   `stsc` (Sample To Chunk): Sample 到 Chunk 的映射 (RLE 压缩)。
    *   `stsz` (Sample Size): 每个 Sample 的大小。
    *   `stco` (Chunk Offset): 32位 Chunk 偏移量。
    *   `co64` (Chunk Offset 64): 64位 Chunk 偏移量 (大文件必选)。

### 2.2 核心算法: I 帧物理偏移计算
MP4 的索引查找是间接的，必须实现以下多步映射算法：

**输入**: `stss` 列表 (I 帧的 Sample ID)。
**输出**: `Vec<Region>` (Offset + Length)。

**步骤**:
1.  **构建 Sample Map**:
    *   遍历 `stsc` 表，展开 RLE 压缩，建立 `Sample ID -> Chunk ID` 和 `Samples Per Chunk` 的映射。
    *   *注意*: `stsc` 表通常很小，但在内存中展开所有 Sample 可能会很大，建议使用迭代器或二分查找逻辑。
2.  **定位 Chunk**:
    *   使用 `stco` 或 `co64` 获取 `Chunk ID -> File Offset (Base)`。
3.  **计算 Sample 偏移**:
    *   如果 `Samples Per Chunk == 1` (常见情况)，则 `Region.offset = Chunk_Offset`。
    *   如果 `Samples Per Chunk > 1`，则需要累加该 Chunk 内先于目标 Sample 的所有 Sample 的大小 (`stsz`)。
    *   `Target_Offset = Chunk_Offset + Sum(Sizes of previous samples in this chunk)`。

### 2.3 音频处理 (可选)
如果 `EncryptionConfig.encrypt_audio` 为 `true`：
1.  在 `trak` -> `mdia` -> `hdlr` 中检查 `handler_type`。
    *   `'vide'`: 视频轨道 (处理 I 帧)。
    *   `'soun'`: 音频轨道。
2.  对于音频轨道，**忽略 `stss`** (所有音频帧均视为需加密)，直接通过 `stsc` + `stsz` + `stco` 计算所有 Sample 的区域。

### 2.4 隐私元数据 (Metadata)
扫描以下路径，生成 `RegionKind::Metadata`：
*   `moov` -> `udta` -> `meta` -> `ilst` (iTunes 风格元数据)。
*   匹配 Tag: `©nam` (Title), `©too` (Tool), `©xyz` (Location)。

---

## 3. MKV 解析器规范 (`parsers/mkv.rs`)

MKV 基于 EBML (Extensible Binary Meta Language)。结构是流式的，与 MP4 不同，MKV 通常将索引 (`Cues`) 放在文件末尾，但数据块 (`SimpleBlock`) 本身包含关键帧标记。

### 3.1 EBML 基础工具
必须实现 **VINT (Variable Size Integer)** 读取器：
*   读取首字节，确定前导零的个数 N。
*   读取后续 N 个字节，组合成整数。

### 3.2 遍历逻辑
MKV 的解析是线性的（除非去读 SeekHead，但顺序扫描更稳健）：

1.  **EBML Header**: 验证 DocType = `matroska` 或 `webm`。
2.  **Segment**: 主要容器。
3.  **Tracks**: 读取 `TrackEntry`。
    *   记录 `TrackNumber` 与 `TrackType` (Video=1, Audio=2) 的映射。
4.  **Cluster** (核心数据区):
    *   包含时间戳和数据块。
    *   **重点关注 Element**: `SimpleBlock` (ID: `0xA3`)。

### 3.3 SimpleBlock 解析与判定
`SimpleBlock` 结构：
`[ID (0xA3)] [Size (VINT)] [TrackNumber (VINT)] [Timecode (i16)] [Flags (u8)] [Data ...]`

**关键帧判定逻辑**:
1.  读取 `TrackNumber`，确认是否为视频轨道。
2.  读取 `Flags` (u8)。
3.  **Keyframe Flag**: `Flags & 0x80 == 0x80` (即最高位为 1)。
4.  如果为 I 帧：
    *   `Region.offset` = 当前文件指针位置 (数据开始处)。
    *   `Region.len` = `SimpleBlock Size` - `Header Size` (TrackNum len + 2 + 1)。

*(注：MKV 也支持 `BlockGroup`，但现代文件绝大多数使用 `SimpleBlock`。V1 版本可仅支持 `SimpleBlock`，遇到 `BlockGroup` 记录日志并跳过)*

---

## 4. 接口与测试定义

### 4.1 Mock 数据策略
由于不需要真实解码视频，测试数据可以使用 `ffmpeg` 生成极小的文件：
*   `ffmpeg -f lavfi -i testsrc=duration=1:size=1280x720:rate=1 -c:v libx264 -g 1 -f mp4 test.mp4`
    *   `-g 1`: 强制每帧都是 I 帧 (All Intra)，便于验证 parser 是否提取了所有帧。

### 4.2 单元测试清单 (Unit Tests)

#### MP4 Tests
1.  **`test_mp4_probe`**: 识别有效 MP4 和无效文件。
2.  **`test_find_stss`**: 验证能正确找到关键帧列表。
3.  **`test_calc_offset_complex`**:
    *   构造一个 `stsc` 包含 `2 samples per chunk` 的场景。
    *   验证计算出的 `Region.offset` 是否等于 `Chunk Offset + Sample 1 Size`。
4.  **`test_ignore_mdat`**: 确保解析器没有读取 `mdat` 内容 (通过 Mock IO 的 read 计数器验证)。

#### MKV Tests
1.  **`test_vint_decoding`**: 验证 EBML 变长整数解析正确性。
2.  **`test_simple_block_flag`**: 模拟一段二进制流，包含 `SimpleBlock`，验证是否能提取 Keyframe bit。

---

## 5. 安全性与边界情况
1.  **整数溢出**: 计算 `offset + len` 时必须使用 `checked_add`，防止溢出导致 Panic 或错误回绕。
2.  **EOF 处理**: 如果 Box 长度声明超过文件实际长度，应返回错误，而不是 Panic。
3.  **无限循环**: 解析 Atom 树时，需检测嵌套深度或 Box Size = 0 的异常情况 (MP4 Spec 规定 Size=0 代表延伸到 EOF，Size=1 代表 Extended Size)。