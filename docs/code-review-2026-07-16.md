# subtitler 代码审查报告

> 审查对象：`/Users/mankong/volumes/code/subtitle-rs/subtitler` (v1.0.0, edition 2024)
> 范围：全部 `src/`、`tests/`、`benches/`、`examples/`、`Cargo.toml`、CLI
> 重点：架构设计、健壮性、API 一致性、错误处理、性能、测试覆盖
> 立场：仅提出问题与建议，不修改任何代码

---

## 0. 总体评价

`subtitler` 是一个**覆盖面很广、模块化清晰**的 Rust 字幕解析库。亮点：

- 9 种格式支持（SRT / VTT / ASS / SSA / MicroDVD / SubViewer / TTML / SBV / LRC），通过统一的 `SubtitleFile` 枚举 + `SubtitleFormat` trait 提供一致的编辑/校验 API。
- 大量的功能扩展：validate / merge / split / shift / framerate / quality / normalize / 翻译接口。
- 编译期功能裁剪（`--no-default-features`）做得不错。
- 关键路径（`parse_timestamp`）使用了手写 byte-scan 而非 regex，性能可控。
- 单测与集成测试覆盖 SRT/VTT/ASS 主体路径，benchmarks 覆盖较全。

但同时存在**几个高优先级问题**需要立刻处理，分布在以下方面：

1. **解析器正确性** —— 至少 2 处正则 group 错位、若干边界情况会静默吞错或丢数据；
2. **编码/错误处理不一致** —— 大文件 IO 路径绕过 `encoding` 模块；SSA/ASS 入口行为不一致；`encoding` 模块基本上是"死代码"；
3. **API 表面碎片化** —— `parse_content` 在不同 format 返回类型不同（`Vec<Subtitle>` vs `SubtitleFile` vs `(Option<String>, Vec<Subtitle>)`），破坏一致性；
4. **内存/性能** —— `parse_bytes` 在每个 format 内部都 `data.to_vec()` 一次全量复制；除 SRT 外没有真正的 streaming parser；
5. **测试盲区** —— SBV / LRC / SubViewer / MicroDVD 的 round-trip 与 edge case 覆盖极薄；没有任何 fuzz / property-based 测试。

下面按主题展开。

---

## 1. 架构与设计

