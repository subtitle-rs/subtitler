# Subtitler Code Wiki

> 版本: v1.4.0 · Rust Edition 2024 · MSRV 1.85
> 一个用于解析、转换、校验、编辑和生成 13 种字幕格式的高性能 Rust 库 + CLI 工具。

---

## 目录

1. [项目概览](#1-项目概览)
2. [整体架构](#2-整体架构)
3. [目录结构](#3-目录结构)
4. [核心数据模型](#4-核心数据模型)
5. [模块职责详解](#5-模块职责详解)
6. [关键类与函数说明](#6-关键类与函数说明)
7. [依赖关系](#7-依赖关系)
8. [Feature Flags](#8-feature-flags)
9. [构建与运行](#9-构建与运行)
10. [CLI 使用手册](#10-cli-使用手册)
11. [库 API 使用指南](#11-库-api-使用指南)
12. [测试体系](#12-测试体系)
13. [CI 与发布](#13-ci-与发布)
14. [设计决策与约定](#14-设计决策与约定)

---

## 1. 项目概览

`subtitler` 是一个纯 Rust 实现的字幕处理工具集，同时提供:

- **库 (library)**: 可被任何 Rust 项目依赖，用于程序化处理字幕。
- **CLI 二进制**: 名为 `subtitler` 的命令行工具，面向终端用户。

### 支持的 13 种格式

| 领域 | 格式 | 扩展名 | Feature |
|------|------|--------|---------|
| Web | SRT | `.srt` | `srt` |
| Web | WebVTT | `.vtt` | `vtt` |
| Web | TTML/IMSC | `.ttml`, `.xml` | `ttml` |
| Web | SAMI | `.smi`, `.sami` | `sami` |
| 视频编辑 | ASS | `.ass` | `ass` |
| 视频编辑 | SSA | `.ssa` | `ssa` |
| DVD | MicroDVD | `.sub` | `microdvd` |
| DVD | SubViewer | `.sub` | `subviewer` |
| 广播 | SCC | `.scc` | `scc` |
| 广播 | EBU STL | `.stl` | `ebu_stl` |
| YouTube | SBV | `.sbv` | `sbv` |
| 卡拉OK | LRC | `.lrc` | `lrc` |
| 东欧 | MPL2 | `.mpl`, `.txt` | `mpl2` |

### 核心能力

- 解析 / 生成 / 格式转换
- 自动格式检测（基于内容签名）
- 编码自动识别 (UTF-8/UTF-16/BOM/chardetng)
- 富文本提取 (bold / italic / underline / color / voice)
- 帧时间码支持
- 流式解析器 (大文件友好)
- 工具操作: 排序、合并、拆分、校验、帧率转换、时间偏移
- 异步 I/O (基于 `tokio`)
- Serde 序列化 / 反序列化

---

## 2. 整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                        CLI (main.rs)                         │
│                  subtitler <command> [args]                  │
└──────────────┬──────────────────────────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────────────────────────┐
│                     cli.rs (clap 定义)                       │
│   Parse / Convert / Validate / Edit / Info / Detect / ...    │
└──────────────┬──────────────────────────────────────────────┘
               │  调用
               ▼
┌─────────────────────────────────────────────────────────────┐
│                   lib.rs (公共 API 层)                       │
│   parse_bytes / parse_file / parse_url / detect_format       │
└──────┬───────────────────────────────┬──────────────────────┘
       │                               │
       ▼                               ▼
┌─────────────────┐          ┌────────────────────────┐
│   model.rs      │          │   格式模块 (13 个)      │
│  数据模型 +     │◀─────────│  srt/vtt/ass/ttml/...  │
│  SubtitleFormat │          │  每个模块独立 feature   │
│  trait 统一接口 │          └────────────────────────┘
└────┬────────────┘
     │
     ├──── utils.rs     (时间戳解析/格式化)
     ├──── config.rs    (共享正则常量)
     ├──── encoding.rs  (编码检测)
     ├──── error.rs     (结构化错误)
     ├──── types.rs     (AnyResult 别名)
     ├──── normalize.rs (文本规范化)
     └──── quality.rs   (质量分析 + 翻译 trait)
```

### 分层原则

1. **公共 API 层** ([lib.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/lib.rs)): 提供 `parse_bytes`、`parse_file`、`parse_url`、`detect_format`、`parse_bytes_as` 等高层入口，自动路由到具体格式模块。
2. **数据模型层** ([model.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/model.rs)): 定义统一的 `SubtitleFile` 枚举和 `SubtitleFormat` trait，所有格式的编辑方法（sort/merge/validate 等）通过 trait 默认实现共享。
3. **格式实现层** (`srt.rs`, `vtt.rs`, ...): 每个格式一个模块，编译期通过 feature flag 启用/禁用。每个模块对外暴露统一的 `parse_content` / `parse_bytes` / `parse_file` / `to_string` / `generate` / `detect_format` 接口模式。
4. **工具层**: `utils`、`encoding`、`normalize`、`quality` 提供横切关注点支持。
5. **CLI 层** ([main.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/main.rs) + [cli.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/cli.rs)): 用 `clap` 定义子命令，调用库层完成具体功能。

---

## 3. 目录结构

```
subtitler/
├── Cargo.toml              # 包定义、依赖、feature flags、example 声明
├── Cargo.lock
├── README.md               # 用户文档
├── CHANGELOG.md            # 版本变更记录
├── MIGRATION.md            # 0.1.x → 1.x 升级指南
├── AGENTS.md               # 仓库工作流约定（构建/测试命令）
├── SKILL.md                # Skill 描述文件
├── LICENSE                 # Apache-2.0
├── rustfmt.toml            # 2 空格缩进配置
├── dist-workspace.toml     # cargo-dist 发布配置
│
├── src/                    # 源代码
│   ├── lib.rs              # ★ 库根，公共 API
│   ├── main.rs             # ★ CLI 入口
│   ├── cli.rs              # ★ clap 命令定义
│   ├── model.rs            # ★ 核心数据模型 + SubtitleFormat trait
│   ├── utils.rs            # ★ 时间戳工具
│   ├── config.rs           # 共享正则常量
│   ├── encoding.rs         # 编码检测与解码
│   ├── error.rs            # 结构化错误类型
│   ├── types.rs            # AnyResult 类型别名
│   ├── normalize.rs        # 文本规范化
│   ├── quality.rs          # 质量报告 + Translator trait
│   │
│   ├── srt.rs              # 格式: SRT
│   ├── vtt.rs              # 格式: WebVTT
│   ├── ass.rs              # 格式: ASS/SSA (共享)
│   ├── microdvd.rs         # 格式: MicroDVD
│   ├── subviewer.rs        # 格式: SubViewer
│   ├── ttml.rs             # 格式: TTML/IMSC (用 quick-xml)
│   ├── sbv.rs              # 格式: SBV
│   ├── lrc.rs              # 格式: LRC
│   ├── sami.rs             # 格式: SAMI
│   ├── mpl2.rs             # 格式: MPL2
│   ├── scc.rs              # 格式: SCC (CEA-608)
│   └── ebu_stl.rs          # 格式: EBU STL (二进制)
│
├── examples/               # 23+ 使用示例 (每个 [[example]] 在 Cargo.toml 声明)
├── benches/                # criterion 性能基准
│   └── subtitler_benchmark.rs
├── tests/                  # 集成测试
│   ├── integration.rs
│   ├── cross_format.rs     # 跨格式转换测试
│   ├── arch_unification.rs
│   ├── cleanup_batch.rs
│   └── proptest.rs         # 属性测试
│
├── docs/                   # 内部设计与分析文档
└── .github/workflows/      # CI: rust.yml + release.yml
```

---

## 4. 核心数据模型

### 4.1 `Subtitle` — 单条字幕

定义于 [model.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/model.rs#L44-L62)。

```rust
pub struct Subtitle {
  pub index: Option<usize>,            // 字幕序号
  pub start: u64,                      // 起始时间（毫秒）
  pub end: u64,                        // 结束时间（毫秒）
  pub text: String,                    // 纯文本（已剥离标签）
  pub settings: Option<String>,        // VTT cue 设置
  pub text_parts: SmallVec<[TextPart; 4]>, // 富文本结构化分段
  pub style: Option<String>,           // ASS/SSA 样式名
  pub actor: Option<String>,           // ASS/SSA 说话人
  pub is_comment: bool,                // ASS Comment 行
}
```

**设计要点**:
- 时间一律用**毫秒** (`u64`)，不是秒。
- `text_parts` 用 `SmallVec<[TextPart; 4]>`：大多数字幕 ≤4 个样式段，可栈分配避免堆开销。
- 提供 builder 风格的 `with_index` / `with_style` / `with_settings`。

### 4.2 `TextPart` — 富文本片段

```rust
pub struct TextPart {
  pub text: String,
  format: TextFormat,        // bitflags: BOLD | ITALIC | UNDERLINE
  pub color: Option<String>, // 颜色（来自 <font color=...>）
  pub voice: Option<String>, // VTT 说话人（<v Alice>）
}
```

`TextFormat` 使用 `bitflags` 宏定义，把 bold/italic/underline 压缩进一个 `u8`，每个 `TextPart` 节省 2~7 字节内存。提供 `bold()` / `set_bold()` 等访问器保持 API 兼容。

### 4.3 `SubtitleFile` — 字幕文件（多态枚举）

```rust
pub enum SubtitleFile {
  Srt(Vec<Subtitle>),
  Vtt { header: Option<String>, subtitles: Vec<Subtitle> },
  Ass(AssData),
  Ssa(AssData),
  MicroDvd { fps: f64, subtitles: Vec<Subtitle> },
  SubViewer { header: Option<String>, subtitles: Vec<Subtitle> },
  Ttml { header: Option<String>, subtitles: Vec<Subtitle> },
  Sbv(Vec<Subtitle>),
  Lrc(Vec<Subtitle>),
  Sami(SamiData),
  Mpl2(Vec<Subtitle>),
  Scc(SccData),
  EbuStl(Box<EbuStlData>),  // 体积大，用 Box
}
```

每个变体在编译期由对应 feature flag 控制 (`#[cfg(feature = "srt")]` 等)。EBU STL 因含 1024 字节 GSI 块，使用 `Box` 减小枚举体积。

### 4.4 `SubtitleFormat` trait — 统一操作接口

定义于 [model.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/model.rs#L583)。

```rust
pub trait SubtitleFormat: Debug + Clone + Send + Sync {
  // 必须 实现（每格式变体各自实现）
  fn subtitles(&self) -> &[Subtitle];
  fn subtitles_mut(&mut self) -> &mut Vec<Subtitle>;
  fn format(&self) -> Format;
  fn to_string_with_format(&self, format: &Format) -> String;

  // 默认实现 —— 所有格式免费获得
  fn to_string(&self) -> String;
  fn shift_all(&mut self, offset_ms: i64);
  fn map<F: FnMut(&mut Subtitle)>(self, f: F) -> Self;
  fn filter<F: FnMut(&Subtitle) -> bool>(self, f: F) -> Self;
  fn sort(&mut self);
  fn validate(&self) -> Vec<ValidationIssue>;
  fn validate_extended(&self, max_chars, max_gap_ms, max_cps) -> Vec<ValidationIssue>;
  fn merge_adjacent(&mut self, max_gap_ms: u64);
  fn remove_overlaps(&mut self);
  fn enforce_min_duration(&mut self, min_ms: u64);
  fn enforce_max_duration(&mut self, max_ms: u64);
  fn auto_extend_for_cps(&mut self, max_cps: f64);
  fn extract_range(&self, start_ms: u64, end_ms: u64) -> Vec<Subtitle>;
  fn split_long(&mut self, max_chars: usize);
  fn transform_framerate(&mut self, in_fps: f64, out_fps: f64);
}
```

**这是整个库的关键抽象**: 编辑/校验逻辑只写一次，13 种格式全部复用。这是通过 trait 默认方法 + `subtitles()` / `subtitles_mut()` 两个必需访问器实现的。

> ⚠ 使用时需 `use subtitler::SubtitleFormat;` 把 trait 方法引入作用域。

### 4.5 `Format` 枚举

```rust
pub enum Format { Srt, Vtt, Ass, Ssa, MicroDvd, SubViewer,
                  Ttml, Sbv, Lrc, Sami, Mpl2, Scc, EbuStl }
```

用于格式检测和转换的目标指定。

### 4.6 `ValidationIssue` — 校验问题

```rust
pub enum ValidationIssue {
  Overlap { index_a, index_b, end_a, start_b },
  NegativeDuration { index, start, end },
  ZeroDuration { index, time },
  DecreasingStartTime { index, prev_start, curr_start },
  TooLongGap { index, prev_end, curr_start, gap_ms },
  TextTooLong { index, chars, max_chars },
  CpsTooHigh { index, cps, max_cps },
}
```

每个变体携带结构化上下文，`.description()` 方法返回人类可读字符串。

### 4.7 辅助类型

| 类型 | 说明 |
|------|------|
| `AssStyle` | ASS/SSA 样式定义（23 字段） |
| `AssData` | `Ass` / `Ssa` 变体共享：`info` + `styles` + `subtitles` |
| `Timestamp` | `{ start, end, settings }` 三元组 |
| `WritePolicy` | `Overwrite` / `RefuseIfExists` / `Append` |
| `ParseConfig` | 解析行为配置（builder API） |
| `SubtitleFileBuilder` | 流式构建 `SubtitleFile` |
| `StreamingParser` trait | 流式解析器统一接口 |

---

## 5. 模块职责详解

### 5.1 [lib.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/lib.rs) — 库根

**职责**: 模块声明 + 高层 API + 格式自动路由。

**核心函数**:
- `detect_format(&[u8]) -> Option<Format>` — 顺序尝试每个启用格式模块的 `detect_format`。
- `parse_bytes(&[u8]) -> Result<SubtitleFile>` — 自动检测 + 解析。
- `parse_bytes_as(&[u8], Format) -> Result<SubtitleFile>` — 按指定格式解析。
- `parse_file(path)` / `parse_url(url)` / `parse_url_with(url, client)` — 异步 I/O 入口（`http` feature 控制 URL 支持）。

### 5.2 [model.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/model.rs) — 数据模型中心

**职责**: 定义全部核心类型 + `SubtitleFormat` trait 默认实现 + `SubtitleFileBuilder` + `ParseConfig` + `StreamingParser` trait + 帧转换工具。

**关键自由函数**:
- `ms_to_frames(ms, fps) -> u64`
- `frames_to_ms(frames, fps) -> u64`
- `parse_ass_color(str) -> (r, g, b, a)`
- `format_ass_color(r, g, b, a) -> String`

### 5.3 格式模块（13 个）

每个格式模块 (`srt.rs`, `vtt.rs`, ...) 遵循**统一的对外 API 模式**:

| 函数 | 说明 |
|------|------|
| `parse_content(&str) -> AnyResult<...>` | 同步从字符串解析（核心） |
| `parse_bytes(&[u8]) -> AnyResult<...>` | 从字节解析（自动解码） |
| `parse_file(path)` | 异步从文件解析 |
| `parse_url(url)` | 异步从 URL 解析（需 `http`） |
| `to_string(&[Subtitle], ...) -> String` | 序列化为字符串 |
| `generate(&[Subtitle], path, policy)` | 异步写入文件 |
| `write_stream(&[Subtitle], &mut W)` | 异步流式写入 |
| `detect_format(&[u8]) -> Option<Format>` | 格式签名检测 |
| `parse_stream(content) -> impl Iterator` | 流式解析器（部分格式） |

**返回类型差异**:
- 简单格式（SRT/VTT/SBV/LRC/MPL2）: 返回 `Vec<Subtitle>`。
- 富格式（ASS/SAMI/SCC/EBU STL/MicroDVD/SubViewer/TTML）: 返回 `SubtitleFile` 或自定义数据结构，保留 header/styles/fps 等元信息。

**特殊实现**:
- `ass.rs` 同时处理 ASS 和 SSA（共用 `AssData`，仅 `format()` tag 不同）。
- `ttml.rs` 是唯一依赖 `quick-xml` 的模块（XML 解析）。
- `ebu_stl.rs` 是唯一的二进制格式（GSI 1024 字节 + TTI 128 字节块）。
- `scc.rs` 实现 CEA-608 字符集与 SMPTE timecode。

### 5.4 [utils.rs](file:////Users/mankong/volumes/code/subtitle-rs/subtitler/src/utils.rs) — 时间戳工具

**关键函数**:
- `parse_timestamp(ts) -> AnyResult<u64>` — 高速手动解析 `hh:mm:ss[,.]mmm`，失败时回退到正则。支持 SRT/VTT 单/双位小时。
- `parse_timestamps(line)` — 解析 `start --> end [settings]` 行，返回 `Timestamp`。
- `format_timestamp(ms, options)` — 格式化为 SRT (`,`) 或 WebVTT (`.`) 分隔符。
- `pad_left(value, length)` — 零填充。

### 5.5 [config.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/config.rs) — 共享正则常量

仅 2 行：`RE_TIMESTAMP` 和 `RE_TIMESTAMPS` 字符串常量，被 `utils.rs` 编译为 `LazyLock<Regex>`。

### 5.6 [encoding.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/encoding.rs) — 编码处理

**函数**:
- `detect_encoding(&[u8]) -> &'static str` — 检测顺序: BOM(UTF-8/16) → UTF-8 验证 → chardetng 启发式。
- `decode_to_string(&[u8]) -> AnyResult<String>` — 检测 + 解码，支持 UTF-8/UTF-16BE/UTF-16LE 及通过 `encoding_rs` 解码 GBK/Shift_JIS/Big5 等。
- `try_decode_for_detection(&[u8]) -> Option<String>` — 用于格式检测，永不返回 Err。

### 5.7 [error.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/error.rs) — 错误类型

提供两层错误:
- `ParseError`: 高层 API 错误（`UnknownFormat` / `Unsupported(Format)` / `Anyhow` / `Decode` / `Io` / `Http`）。
- `SubtitleError`: 细粒度解析错误（`InvalidTimestamp` / `UnexpectedLine` / `InvalidUtf8` / `Io`）。

通过 `thiserror` 自动实现 `Display` + `From` 转换，可与现有 `AnyResult` 互操作。

### 5.8 [normalize.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/normalize.rs) — 文本规范化

| 函数 | 作用 |
|------|------|
| `normalize_whitespace` | 折叠多空格、去尾空格、限制连续换行 |
| `normalize_quotes` | 智能引号 → ASCII |
| `normalize_punctuation` | 修标点前空格、折叠重复标点、合并省略号 |
| `fix_ocr_errors` | 修正常见 OCR 错误（`rn→m`, `l→1`, `O→0`） |
| `strip_hearing_impaired` | 移除听障标签 `(LAUGHS)` / `[APPLAUSE]` / `♪` / 说话人标签 |
| `optimize_line_breaks` | 在自然边界智能断行 |
| `normalize_text` / `normalize_subtitle` | 组合规范化 |

### 5.9 [quality.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/quality.rs) — 质量分析

- `SubtitleQuality`: 单条字幕指标（CPS、WPM、字数、问题列表等）。
- `QualityReport`: 文件级汇总（总数、平均 CPS/WPM、问题总数）。
- `generate_report(subs, max_chars, max_gap, max_cps)` — 生成报告。
- `Translator` trait + `DummyTranslator` — 翻译服务抽象（可对接任何翻译 API）。

### 5.10 [main.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/main.rs) + [cli.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/cli.rs) — CLI

- `cli.rs`: 用 `clap` derive 定义 `Cli` / `Commands` 枚举 / 各子命令的 `*Args` 结构体 / `Format` 枚举（含 `from_ext` 扩展名映射）。
- `main.rs`: `#[tokio::main]` 入口，dispatch 到 `cmd_parse` / `cmd_convert` / `cmd_validate` / `cmd_edit` / `cmd_info` / `cmd_detect` / `cmd_quality` / `cmd_normalize` / `cmd_shift` 九个命令处理函数。

**辅助函数**:
- `read_input(input)` — 支持 `-` (stdin) / `http(s)://` / 本地路径，返回 `(bytes, ext_hint)`。
- `resolve_format(data, hint)` — hint 优先，否则调用 `detect_format`。
- `resolve_output_format(output, hint)` — hint 优先，否则从扩展名推断。

---

## 6. 关键类与函数说明

### 6.1 高层入口（最常用）

```rust
// 自动检测格式并解析（最常用）
subtitler::parse_bytes(&data)?           // 同步
subtitler::parse_file("a.srt").await?    // 异步，自动检测
subtitler::parse_url("https://...").await?  // 需 http feature

// 指定格式解析
subtitler::parse_bytes_as(&data, Format::Vtt)?
```

### 6.2 Subtitle 关键方法

| 方法 | 签名 | 说明 |
|------|------|------|
| `new` | `(start, end, &str) -> Self` | 构造 |
| `shift` | `(&mut self, offset_ms: i64)` | 时间偏移，负值钳为 0 |
| `duration_ms` | `() -> u64` | `end - start`（饱和减） |
| `chars_per_second` | `() -> f64` | 用 `plaintext()`，不含标签 |
| `reading_speed_wpm` | `() -> f64` | 字/分钟 |
| `is_empty` | `() -> bool` | 文本是否仅空白 |
| `strip_tags` | `(&mut self)` | 原地移除 HTML/ASS 标签 |
| `plaintext` | `() -> String` | 去标签 + ASS 转义转换，有快速路径 |

### 6.3 SubtitleFormat trait 方法（通过 `&mut SubtitleFile` 调用）

```rust
file.sort();
file.shift_all(500);
file.merge_adjacent(300);
file.split_long(42);
file.remove_overlaps();
file.transform_framerate(23.976, 25.0);
file.validate();
file.validate_extended(42, 5000, 25.0);
file.auto_extend_for_cps(25.0);
file.extract_range(1000, 5000);
file.map(|s| { s.shift(100); });
file.filter(|s| !s.is_empty());
let s: String = file.to_string();                    // 原格式输出
let s: String = file.to_string_with_format(&Format::Vtt); // 转换格式输出
```

### 6.4 SubtitleFileBuilder（流式构建）

```rust
use subtitler::model::{SubtitleFileBuilder, Subtitle, Format};

let file = SubtitleFileBuilder::new(Format::Srt)
  .add_subtitle(Subtitle::new(0, 5000, "Hello"))
  .add_subtitle(Subtitle::new(6000, 10000, "World"))
  .build()
  .unwrap();
```

支持 `with_fps` / `with_header` / `add_style` / `add_styles` / `add_subtitles`。MicroDVD 必须传 `fps`，否则 `build()` 返回 `None`。

### 6.5 流式解析

```rust
use subtitler::model::StreamingParser;

let mut parser = subtitler::srt::parse_stream(content);
while let Some(result) = parser.next() {
  let sub = result?;
  // 处理单条
}
// 或一次性收集
let all = parser.collect_all()?;
```

支持流式解析的格式: SRT、VTT、ASS、MicroDVD、SubViewer、SBV、LRC、SAMI、MPL2、SCC。（TTML / EBU STL 因 XML/二进制结构限制为同步整体解析。）

---

## 7. 依赖关系

### 7.1 运行时依赖

| 依赖 | 版本 | 用途 | 可选 |
|------|------|------|------|
| `anyhow` | 1 | 错误处理（`AnyResult`） | 否 |
| `bitflags` | 2 | `TextFormat` 位标志 | 否 |
| `chardetng` | 1 | 编码启发式检测 | 否 |
| `clap` | 4 | CLI 参数解析（derive） | 否 |
| `encoding_rs` | 0.8 | 多编码解码（GBK/JIS/Big5） | 否 |
| `quick-xml` | 0.41 | TTML XML 解析 | ✅ (ttml) |
| `regex` | 1 | 时间戳/标签/规范化正则 | 否 |
| `reqwest` | 0.13 | HTTP 客户端（rustls） | ✅ (http) |
| `serde` | 1 | 序列化（derive） | 否 |
| `serde_json` | 1 | JSON 输出 | 否 |
| `smallvec` | 1 | `SmallVec<[TextPart; 4]>` | 否 |
| `thiserror` | 2 | 错误类型 derive | 否 |
| `tokio` | 1 | 异步运行时（fs/io-util/rt/macros） | 否 |
| `tracing` | 0.1 | 日志门面 | 否 |
| `tracing-subscriber` | 0.3 | 日志订阅器 | 否 |

### 7.2 开发依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| `criterion` | 0.8 | 性能基准（`benches/`） |
| `proptest` | 1.5 | 属性测试（`tests/proptest.rs`） |

### 7.3 内部模块依赖图

```
            ┌──────────┐
            │  lib.rs  │ ───── re-exports ──────┐
            └────┬─────┘                        │
                 │ mod                          ▼
   ┌─────────────┼─────────────────────────┐  model types
   │   │   │   │   │   │   │   │   │   │   │
  srt vtt ass ... (13 格式)               error types
   │   │   │                             encoding
   └─┬─┴───┴─── 都依赖 ──→  model  ←──── utils
                                           │
                                          config (共享正则)
```

- **所有格式模块** 都 `use crate::model::{...}` 和 `use crate::utils::{...}`。
- **格式模块之间不互相依赖**（保持独立可裁剪）。
- **CLI 依赖库层** 通过 `use subtitler::{...}`。

---

## 8. Feature Flags

在 [Cargo.toml](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/Cargo.toml#L61-L77) 定义:

```toml
[features]
default = ["srt", "vtt", "ass", "ssa", "microdvd", "subviewer",
           "ttml", "sbv", "lrc", "sami", "mpl2", "scc", "ebu_stl", "http"]
```

| Feature | 启用内容 | 额外依赖 |
|---------|----------|----------|
| `srt` / `vtt` / `ass` / `ssa` / `microdvd` / `subviewer` / `sbv` / `lrc` / `sami` / `mpl2` / `scc` / `ebu_stl` | 对应格式模块 + `Format`/`SubtitleFile` 变体 | 无 |
| `ttml` | TTML 模块 | `quick-xml` |
| `http` | `parse_url` / `parse_url_with` | `reqwest` |

**精简编译**:

```toml
[dependencies]
subtitler = { version = "1.4", default-features = false, features = ["srt", "vtt"] }
```

每个 feature 通过 `#[cfg(feature = "xxx")]` 控制模块声明、`Format` 枚举变体、`SubtitleFile` 枚举变体、所有 `match` 分支，确保未启用的格式完全从编译产物中移除。

---

## 9. 构建与运行

### 9.1 环境要求

- Rust 1.85+（Edition 2024）
- 单 crate 仓库，所有 `cargo` 命令在 [subtitler/](file:///Users/mankong/volumes/code/subtitle-rs/subtitler) 目录下执行。

### 9.2 常用命令

```sh
# 构建
cargo build --verbose

# 构建 CLI 二进制（release 优化）
cargo build --release

# 运行测试（全部目标）
cargo test --all-targets

# 格式检查（2 空格缩进，见 rustfmt.toml）
cargo fmt -- --check

# Lint（警告视为错误）
cargo clippy -- -D warnings

# 运行 CLI
cargo run -- parse examples/example.srt
cargo run -- convert examples/example.srt output.vtt

# 运行指定 example（HTTP example 需显式启用 feature）
cargo run --example parse-srt-file
cargo run --example parse-srt-http --features="http"

# 性能基准
cargo bench

# 精简构建（关闭非必需格式）
cargo build --no-default-features --features "srt,vtt"
```

### 9.3 Release Profile

[Cargo.toml](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/Cargo.toml#L79-L84) 配置了体积优化:

```toml
[profile.release]
codegen-units = 1   # 单 CGU，更好优化
lto = true           # 全 LTO
opt-level = "z"      # 体积优先
panic = "abort"      # 不生成 unwind 表
strip = true         # 去 symbol
```

---

## 10. CLI 使用手册

### 10.1 `parse` — 解析并展示

```sh
subtitler parse movie.srt                  # 终端展示
subtitler parse movie.vtt --json           # JSON 输出
subtitler parse https://example.com/a.srt  # 从 URL
cat movie.srt | subtitler parse -          # 从 stdin
subtitler parse data.txt --format srt      # 强制格式
```

### 10.2 `convert` — 格式转换

```sh
subtitler convert input.srt output.vtt                    # 自动检测+推断
subtitler convert input.srt output.ass --from srt --to ass
subtitler convert input.srt output.vtt --shift -500       # 同时偏移
subtitler convert input.srt -                              # 输出到 stdout
```

### 10.3 `validate` — 校验

```sh
subtitler validate movie.srt                              # 基础时序校验
subtitler validate movie.srt --max-chars 42 --max-gap 5000 --max-cps 25
subtitler validate movie.srt --basic                      # 仅时序，不查文本
subtitler validate movie.srt --json
```

退出码: 有问题时返回 `1`。

### 10.4 `edit` — 编辑变换

```sh
subtitler edit input.srt --output output.srt --sort
subtitler edit input.srt --output output.srt --shift 500
subtitler edit input.srt --output output.srt --merge 300
subtitler edit input.srt --output output.srt --split 42
subtitler edit input.srt --output output.vtt --sort --shift -300 --merge 100  # 组合
subtitler edit input.srt --output output.srt --transform-fps 23.976 25.0
```

至少需要一个操作，否则报错。

### 10.5 `info` — 文件统计

```sh
subtitler info movie.srt
```

输出格式、条数、时长范围、平均/最小/最大时长、总字数、最大 CPS、时序问题数。

### 10.6 `detect` — 检测格式

```sh
subtitler detect unknown.sub   # 输出: srt / vtt / ass / ...
```

### 10.7 `quality` — 质量报告

```sh
subtitler quality movie.srt [--json] [--max-chars N] [--max-gap MS] [--max-cps N]
```

### 10.8 `normalize` — 文本规范化

```sh
subtitler normalize input.srt --output out.srt --all       # 全部规范化
subtitler normalize input.srt --output out.srt --strip-hi --fix-ocr --quotes --whitespace
```

### 10.9 `shift` — 时间偏移

```sh
subtitler shift input.srt --output out.srt 500             # 延迟 500ms
subtitler shift input.srt --output out.srt -- -200         # 提前 200ms
```

---

## 11. 库 API 使用指南

### 11.1 解析（推荐高层 API）

```rust
use subtitler::SubtitleFormat; // 必须 import 才能用 subtitles() / validate()

// 自动检测
let data = std::fs::read("subtitle.srt")?;
let file = subtitler::parse_bytes(&data)?;
println!("{} subtitles, format: {:?}", file.subtitles().len(), file.format());
```

### 11.2 按具体格式解析（低层）

```rust
use subtitler::srt;
let subs = srt::parse_content("1\n00:00:01,000 --> 00:00:03,500\nHi\n\n")?;
```

### 11.3 格式转换

```rust
use subtitler::{SubtitleFormat, model::Format};

let file = subtitler::parse_file("input.srt").await?;
let vtt_str = file.to_string_with_format(&Format::Vtt);
std::fs::write("output.vtt", vtt_str)?;
```

### 11.4 生成文件

```rust
use subtitler::model::Subtitle;
use subtitler::srt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let subs = vec![
    Subtitle::new(1000, 3500, "Hello!"),
    Subtitle::new(4000, 6500, "World!"),
  ];
  srt::generate(&subs, "output.srt", None).await?;
  Ok(())
}
```

### 11.5 流式写入（大文件）

```rust
use subtitler::srt;
use tokio::io::AsyncWriteExt;

let mut file = tokio::fs::File::create("out.srt").await?;
srt::write_stream(&subs, &mut file).await?;
file.flush().await?;
```

### 11.6 编辑操作链

```rust
use subtitler::SubtitleFormat;

let mut file = subtitler::parse_file("in.srt").await?;
file.sort();
file.shift_all(-200);
file.merge_adjacent(300);
file.split_long(42);
file.remove_overlaps();
file.auto_extend_for_cps(25.0);
std::fs::write("out.srt", file.to_string())?;
```

---

## 12. 测试体系

### 12.1 测试分布

- **单元测试**: 各 `src/*.rs` 的 `#[cfg(test)] mod tests`（共 124 个）。
- **集成测试**: [tests/](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/tests) 目录（共 92 个）:
  - [integration.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/tests/integration.rs) — 端到端流程
  - [cross_format.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/tests/cross_format.rs) — 跨格式转换矩阵
  - [proptest.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/tests/proptest.rs) — 属性测试
  - [arch_unification.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/tests/arch_unification.rs) — 架构统一性
  - [cleanup_batch.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/tests/cleanup_batch.rs) — 清理批处理
- **总测试数**: 216。

### 12.2 运行

```sh
cargo test --verbose           # 全部
cargo test --all-targets       # 含 benches/examples/tests
cargo test --test cross_format # 单个集成测试
cargo test -- --nocapture      # 显示 println! 输出
```

### 12.3 基准测试

[benches/subtitler_benchmark.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/benches/subtitler_benchmark.rs) 使用 `criterion`，通过 `cargo bench` 运行。

---

## 13. CI 与发布

### 13.1 CI 流程 ([.github/workflows/rust.yml](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/.github/workflows/rust.yml))

1. `cargo fmt -- --check`
2. `cargo clippy -- -D warnings`
3. `cargo build --verbose` + `cargo test --verbose`，配合 feature 矩阵（default / `--no-default-features` + 各组合）。

### 13.2 发布流程 ([.github/workflows/release.yml](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/.github/workflows/release.yml))

使用 **cargo-dist** 自动化，由 git tag 触发:

1. 打 tag (如 `v1.4.0`)。
2. CI 构建 release artifact（用 `[profile.dist]` 配置）。
3. 自动创建 GitHub Release。
4. crates.io 发布需手动: `cargo publish`。

### 13.3 版本规范

遵循 [Semantic Versioning](https://semver.org/):

- 新增格式 / 新公共 API → **minor**
- 优化 / bug 修复 → **patch**
- 破坏性变更 → **major**（v2.0 计划做零拷贝解析等重构）

记录于 [CHANGELOG.md](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/CHANGELOG.md)。

---

## 14. 设计决策与约定

### 14.1 时间单位

**全库统一使用毫秒 (`u64`)**，不是秒。这是 SRT/VTT 的原生精度，避免浮点误差。帧格式（MicroDVD/MPL2/SCC/EBU STL）通过 `ms_to_frames` / `frames_to_ms` 转换。

### 14.2 同步 vs 异步

- **解析核心是同步的** (`parse_content`) — 不做真实 I/O，零开销。
- **文件 / URL / 流式写入是异步的** (`parse_file` / `parse_url` / `generate` / `write_stream`) — 基于 `tokio`。
- TTML/EBU STL 的流式写入因 `quick-xml` / 二进制结构限制为同步。

### 14.3 写入策略

`generate()` 函数通过 `WritePolicy` 控制行为:
- `Overwrite`（默认）: `OpenOptions::write(true).truncate(true)`
- `RefuseIfExists`: 目标存在则报错
- `Append`: 追加

> ⚠ 默认覆写，不是追加。

### 14.4 编码处理

所有 `parse_bytes` 入口都先经 `encoding::decode_to_string`，自动处理 BOM / UTF-16 / GBK / Shift_JIS / Big5 等，调用方无需关心编码。

### 14.5 富文本提取

SRT/VTT/ASS 解析时会:
1. 把标签内的文本拼接到 `text` 字段（纯文本）。
2. 同时把带样式的片段填入 `text_parts: SmallVec<[TextPart; 4]>`。

调用方可选择用 `text`（简单）或 `text_parts`（保留样式）。

### 14.6 性能优化点

- `LazyLock<Regex>` 全局缓存正则，避免重复编译。
- `parse_timestamp` 优先走手动字节扫描快速路径，正则仅作回退。
- `plaintext()` 检测无 `<`/`{`/`\\` 时直接 clone，跳过正则。
- `SmallVec` 减少 `TextPart` 堆分配。
- `bitflags` 把三个 bool 压成 1 字节。
- `generate()` 内部流式写入，不构造完整字符串。

### 14.7 代码风格

- **2 空格缩进**（[rustfmt.toml](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/rustfmt.toml)），非 Rust 默认 4 空格。
- `cargo clippy -D warnings` 必须零警告。
- 错误优先用 `anyhow::Result`（别名 `AnyResult`），公共 API 边界可用 `thiserror` 类型化错误。

### 14.8 CLI 入口

- `subtitler file <path>` / `subtitler url <url>` 是早期形式。
- 现版本子命令为 `parse` / `convert` / `validate` / `edit` / `info` / `detect` / `quality` / `normalize` / `shift`。
- 格式自动检测: 内容签名优先，扩展名作 hint。

---

## 附录: 快速链接

- [README.md](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/README.md) — 用户面向文档
- [CHANGELOG.md](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/CHANGELOG.md) — 版本历史
- [MIGRATION.md](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/MIGRATION.md) — 0.1.x 升级指南
- [AGENTS.md](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/AGENTS.md) — 仓库工作流
- [crates.io: subtitler](https://crates.io/crates/subtitler)
- [docs.rs: subtitler](https://docs.rs/subtitler)
- [GitHub: subtitle-rs/subtitler](https://github.com/subtitle-rs/subtitler)
