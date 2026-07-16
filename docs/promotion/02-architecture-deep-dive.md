# subtitler 架构设计：如何用 Trait 统一 9 种字幕格式

字幕格式之多令人头疼——SRT、VTT、ASS、TTML、LRC... 每种有自己的语法、时间戳格式、样式系统。`subtitler` 1.0 的核心挑战是：**如何用一套 API 统一处理所有格式，同时保持每种格式的特有数据不丢失？**

本文深入解析 subtitler 的架构设计决策。

## 问题：格式碎片化

先看各格式的差异有多大：

```
SRT:     纯文本，逗号毫秒分隔   "00:01:00,000"
VTT:     类 HTML 标签，点分隔    "00:01:00.000"
ASS:     ini 式段落，厘秒        "0:01:00.00"
TTML:    XML，秒或时间码         "60.5s" 或 "00:01:00.500"
LRC:     方括号时间戳             "[01:00.50]"
MicroDVD: 帧号                   "{1500}{1800}"
SBV:     逗号分隔时间+文本        "0:01:00.000,0:01:03.000,text"
```

时间戳格式不同、样式表达不同（HTML tag vs ASS override vs XML attribute vs 无样式）、结构不同（有些有 header/metadata，有些没有）。

## 第一层：统一数据模型

核心是一个 enum，每个变体代表一种格式，保留各自的特有数据：

```rust
pub enum SubtitleFile {
    Srt(Vec<Subtitle>),
    Vtt { header: Option<String>, subtitles: Vec<Subtitle> },
    Ass(AssData),                              // 含 info + styles
    Ssa(AssData),
    MicroDvd { fps: f64, subtitles: Vec<Subtitle> },  // 保留 fps！
    SubViewer { header: Option<String>, subtitles: Vec<Subtitle> }, // 保留 header！
    Ttml { header: Option<String>, subtitles: Vec<Subtitle> },
    Sbv(Vec<Subtitle>),
    Lrc(Vec<Subtitle>),
}
```

**关键决策：不把所有格式降级成 `Vec<Subtitle>`。** 

之前的版本把 MicroDVD 和 SubViewer 都塞进 `Srt(Vec<Subtitle>)`——结果是 fps 和 header 元数据丢失，round-trip 后文件损坏。1.0 给每种格式独立变体，数据完整保留。

`Subtitle` 是所有格式共享的通用结构：

```rust
pub struct Subtitle {
    pub start: u64,       // 毫秒（统一时间单位）
    pub end: u64,
    pub text: String,
    pub text_parts: Vec<TextPart>,  // 带样式的文本片段
    // ASS 特有字段（其他格式为 None）
    pub style: Option<String>,
    pub actor: Option<String>,
    // ...
}
```

时间统一为 **毫秒 (`u64`)**——不暴露帧号、厘秒、秒等格式特定单位给用户。

## 第二层：SubtitleFormat Trait

有了数据模型，下一个问题是：如何让 `validate()`、`shift_all()`、`merge_adjacent()` 这些通用操作自动适用于所有格式？

答案是 trait + 默认方法：

```rust
pub trait SubtitleFormat: Debug + Clone + Send + Sync {
    // 4 个必需方法——每种格式各实现
    fn subtitles(&self) -> &[Subtitle];
    fn subtitles_mut(&mut self) -> &mut Vec<Subtitle>;
    fn format(&self) -> Format;
    fn to_string_with_format(&self, fmt: &Format) -> String;

    // 15 个默认方法——通过 subtitles() 工作，所有格式免费获得
    fn shift_all(&mut self, offset_ms: i64) { /* 默认实现 */ }
    fn validate(&self) -> Vec<ValidationIssue> { /* 默认实现 */ }
    fn merge_adjacent(&mut self, max_gap_ms: u64) { /* 默认实现 */ }
    fn split_long(&mut self, max_chars: usize) { /* 默认实现 */ }
    // ... 还有 sort, filter, map, enforce_min_duration 等
}
```

**为什么这样设计？** 这 15 个编辑方法只依赖 `subtitles()` / `subtitles_mut()`——它们操作的是通用的 `Vec<Subtitle>`，不关心格式特有的字段。所以可以在 trait 里给出默认实现，所有变体自动获得。

加新格式的成本从"改 25 个 match 分支"降到了"实现 4 个方法"。

## 第三层：格式检测与统一入口

用户拿到一个文件，怎么知道是哪种格式？