### 1.1 ✅ 优点
- `SubtitleFormat` trait 设计良好：四方法 (`subtitles` / `subtitles_mut` / `format` / `to_string_with_format`) 是变体相关的，其余 `shift_all` / `sort` / `validate` / `merge_adjacent` / `split_long` / `transform_framerate` 等通用方法通过 `subtitles_mut` 取得默认实现 —— 避免了为 9 个变体复制粘贴。
- 公共 API 分层合理：`subtitler::{parse_bytes, parse_file, parse_url, detect_format}` 是高层入口；`subtitler::srt` 等是低层专项入口。
- 关键路径 `parse_timestamp`（[utils.rs:15-90](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/utils.rs#L15-L90)）有清晰注释说明 fast-path + regex fallback。

### 1.2 ⚠️ 问题

#### 1.2.1 `Subtitle` 字段膨胀
[model.rs:11-39](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/model.rs#L11-L39) 中 `Subtitle` 聚合了 9 个 `Option<...>` 字段（`index`, `settings`, `text_parts`, `style`, `actor`, `layer`, `margin_l/r/v`, `effect`）。SRT/VTT 这类轻量格式 99% 的实例这些字段都是 `None`/`Vec::new()`，每个 `Subtitle` 实例都白白承担 9 个 tag + Option discriminant 的空间成本。
- **建议**：考虑用 `format` 相关的扩展 trait（`AsAssFields { style, actor, ... }`）或者把 ASS-only 字段下沉到一个 sidecar `BTreeMap<String, String>`（类似 ASS 的 unknown info 行）。
- 影响：`Vec<Subtitle>` 在 SRT 文件下的内存占用比纯 `Vec<(u64, u64, String)>` 多 ~80-120 字节/项，10k 行字幕多约 1MB。

#### 1.2.2 `SubtitleFile` 变体对 `to_string_with_format` 的 fallback
[model.rs:764-812](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/model.rs#L764-L812) 的 `to_string_with_format` 在 source 和 target 不一致时（如 `SubtitleFile::Srt` → `Format::MicroDvd`）会 fallback 到默认 fps：
```rust
let fps = match self {
    SubtitleFile::MicroDvd { fps, .. } => Some(*fps),
    _ => None,  // 非 MicroDvd 源转 MicroDvd 时 fps 丢失
};
```
- **建议**：在跨格式转换中应当报错或显式提供 fps 入口。

#### 1.2.3 `concatenate` 仅作为 inherent 方法
[model.rs:838-848](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/model.rs#L838-L848) 把 `concatenate` 放在 inherent impl 而非 trait。文档解释是"避免 trait gymnastics"，但这造成 trait 方法和 inherent 方法混杂，调用方需要明确 `use subtitler::model::SubtitleFile;` 才能找到它。其它 editing 方法都走 trait，这个例外破坏了模式一致性。

#### 1.2.4 `ass_to_string_impl` 的"双重特征"分发
[model.rs:819-832](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/model.rs#L819-L832) 通过 `#[cfg(any(feature = "ass", feature = "ssa"))]` 暴露，但在 `#[cfg(feature = "ass")]` 关闭时如果只有 `ssa`，行为会从 `SubtitleFile::Ssa` 取 info/styles；但 v4+ Style 的 format line 仍然是 V4+ —— SSA (v4) 与 ASS (v4+) 互转会数据变形而不会报错。

#### 1.2.5 `lib.rs::detect_format` 风格不统一
[lib.rs:30-51](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/lib.rs#L30-L51) 用 `cfg` 控制每个 format 的尝试；但 SRT 检测最宽松（只检查"包含 `-->`"），最后才 fallback。这导致 SRT 在 `detect_format` 中**永远胜出**任何"看起来像"含 `-->` 的内容，包括 VTT 去除 header 后。但 VTT 检测放在 SRT 之前（用 cfg 控制链顺序）—— 顺序是**硬编码在源代码中**，与 `cli::Format::from_ext` 顺序不同。
- **建议**：抽出 `Format::from_ext` 与 `detect_format` 的优先级到一个共享的 `DetectionOrder` 常量。

---

## 2. 解析器正确性（高优先级）

### 2.1 🐛 ASS `parse_ass_dialogue` 错误地把 group 15 当作 Effect

[ass.rs:104](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/ass.rs#L104)：
```rust
let is_comment = caps.get(15).is_some_and(|m| m.as_str().contains("Comment"));
```

实际数一下 `RE_DIALOGUE` 的 capture group：
1. Start hour, 2. Start min, 3. Start sec, 4. Start cs,
5. End hour, 6. End min, 7. End sec, 8. End cs,
9. Style, 10. Name (actor), 11. MarginL, 12. MarginR, 13. MarginV,
14. **Effect**（在 `(?:([^,]*),)?` 中）,
15. `(\{.*\})?` —— 即 text 段前的 `{...}` tag 块,
16. `(.+)$` —— text 自身。

所以 `is_comment` 应当是 `caps.get(14).is_some_and(|m| m.as_str().starts_with("Comment"))`。当前实现：
- 任何 Effect 列里包含字面量 `"Comment"` 的 dialogue 都被标记为 comment；
- 任何文本前有 `{...}` tag 的 dialogue 也被误判为 comment（因为 group 15 总是 `Some`）。

> **结论**：这是一个**已确认的 bug**。需要修复并补一个针对 `Dialogue: 0,0:00:01.00,...,Comment: ...,Text` 行格式的测试。

### 2.2 🐛 SRT `parse_stream` 与 `parse` 行为不一致

[srt.rs:142-167](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/srt.rs#L142-L167) 的 `parse` 在 `Phase::Index` 看到 timestamp 时**报错**；但 [srt.rs:305-314](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/srt.rs#L305-L314) 的 `parse_stream` 用 `parse_timestamp(...).ok()` 静默吞错。
- 一个缺失 index 的文件走 `parse_content` 会报错，走 `parse_stream` 会得到一个 start=0/end=0 的损坏 subtitle。
- **建议**：两个函数共享同一个 `parse_inner` 状态机；或者让 `parse_stream` 在错误时返回 `Some(Err(...))`。

### 2.3 🐛 SRT `extract_text_parts` 没有提取 SRT 字体属性中的 italic / underline

[srt.rs:25-103](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/srt.rs#L25-L103) 只识别 `b/i/u/font`，但 SRT 允许 `<i>...</i>`、`<b>...</b>` 嵌套，bold + italic 都需要累积。但当前实现是**last-write-wins**（遇到 `<i>` 改 italic=true，遇到 `</i>` 改 italic=false），这对嵌套 OK，但 flat text 中在 bold 段里出现 `<i>` 不会被记录为 bold+italic 共存 —— 而代码本身其实正确（push 时只 push 当前累积），没问题。

但 [srt.rs:43-44](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/srt.rs#L43-L44) 的 condition 是 `if bold || italic || underline || color.is_some()`，漏掉了"非格式纯文本" —— 实际是对的，因为非格式文本可以靠 `text` 字段还原。

### 2.4 ⚠️ VTT `parse` 的 Phase 状态机有两个相同分支

[vtt.rs:137-141](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/vtt.rs#L137-L141)：
```rust
phase = match phase {
    Phase::VttComment => Phase::Cue,
    Phase::Header => Phase::Cue,
    _ => Phase::Cue,
};
```
三个分支完全相同，可以直接写 `phase = Phase::Cue;`。
另外 [vtt.rs:153](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/vtt.rs#L153) 的 `if trimmed.starts_with("WEBVTT") {}` 是空分支，可以删。

### 2.5 ⚠️ VTT 头部 NOTE 块处理与 header 输出耦合

[vtt.rs:126-143](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/vtt.rs#L126-L143) 的逻辑：进入 `Header` 阶段时把每行推入 `header_lines`；遇到 `NOTE` 行切换到 `VttComment`；遇到空行时若在 `Header` 且 `header_lines` 非空则拼成 header 并 clear。
- 问题：**[INFORMATION] ...** 风格的元数据若包含 `NOTE` 关键字会被误判。
- 问题：header 与 NOTE 块的边界仅由空行决定，而 VTT 规范允许 NOTE 单行存在；`Phase::VttComment` 实际上**不会持续多行** —— 进入 NOTE 后再遇到任何非空行都会被当作 cue 头处理。
- **建议**：在 `VttComment` 状态下不要把 `trimmed` 加入 header_lines，并显式地只在 `Header` 阶段拼接 header。

### 2.6 ⚠️ ASS `RE_DIALOGUE` 对 `text` 字段含逗号/换行的处理

[ass.rs:9-10](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/ass.rs#L9-L10) 的正则在 `(.+)$` 处贪心匹配直到字符串末尾 —— 对单行 dialogue 正确；但 ASS 规范允许 Text 列以 `\N` 强制换行（多行文本），目前解析是按行进行的（[ass.rs:123](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/ass.rs#L123) `for line in content.lines()`），所以**多行 dialogue 会被截断**。
- **建议**：检测 `Format:` 中 Text 列的字段数；如果不是 10 列而 `Format:` 显式声明更多（如 Custom 字段），则用 comma-aware 的列切分。

### 2.7 ⚠️ MicroDVD `parse_content` 的 FPS 头解析顺序

[microdvd.rs:39-49](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/microdvd.rs#L39-L49)：
```rust
if subtitles.is_empty()
  && let Some(caps) = RE_FPS_HEADER.captures(trimmed)
{
  let fps_str = caps.get(3).map_or_else(|| caps[4].to_string(), |m| m.as_str().to_string());
```
`RE_FPS_HEADER` = `r"\{(\d+)\}\{(\d+)\}(?:\s*\[(\d+(?:\.\d+)?)\])?\s*(\d+(?:\.\d+)?)"`。`caps[3]` 是不带方括号的 fps；`caps[4]` 是带方括号的 fps。但只有当行匹配 `RE_FPS_HEADER` 时才能进入分支，所以 `caps[3]` 存在但可能为 `None`（无方括号），此时 `caps[4]` 必然存在（因为正则有 `(?:\s*\[...\])?\s*(\d+...)` —— 即使没方括号，最后一个 `(\d+...)` 必须匹配才能让整条 regex 命中）。
- 但 cap 4 在 cap 3 为 None 时**未必是 fps**：例如 `{1}{1}30` —— cap 3=None, cap 4="30"，OK；但 `{1}{1}xyz 30` 不会命中整个 regex（`{1}{1}` 后必须是 `\d+` 才会让 `{1}{1}` 贪婪匹配完）。
- 边界正确但**易碎**：未来若改 regex 这里会静默 broken。
- 此外 fps 一旦从第一行读取，后续字幕都用这个 fps —— 这是**符合规范的**，但**没有警告用户 fps 改变的情况**。如果文件实际有 fps 变化（如 `{1}{1}25\n{30}{60}Hello\n{1}{1}30\n{70}{80}World`）会被一致错误地按首个 fps 解析。

### 2.8 ⚠️ TTML `<p>` 嵌套 / 自闭合处理

[ttml.rs:105-131](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/ttml.rs#L105-L131) 的 `Event::Empty` 处理自闭合 `<p/>` 是正确的，但 `Event::Start` + `Event::End` 配对处理中：
- 切换 `in_p` 状态在 `Event::Start` 立即置 true（[ttml.rs:62](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/ttml.rs#L62)），但 `current_text.clear()` 等初始化在 start 阶段就执行了；
- 这意味着当 `Event::End` 触发时，**任何在 `p` 内嵌的另一个 `p`** 会先把 `current_text` 写入 Vec 但 `in_p` 仍为 true —— 父级 `<p>` 的文本就被截断。
- TTML 规范禁止嵌套 `<p>`，但恶意/损坏的输入可能触发；`quick-xml` 不会做 schema 校验。
- **建议**：进入嵌套 `p` 时显式报错或记录 `warning!()`。

### 2.9 ⚠️ LRC 多时间戳行的 round-trip 不可逆

[lrc.rs:54-57](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/lrc.rs#L54-L57) 把 `[00:10.00][00:30.00]text` 拆成两个 Subtitle（start=10000, end=15000; start=30000, end=35000），[lrc.rs:98-110](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/lrc.rs#L98-L110) 写回时变成两行独立的 `[00:10.00]text` + `[00:30.00]text`。
- 文本内容在第二轮解析时会被解释为第二个时间戳的"歌词"，导致 round-trip 数据偏移。
- **建议**：要么 LRC 不支持 round-trip（文档说清），要么用专门的 `LrcData { line: Vec<(Vec<u64>, String)> }` 模型。

### 2.10 ⚠️ SBV 时间戳只接受 `:` 与 `.` 都不缺失的格式

[sbv.rs:67-89](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/sbv.rs#L67-L89) 的 detect 用 `parts[0].contains(':') && parts[0].contains('.')` 来识别 SBV，但 SRT 的"看起来像"SBV 也能通过（例如 `1,2:00:01.000,0:00:03.500,hello` 罕见但理论可能）；**SRT 优先级在 `lib.rs::detect_format` 中比 SBV 早**，所以一般不会冲突，但当文件以 SRT 形式 `0:00:01,000 --> 0:00:03,500` 给出时，根本不会进入 SBV 检测 —— 这是对的。
- **但**：当 extension 为 `.sbv` 但内容是 SRT 时，CLI 的 `Format::from_ext` 返回 `Format::Sbv`，会调用 sbv parser 然后大概率失败。

### 2.11 ⚠️ SubViewer 时间格式 centiseconds 假设

[subviewer.rs:32-47](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/subviewer.rs#L32-L47)：
```rust
let ms: u64 = if s_parts.len() > 1 {
    s_parts[1].parse::<u64>()? * 10 // SubViewer uses centiseconds, convert to ms
} else {
    0
};
```
- `* 10` 把 centiseconds (0-99) → ms (0-990)；
- 但 SubViewer 2.0 规范实际使用 **centiseconds** 没错；
- 1 位小数（如 `00:00:01.5`）会被解析为 5 centiseconds = 50ms，正确；
- 3+ 位小数（如 `00:00:01.500`）会解析为 500 centiseconds = 5000ms —— **错**！这会让毫秒数被错误放大 100 倍。
- **建议**：明确限 2 位；超过 2 位报错或截断。

---

## 3. 错误处理 & 健壮性

### 3.1 ⚠️ `encoding` 模块几乎未被使用（死代码）

[encoding.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/encoding.rs) 提供了 `detect_encoding` / `decode_to_string`（带 chardetng fallback），但：
- 所有 format 的 `parse_bytes` 都直接 `String::from_utf8(data.to_vec())`；
- `parse_file` 都用 `tokio::fs::read_to_string` 假设 UTF-8；
- ASS/SSA 真实场景中常是 GBK / Shift-JIS / Big5，会直接失败。
- **建议**：在 `lib.rs::parse_bytes` 中先用 `encoding::decode_to_string`，失败时再尝试按 format 默认编码 fallback；或让 format 入口接受 `encoding` 参数。

### 3.2 ⚠️ `SubtitleError` 几乎没被使用

[error.rs:32-49](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/error.rs#L32-L49) 定义了 `SubtitleError` 但 parser 全部用 `anyhow::anyhow!`，只在 `ParseError::Decode(#[from] SubtitleError)` 间接暴露。
- 文档承诺"structured error variants for new code and gradual migration" —— 但没有任何 format parser 切换到 `SubtitleError`。
- 注释中说"a function returning `Result<_, SubtitleError>` interops with `AnyResult` callers" —— 实际上 `AnyResult = Result<T, anyhow::Error>`，通过 `?` 自动转换是 OK 的。
- **建议**：把至少一个 format（建议 SRT）切换到 `SubtitleError` 作为示范；并把 `ParseError::Anyhow` 注释中"This is a code smell"明确写出来。

### 3.3 ⚠️ `main.rs:22` `expect("setting default subscriber failed")`

[main.rs:22](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/main.rs#L22) 在设置 tracing subscriber 失败时直接 panic。在用户已经 `RUST_LOG` 设了其他 subscriber 后启动会失败。
- **建议**：失败时打印 warning 并跳过（降级到无 logging）。

### 3.4 ⚠️ `ttml.rs:263` `expect("TTML writer always produces valid UTF-8")`

[ttml.rs:263](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/ttml.rs#L263) 假设 `quick_xml::Writer<Cursor<Vec<u8>>>` 总是产 UTF-8 —— `quick-xml` 的 `Writer` 内部就是 byte 流，**只在传入字符串失败时 panic**（永远不会因为 UTF-8 invalid 而失败，但 `BytesText::new` 接受 &str 而该 `&str` 来自用户数据）。
- 实际：如果 `sub.text` 含 invalid surrogate pair，调用时 `BytesText::new` 会 panic 上游而不是这里；这里相对安全。
- 但 `expect` 在 `panic = "abort"` 的 release profile 下表现为 abort —— 不友好。
- **建议**：把 `expect` 替换为 `unwrap_or_default()` 并在调试模式下记录 warning。

### 3.5 ⚠️ `cli.rs:33-62` extension 推断不区分大小写的子串匹配

[cli.rs:33-62](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/cli.rs#L33-L62) 使用 `to_lowercase().ends_with(...)`；对 `.sub` 来说容易和 VobSub 混淆。`detect_format` 应该优先于 `from_ext`（事实上 `resolve_format` 顺序是这样），但当 source 是 stdin (`-`) 时，hint 是 `None`，全靠 `detect_format`。
- **建议**：对 `.sub` 这种情况给出 warning 让用户显式 `--from`。

### 3.6 ⚠️ 解析器不强制"严格模式"或"宽松模式"

所有 parser 都是"宽容"的（坏数据 → 跳过该行或 fallback），调用方无法：
- 拒绝模糊的 cue；
- 区分"成功解析 0 个字幕"和"输入为空"；
- 接收警告（truncated cue / unknown tag）而不中断。

### 3.7 ⚠️ `parse_url` 没有超时/重试/重定向/headers 配置

[lib.rs:123-127](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/lib.rs#L123-L127) 直接 `reqwest::get(url)`，使用默认 client。
- 公共 API 暴露了网络入口但调用方无法配置 —— 不能禁用 redirect（SSRF 风险）、不能设 timeout（DoS 风险）。
- **建议**：暴露一个 `parse_url_with(url, &reqwest::Client)` 变体；或者接受一个 `&reqwest::Client` 参数。

### 3.8 ⚠️ 大文件无 size 限制

`parse_file` 接受任意路径的 `tokio::fs::read` —— 调用方无 way to cap memory usage。一个恶意的 10GB SRT 文件会 OOM 整个进程。
- **建议**：添加 `parse_file_with_size_cap(path, max_bytes)` 或要求调用方传 `impl Read`。

---

## 4. 性能

### 4.1 ⚠️ `parse_bytes` 全量复制

每个 format 的 `parse_bytes` 都执行 `String::from_utf8(data.to_vec())` —— 整个 buffer 被 clone。
- 在 `parse_file` 路径上：`tokio::fs::read_to_string` 也做了 UTF-8 校验（隐含 copy）。
- **建议**：用 `std::str::from_utf8(data)` 直接借用；只在需要 owned 时再 to_string。

### 4.2 ⚠️ `detect_format` 每次都重新 `from_utf8 + lines()`

[lib.rs:30-51](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/lib.rs#L30-L51) 在 9 个 format 间链式 OR 调用 —— 每个 format 内部都做 `String::from_utf8 + lines()`。对 1MB 输入这是 9 次 UTF-8 校验 + 9 次 lines iterator 创建。
- **建议**：在 `lib.rs::detect_format` 中先做一次 `from_utf8` + `lines()`，把 `&str` 切片传给各 format 的 `detect_format_str(&str)`。

### 4.3 ⚠️ `extract_text_parts` 在每个 subtitle 都重新跑 regex

[srt.rs:25-103](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/srt.rs#L25-L103) 使用 `RE_SRT_TAG.find_iter` —— 静态 LazyLock 已优化。但 find_iter 每次会扫描整个 string，即便没有 tag。
- 100k 行字幕大多数没 tag（普通翻译字幕）—— 仍是 100k 次 regex 扫描。
- **建议**：先做 `text.contains('<')` 快速跳过；或在 parser 中就识别 tag 边界而不必重扫。

### 4.4 ⚠️ 几乎所有 parser 都是 non-streaming

除 `srt::parse_stream` 外，所有 parser 都一次性把整文件读到 String 再 `lines()`。
- 内存峰值 = 文件 size + 解析结构，对 100MB TTML 不可接受。
- **建议**：为 MicroDVD/ASS/SBV/LRC 也提供 `*Stream` 迭代器。

### 4.5 ⚠️ `validate()` 的 sorted-order overlap 检查

[model.rs:440-488](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/model.rs#L440-L488) 先 sort indices 然后 windows(2) —— **正确且高效**（O(n log n)）。但 `validate_extended` 在 1..subs.len() 中再 `saturating_sub` 计算 gap —— 这是 O(n) 单独一遍。两次 pass 没问题。
- 注释良好，可读性强。

### 4.6 ⚠️ `subtitler::normalize::optimize_line_breaks` 递归

[normalize.rs:106-141](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/normalize.rs#L106-L141) 递归实现 split —— 对超长单行（>10k chars）会爆栈。CLI 用 `--max-chars 42` 默认值一般安全，但 API 用户可能传 `max_chars: 1000`。
- **建议**：改为循环（`while ... { ... }`）。

---

## 5. API 一致性 & 抽象

### 5.1 ⚠️ 各 format 的 `parse_*` 签名不一致

| Format | `parse_content` 返回 | `parse_bytes` 返回 | `parse_file` 返回 |
|---|---|---|---|
| srt | `Vec<Subtitle>` | `Vec<Subtitle>` | `Vec<Subtitle>` |
| vtt | `Vec<Subtitle>` | `Vec<Subtitle>` | `Vec<Subtitle>` |
| vtt `*_full` | `(Option<String>, Vec<Subtitle>)` | `(Option<String>, Vec<Subtitle>)` | — |
| ass | `SubtitleFile` | `SubtitleFile` | `SubtitleFile` |
| microdvd | `SubtitleFile` | `(f64, Vec<Subtitle>)` | `SubtitleFile` |
| subviewer | `(Option<String>, Vec<Subtitle>)` | `(Option<String>, Vec<Subtitle>)` | `(Option<String>, Vec<Subtitle>)` |
| ttml | `Vec<Subtitle>` | `Vec<Subtitle>` | `Vec<Subtitle>` |
| sbv | `Vec<Subtitle>` | `Vec<Subtitle>` | `Vec<Subtitle>` |
| lrc | `Vec<Subtitle>` | `Vec<Subtitle>` | `Vec<Subtitle>` |

**MicroDVD** 特别奇怪：`parse_content` 返回 `SubtitleFile`，`parse_bytes` 拆成元组。需要调用方根据 format 类型选 API。
- **建议**：统一为 `SubtitleFile` 一种返回类型；`bytes` / `file` / `url` 走 thin wrapper。

### 5.2 ⚠️ `to_string` 签名不一致

`vtt::to_string(subtitles, header: Option<&str>)` 接受 header；`subviewer::to_string(subtitles, header: Option<&str>)` 同样；`ass::to_string(info, styles, subtitles)` 三个参数；`ttml::to_string(subtitles, _header: Option<&str>)` 第二个参数是 `_`（被忽略！）。
- TTML 的 `_header` 是个**未实现的 TODO** —— header 没有被写回输出。
- **建议**：明确在文档中说"TTML 的 header 暂未实现"；或直接删除该参数。

### 5.3 ⚠️ `generate` 写文件时静默覆盖

[srt.rs:393-409](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/srt.rs#L393-L409) 等都 `OpenOptions::new().create(true).write(true).truncate(true).open(path)` —— 静默覆盖现有文件。AGENTS.md 也提到"they overwrite, not append"，但**没有提供 "refuse to overwrite" 选项**。
- **建议**：添加 `generate_safe` / `policy: WritePolicy` 参数（`Overwrite` / `RefuseIfExists` / `Append`）。

### 5.4 ⚠️ `pub fn detect_format(data: &[u8])` 在 lib 和 format 模块都暴露

`subtitler::detect_format`（[lib.rs:30](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/lib.rs#L30)）和 `subtitler::srt::detect_format` 等是并行存在的两套 API —— 前者做全格式链式检测，后者只检测一种。命名合理，但**没有 const generics / 标记类型**让调用方根据输入类型做编译期分派（例如 `parse_bytes_as`）。

### 5.5 ⚠️ `Subtitle::shift` 的 clamp 行为没有文档

[model.rs:61-66](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/model.rs#L61-L66)：
```rust
pub fn shift(&mut self, offset_ms: i64) {
    let start = self.start as i64 + offset_ms;
    let end = self.end as i64 + offset_ms;
    self.start = start.max(0) as u64;
    self.end = end.max(0) as u64;
}
```
clamp 到 0 —— 但当 end > start 不变量被破坏时（如 shift -5000 让 end 变 0、start 变 0），subsequent `validate()` 会报 `ZeroDuration` / `NegativeDuration`。**没有 panic，但会让 invariant 静默破坏**。
- 文档应说："Negative shift may cause end < start; downstream validation will report it."

### 5.6 ⚠️ `Subtitle` 的构造函数没接受 `index` 和 `settings`

`Subtitle::new(start, end, text)` 永远设 `index = None`, `settings = None`。调用方需要 `mut s = Subtitle::new(...); s.index = Some(1);`。这与 Rust 习惯 builder pattern 偏离。
- 例子：所有测试都用裸 struct literal 构造完整对象 —— 反映了 `new` 是不完整的。
- **建议**：添加 `Subtitle::builder()` 或接受更多 optional 参数。

---

## 6. CLI (`main.rs` / `cli.rs`)

### 6.1 ✅ 优点
- clap derive 风格、参数 help 自动生成；
- 子命令清晰：parse / convert / validate / edit / info / detect / quality / normalize / shift；
- stdin/stdout 通过 `-` 支持。

### 6.2 ⚠️ 问题

#### 6.2.1 `parse` 子命令输出忽略原 index 字段

[main.rs:190-202](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/main.rs#L190-L202)：
```rust
for (i, sub) in subs.iter().enumerate() {
    println!("[{}] {:0>2}:{:0>2}:{:0>2},{:0>3} --> ...
    // i + 1 是行号，不是 sub.index
}
```
对 SRT/VTT 这没问题；但对 ASS/SubViewer 来说"index"的概念不一样（ASS 没有 index）。当前实现统一用 `i+1`，对**所有格式都用 SRT 风格打印**。
- **建议**：根据 format 选不同的输出格式；或者直接 `--format json` 输出去掉行号打印。

#### 6.2.2 `cmd_info` 在空文件上 panic

[main.rs:384-389](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/main.rs#L384-L389) 已经处理了 `subs.is_empty()` —— OK；但 [main.rs:393](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/main.rs#L393) `last.end - first.start` 用 `saturating_sub` 是好的；[main.rs:395](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/main.rs#L395) `durations.iter().sum::<u64>() / subs.len()` —— 已确保 subs 非空才到这里。
- 但 `total_chars` 用 `chars().count()` 对每个 sub —— O(n)，对超大文件不友好；可以 streaming 累加。

#### 6.2.3 `cmd_edit` 的 `--output` 是必需

[cli.rs:202-238](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/cli.rs#L202-L238) 强制 `output`，但 `subtitler edit` 的 `output` 参数定义为 `#[arg(short, long)]` 意味着可以用 `-o` 或 `--output`，但**没有 `default_value`**，缺省会 clap 报错。
- 允许 `-` 作为 output (stdout) 是显式设计，但**没在 doc-comment 中说明**。

#### 6.2.4 `--transform-fps` 解析逻辑

[main.rs:351-356](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/main.rs#L351-L356)：
```rust
if let Some(fps_pair) = args.transform_fps
  && fps_pair.len() == 2
{
```
如果 `fps_pair.len() != 2`（即 1 个或 3 个值），**静默忽略**，不报错。
- **建议**：clap 已经有 `number_of_values = 2` 校验，但若用户给 1 个值，clap 会拒；给 3 个值也拒。代码里的 `.len() == 2` 应当是 `assert_eq!(fps_pair.len(), 2)` 或直接用 unwrap（因为 clap 已保证）。

#### 6.2.5 没有任何 subcommand 是 `batch`

CLI 没有并行/多文件处理。对 SRT 转换工具来说，常见的 `subtitler convert *.srt` 通配符需要 shell 循环。
- **建议**：添加 `--jobs N` 或 `batch` 子命令。

---

## 7. 测试

### 7.1 ✅ 优点
- `tests/integration.rs` 包含 ~30+ 测试覆盖 SRT/VTT/ASS 主体路径；
- `tests/cleanup_batch.rs` 验证了 validate 在 unsorted input 上的正确性；
- `benches/subtitler_benchmark.rs` 覆盖了所有 format 的 parse/stringify + 转换 + regex hotspots；
- 单元测试基本覆盖了 `model.rs` 的所有 trait 方法。

### 7.2 ⚠️ 缺口

| Format | Round-trip | 边界 case | Fuzz |
|---|---|---|---|
| SRT | ✅ | ✅ (BOM, empty, leading newline) | ❌ |
| VTT | ✅ | ✅ (BOM, NOTE, header) | ❌ |
| ASS | ✅ | ⚠️ (无 BOM / 无 ScriptType / 多 Style) | ❌ |
| SSA | ⚠️ (只测 v4 styles) | ❌ | ❌ |
| MicroDVD | ✅ | ⚠️ (无 FPS 头) | ❌ |
| SubViewer | ✅ | ❌ | ❌ |
| TTML | ✅ | ⚠️ (无 br, 无 span) | ❌ |
| SBV | ⚠️ (无 multiline text) | ❌ | ❌ |
| LRC | ⚠️ (多 timestamp 行 round-trip 不可逆, 未测) | ❌ | ❌ |

#### 7.2.1 SBV / LRC / SubViewer 缺少 round-trip fuzz
最起码应当补：
- SBV: multiline text（text 字段含 `\n`）
- LRC: 多个时间戳行 `[00:10.00][00:30.00]text` 的 round-trip
- SubViewer: 1.0 vs 2.0 header 区别
- MicroDVD: fps 头（`{1}{1}30.000`）的 round-trip —— [arch_unification.rs:68-78](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/tests/arch_unification.rs#L68-L78) 测了 fps 保留但没测 ms→frame→ms round-trip 的容差

#### 7.2.2 没有 property-based testing
`proptest` 或 `quickcheck` 都没有。例如：
- 任意 `Subtitle` Vec → `to_string` → `parse` 应当等于原 Vec
- 任意 `t1, t2` → `format_timestamp` → `parse_timestamp` 应当等于原值（除去精度损失）

#### 7.2.3 Benchmarks `from_str -> -> String` 字符串拼接
[benches/subtitler_benchmark.rs:99-112](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/benches/subtitler_benchmark.rs#L99-L112) 等几处 `format!` 在 hot path 反复使用 —— 这本身可能是优化机会（用 `write!` + buffer）。Bench 应当能帮助发现这类问题。

#### 7.2.4 ASS `is_comment` bug 没有 regression test
如 §2.1 所述，没有针对 `Dialogue: 0,0:00:01.00,...,Comment: ...,Hello` 这种"Effect 列以 Comment 开头"情形的测试。

---

## 8. 文档

### 8.1 ⚠️ README 不完整
- 没有 `TTML`、`SBV`、`LRC` 三个 format 的 API 表格（README 只列到 SubViewer）；
- 没有 `extract_range` / `concatenate` / `enforce_min_duration` / `enforce_max_duration` / `auto_extend_for_cps` / `remove_overlaps` 的方法表；
- 没有 `QualityReport` 结构体的字段说明；
- 没有 `Translator` trait 的说明（README 提到"full CLI"但完全不提 translation 接口）。

### 8.2 ⚠️ `SubtitleFile` 枚举无 doc comment
[model.rs:275-307](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/model.rs#L275-L307) 每个变体都没有 `///` 文档；公共 API 的核心类型完全没文档。

### 8.3 ⚠️ `parse_url` 的网络/安全警告缺失
文档没说 `parse_url` 会发出任意 HTTP 请求。

### 8.4 ⚠️ docs/promotion/04-english-reddit-hn.md 中的"production code 0 panic"声明需更新
> 实际：main.rs:22 和 ttml.rs:263 各有 `expect`；并且依赖 `regex::Regex::new(...).unwrap()` 这种 LazyLock 初始化是 `unwrap` 但**在 panic 时会 abort process** —— 算"production code 中的 panic 风险"。

---

## 9. 依赖与 Cargo

### 9.1 ⚠️ `reqwest` 默认 features
[Cargo.toml:51](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/Cargo.toml#L51)：
```toml
reqwest = { version = "0.13", optional = true }
```
没禁 default features —— 会引入 `default-tls`（native-tls / OpenSSL）。**应该** `default-features = false, features = ["rustls-tls"]` 以避免在 macOS 之外的平台链接 OpenSSL。

### 9.2 ⚠️ `tracing` 与 `tracing-subscriber` 的角色错位
- `tracing = "0.1"` 在 deps；
- `tracing-subscriber = "0.3"` 在 deps；
- 库代码（`src/`）**完全没用 `tracing::span!` / `tracing::info!` 等宏**；
- 只在 `main.rs` 设置 subscriber。
- 库应当至少为 `parse_*` 入口加 `#[instrument]` 或 `info_span!`，否则 tracing 依赖是"装饰"。
- `tracing-subscriber` 在 release profile 仍会编译 —— 应当放到 `[dev-dependencies]`（实际上 `main.rs` 用的是 binary，不影响 lib 用户的 release 构建）。

### 9.3 ⚠️ `quick-xml` 仅在 ttml feature 才用，但 `ass` 可能含 inline XML
- [ttml.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/ttml.rs) 是唯一使用者；
- 文档"supports IMSC 1.0/1.1"是 OK 的，但 `quick-xml` 的版本 0.37 在 2024 后被新版本替代 —— 升级到 0.36/0.38 看是否仍有 breaking。

### 9.4 ⚠️ `tokio` feature 选择
[Cargo.toml:55](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/Cargo.toml#L55)：
```toml
tokio = { version = "1", features = ["fs", "io-util", "rt-multi-thread", "macros"] }
```
- `rt-multi-thread` 引入较大；只需要 `rt` 即可（`parse_file` 用 `tokio::fs::read` 已经在 `#[tokio::main]` 中）。
- 文档说"async I/O powered by `tokio`"是事实但增加了二进制大小。

### 9.5 ⚠️ `panic = "abort"` 在 release 但 dev profile 仍 unwind
[profile.release] 中 `panic = "abort"` 没问题；但测试和示例都用 default dev profile，会有 unwind tables。

---

## 10. 安全 & 隐私

### 10.1 ✅ 没有 `unsafe` 块（已确认 grep）
### 10.2 ⚠️ SSRF
`parse_url` 接受任意 `http://` URL，包括内网 IP（10.0.0.0/8、192.168.0.0/16 等）。如果库被嵌入到 server-side 应用（如 web 服务），恶意 URL 可用于内网扫描。
- **建议**：提供 `parse_url_safe` 或要求调用方自己过滤。

### 10.3 ⚠️ 路径遍历
`parse_file(path)` 接受任意 `AsRef<Path>`，无沙箱。如果库被嵌入接受用户输入的 web 服务，需要调用方做路径校验。

### 10.4 ⚠️ ReDoS 风险
`normalize::RE_HI_PAREN = r"\s*[\(\[][^)\]]{2,60}[\)\]]"` 上限 60 字符 OK；但 `RE_OCR_PATTERNS` 里的 `r"(\d)rn(\w)"` 这种没有 anchor 也不长尾 —— 实际无 ReDoS 风险。但 `parse_timestamps` 的 `RE_TIMESTAMPS` 在 `RE_TIMESTAMP` 之外增加了 `--> ` 间隔——`RE_TIMESTAMP` 的 `\d{1,}` 没有上限，恶意输入会做大量回溯。
- **建议**：给 `\d{1,}` 加明确上限（如 `\d{1,10}`），hours/minutes/seconds 不可能超过 4 位。

---

## 11. 优先级建议清单

### 🔴 高优先级（影响正确性 / 兼容性）
1. **修复 §2.1 ASS `is_comment` group 错位 bug**
2. **修复 §2.11 SubViewer 多位小数 centiseconds ×10 错位**
3. **修复 §2.2 SRT `parse_stream` 静默吞错**
4. **§3.1 让 `parse_bytes` / `parse_file` 走 `encoding::decode_to_string`（GBK/Shift-JIS ASS/SSA）**
5. **§3.7 给 `parse_url` 暴露 reqwest::Client 配置入口**
6. **§10.4 给 `RE_TIMESTAMP` 数字加上限**

### 🟡 中优先级（API 一致性 / 健壮性）
7. **§5.1 统一各 format 的 `parse_*` 返回类型**
8. **§5.2 TTML `_header` 未实现，明确或删除**
9. **§5.3 `generate` 添加 `WritePolicy`**
10. **§4.1 `parse_bytes` 避免 `data.to_vec()`**
11. **§4.2 `detect_format` 共享 pre-parsed lines**
12. **§3.5 `parse_url` 支持 stdin/超时/headers**
13. **§3.6 parser 严格/宽松模式开关**
14. **§6.2.4 `transform-fps` 长度校验**
15. **§9.1 reqwest default-features = false + rustls-tls**

### 🟢 低优先级（清理 / 文档）
16. **§1.2.1 `Subtitle` 字段瘦身**
17. **§5.6 `Subtitle` builder 构造**
18. **§7.2 补齐 SBV / LRC / SubViewer / MicroDVD round-trip + edge case**
19. **§7.2.2 引入 proptest 做 property-based testing**
20. **§8 README 补齐 TTML / SBV / LRC + editing API + QualityReport**
21. **§8.2 给 `SubtitleFile` 各变体加 `///` 文档**
22. **§2.10 LRC 多时间戳行 round-trip 模型重构**
23. **§4.3 `extract_text_parts` 提前 `contains('<')` 跳过**
24. **§4.4 为大 format 提供 streaming parser**
25. **§9.2 tracing 在 lib 代码中加 `#[instrument]`，否则移除 deps**
26. **§9.4 tokio 减 feature**
27. **§4.6 `optimize_line_breaks` 改循环避免深递归**

---

## 12. 附录：审查方法

- 完整阅读 9 个 format 模块（`srt.rs` / `vtt.rs` / `ass.rs` / `microdvd.rs` / `subviewer.rs` / `ttml.rs` / `sbv.rs` / `lrc.rs`），`model.rs` / `utils.rs` / `config.rs` / `error.rs` / `encoding.rs` / `quality.rs` / `normalize.rs` / `types.rs` / `lib.rs` / `cli.rs` / `main.rs`；
- 阅读 `tests/integration.rs` / `tests/arch_unification.rs` / `tests/cross_format.rs` / `tests/cleanup_batch.rs` / `benches/subtitler_benchmark.rs`；
- 静态扫描 `unwrap` / `expect` / `panic` / `unsafe` / `TODO` / `FIXME` / `unimplemented!` 等可疑模式；
- 抽样验证 `RE_DIALOGUE` / `RE_TIMESTAMP` / `RE_SRT_DETECT` 等关键正则的 capture group 编号；
- 未执行 `cargo build` / `cargo test` —— 本报告是静态分析；建议补一轮 `cargo clippy -- -D warnings` 与 `cargo test --all-targets --all-features` 作为后续动作的输入。

> 本报告**未修改任何代码**；如需对其中某项建议落实，可另起任务。