```rust
pub fn detect_format(data: &[u8]) -> Option<Format> {
    // 按特征链式检测：
    // 1. WEBVTT 前缀 → Vtt
    // 2. <tt + ttml namespace → Ttml
    // 3. [Script Info] → Ass/Ssa
    // 4. {N}{N} 帧号 → MicroDvd
    // 5. [mm:ss.xx] → Lrc
    // 6. H:MM:SS.mmm,... → Sbv
    // 7. --> 存在 → Srt (兜底)
}
```

检测顺序精心设计：先匹配高特异性格式（TTML 的 XML namespace、VTT 的 WEBVTT 前缀），最后才用宽泛规则（`-->` → SRT）。

统一入口隐藏格式细节：

```rust
// 用户不需要知道是哪种格式
let file = subtitler::parse_bytes(&data)?;
// file 是 SubtitleFile enum，format() 告诉你实际检测到的格式
```

## 第四层：Feature Flag 门控

每种格式一个 Cargo feature，编译期裁剪：

```rust
#[cfg(feature = "srt")]
pub mod srt;
#[cfg(feature = "ttml")]
pub mod ttml;
// ...

pub enum Format {
    #[cfg(feature = "srt")] Srt,
    #[cfg(feature = "ttml")] Ttml,
    // ...
}
```

**挑战：`#[cfg]` 在 enum 变体 + match 臂上的组合。** 禁用某格式时，enum 变体不存在，所有 match 臂都要对应 cfg-gate。`Ass | Ssa` 这种 or-pattern 不能直接挂 `#[cfg]`——必须拆成独立分支。

这是整个项目中最繁琐的部分，但保证了 `--features srt,vtt` 能编译出只支持两种格式的精简二进制。

## 第五层：Sync 解析 + Async I/O

早期版本所有 parser 都是 `async`——但 SRT/VTT 的 `.await` 实际作用在 `Cursor<&str>` 上，是假 async（没有真正的 I/O 挂起）。

1.0 的分离：

```
parse_content(&str) → sync    （纯 CPU，无 I/O）
parse_file(path)    → async   （真文件 I/O）
parse_url(url)      → async   （真网络 I/O）
```

好处：解析核心更快（无 async 状态机开销），更容易组合，测试不需要 `#[tokio::test]`。

## 第六层：错误处理

分层错误类型：

```rust
// 底层：结构化解析错误
pub enum SubtitleError {
    InvalidTimestamp(String),
    UnexpectedLine { row, expected, got },
    InvalidUtf8(...),
    Io(...),
}

// 高层：统一入口错误
pub enum ParseError {
    UnknownFormat,           // 检测失败
    Unsupported(Format),     // feature 未启用
    Decode(SubtitleError),   // 解析错误
    Io(...),
    Http(...),
}
```

向后兼容：保留 `AnyResult<T> = Result<T, anyhow::Error>` 别名，旧代码不受影响。

## 性能设计

1. **手写时间戳解析器**——最热路径不走 regex，直接字节扫描
2. **`LazyLock` regex 缓存**——所有 regex 编译一次，零运行时开销
3. **`SrtStream` 流式迭代器**——大文件逐条 yield，不分配 Vec
4. **quick-xml 拉式解析**——TTML 不构建 DOM 树

## 架构总结

```
┌─────────────────────────────────────────────┐
│              统一入口 parse_bytes            │
│         (自动检测 → 路由到对应格式)           │
├─────────────────────────────────────────────┤
│           SubtitleFormat trait               │
│    (validate / shift / merge / split / ...)  │
├──────┬──────┬──────┬──────┬─────────────────┤
│ SRT  │ VTT  │ ASS  │ TTML │ ... (9 formats) │
│module│module│module│module│                  │
├──────┴──────┴──────┴──────┴─────────────────┤
│     Feature flags (每格式独立裁剪)            │
├─────────────────────────────────────────────┤
│  quality (报告) · normalize (文本) · encoding │
└─────────────────────────────────────────────┘
```

这个架构的回报：加 SBV 和 LRC 两个格式时，每个只花了不到 15 分钟——建模块、加变体、实现 4 个方法、加 feature flag。架构的代价已经在前期的统一工作中付清了。

---

*本文涉及的代码全部在 [github.com/subtitle-rs/subtitler](https://github.com/subtitle-rs/subtitler)，Apache-2.0 许可。*
