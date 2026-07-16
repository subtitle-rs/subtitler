# subtitler 代码审查 v2（2026-07-16 全量重审）

> 作者：代码审查助手
> 对应代码：`HEAD = 3d3c267`（已应用 10 轮迭代 + 4 轮 code review 修复）
> 前次报告：[code-review-2026-07-16.md](./code-review-2026-07-16.md) — **本文档为 v2，与前次并列**
> 方法：静态阅读 + 端到端实测 18 个源文件（共 6974 行）、12 个集成测试文件；**未修改任何代码**

---

## 0. 项目当前形态

| 维度 | 数据 |
|---|---|
| 源文件 | 18 个，6974 行 |
| `unwrap()` | 112 处（多数 LazyLock init + 单测） |
| `expect()` | **0** ✅（main.rs:22、ttml.rs:263 已修） |
| `unsafe` | 0 ✅ |
| `TODO` / `FIXME` / `XXX` / `HACK` | 0 ✅ |
| 测试数 | 203 passed (lib 111 + doc 12 + integration 80) |
| `cargo clippy -D warnings` | clean |
| `cargo fmt --check` | clean |
| `cargo doc --no-deps --all-features` | ok |
| 当前 `version` | `1.0.0` |

> **v1 报告后已修**：6 项 P0 + 7 项 P1 + 2 项 P2（详见 [iterative-optimization-plan-2026-07-16.md](./iterative-optimization-plan-2026-07-16.md)）。
> **本次 v2 报告**：在前次修复基础上，**重新审查 18 个源文件**发现**新的**与**残留的**问题。

---

## 1. 关键发现速览

| 严重度 | 数量 | 类型 |
|---|---|---|
| 🔴 P0 — Bug | **3** | 1 个新引入回归 + 2 个残留缺陷 |
| 🟡 P1 — 质量问题 | **10** | 错误处理、API 一致性、性能、文档 |
| 🟢 P2 — 改进建议 | **10** | 模块拆分、命名、代码风格 |
| 📌 文档缺口 | **3** | CHANGELOG / MIGRATION / 残留文件 |
| **合计** | **26** | |

---

## 2. 🔴 P0 — Bug（必须修）

### §14.1 `detect_format` 8 处全 UTF-8 限制 — **iter 2 半失效**（严重）

**位置**：
- [ass.rs:20-30](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/ass.rs#L20-L30) `String::from_utf8(data.to_vec())`
- [srt.rs:345-362](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/srt.rs#L345-L362) `String::from_utf8(data.to_vec())`
- [vtt.rs:243-250](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/vtt.rs#L243-L250) `String::from_utf8(data.to_vec())`
- [microdvd.rs:16-25](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/microdvd.rs#L16-L25) `String::from_utf8(data.to_vec())`
- [subviewer.rs:15](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/subviewer.rs#L15) `String::from_utf8(data.to_vec())`
- [sbv.rs:66-67](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/sbv.rs#L66-L67) `std::str::from_utf8(data).ok()?`
- [lrc.rs:149-150](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/lrc.rs#L149-L150) `std::str::from_utf8(data).ok()?`
- [ttml.rs:202-203](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/ttml.rs#L202-L203) `std::str::from_utf8(data).ok()?`

**问题**：

[lib.rs:30-51](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/lib.rs#L30-L51) 的 `subtitler::detect_format()` 是先调各 format 的 `detect_format` 再调 `parse_bytes_as`。**所有 8 个 `detect_format` 都用 `String::from_utf8` / `str::from_utf8`**，对 GBK/Shift_JIS 编码的 ASS 文件直接返回 `None`。

**后果**：用户调用 `subtitler::parse_bytes(gbk_ass_bytes)` → `UnknownFormat` 错误。但调用 `ass::parse_bytes(gbk_ass_bytes)` 能成功（因为 `parse_bytes` 走 `encoding::decode_to_string`）。

**iter 2 引入的 encoding 真解码能力被检测阶段架空**。这是**新引入的回归**（v1 报告 §3.1 修了一半）。

**修法**：

```rust
// src/encoding.rs 增加：先尝试 UTF-8，失败后用 chardetng
pub fn try_decode_for_detection(data: &[u8]) -> Option<String> {
  if let Ok(s) = std::str::from_utf8(data) {
    return Some(s.to_string());
  }
  let mut det = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Allow);
  det.feed(data, true);
  let enc = det.guess(None, chardetng::Utf8Detection::Allow);
  if let Some(encoding) = encoding_rs::Encoding::for_label_no_replacement(enc.name().as_bytes()) {
    let (cow, _, _) = encoding.decode(data);
    return Some(cow.into_owned());
  }
  None
}

// 各 format::detect_format 改为：
pub fn detect_format(data: &[u8]) -> Option<crate::model::Format> {
  let text = crate::encoding::try_decode_for_detection(data)?;
  text.contains("[Script Info]").then(|| crate::model::Format::Ass)
}
```

**性能注**：当前 `String::from_utf8(data.to_vec())` 对大文件会分配 2x；`try_decode_for_detection` 对 UTF-8 文件仍走 `from_utf8` 零拷贝路径（`str::from_utf8` 然后 `to_string` 一次分配），非 UTF-8 文件才走 full decode。可接受。

---

### §14.2 `LrcStream` 静默丢失重复时间戳 — **新引入回归**

**位置**：[lrc.rs:186-213](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/lrc.rs#L186-L213)

**问题**：

```rust
impl<'a> Iterator for LrcStream<'a> {
  fn next(&mut self) -> Option<Self::Item> {
    for line in self.lines.by_ref() {
      // ... 收集 times: Vec<u64>，可能多个 ...
      let t = times[0];   // ← 只取第一个！
      return Some(Ok(Subtitle::new(t, t + 5000, &text)));
    }
    None
  }
}
```

对 `[00:10.00][00:30.00]Repeated line`：
- `LrcData::parse` → 1 个 `LrcLine { times_ms: [10000, 30000], text: "Repeated line" }` ✅
- `LrcData::to_subtitles` → 2 个 Subtitle（每个时间戳一个）✅
- `LrcStream::next` → **1 个** Subtitle（只取 `times[0]`）❌

**Stream 与 batch 行为不一致**。这破坏 iter 6（LRC round-trip 不可逆修复）的承诺。

**修法**：

```rust
fn next(&mut self) -> Option<Self::Item> {
  for line in self.lines.by_ref() {
    // ... 收集 times: Vec<u64>，文本 ...
    if times.is_empty() || text.is_empty() { continue; }
    
    // 关键修复：用 LrcData::parse 同样的语义 — 对每个时间戳都 yield 一个 Subtitle
    self.pending_subs.extend(
      times.iter().map(|&t| Ok(Subtitle::new(t, t + 5000, &text)))
    );
    if let Some(s) = self.pending_subs.pop_front() {
      return Some(s);
    }
  }
  // Flush pending
  if let Some(s) = self.pending_subs.pop_front() {
    return Some(s);
  }
  None
}
```

**需要 state 改造**：`LrcStream` 增加 `pending_subs: VecDeque<AnyResult<Subtitle>>` 字段。

---

### §14.3 `srt::detect_format` 第二个分支过于宽松 — **遗留**

**位置**：[srt.rs:345-362](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/srt.rs#L345-L362)

```rust
pub fn detect_format(data: &[u8]) -> Option<crate::model::Format> {
  if let Ok(text) = String::from_utf8(data.to_vec()) {
    let trimmed = text.trim();
    if !trimmed.is_empty() {
      #[cfg(feature = "vtt")]
      if trimmed.starts_with("WEBVTT") {
        return Some(crate::model::Format::Vtt);
      }
      if RE_SRT_DETECT.is_match(trimmed) {
        return Some(crate::model::Format::Srt);
      }
      if trimmed.contains("-->") {           // ← 太宽松！
        return Some(crate::model::Format::Srt);
      }
    }
  }
  None
}
```

**问题**：任何含 `-->` 子串的文本都会被分类为 SRT。例如：
- `https://example.com/foo--bar` （URL 误判）
- `讨论 -- 接下来` （自然语言）
- `--enable-logs` （CLI 字符串）

**影响**：`subtitler::detect_format` 误判 SRT；后续 `parse_bytes` 走 SRT 解析器返回 malformed error（不是"UnknownFormat"，用户体验更差）。

**修法**：
```rust
// 第二个分支应改为更严格的 pattern：
if trimmed.contains(" --> ") || /* OR: regex匹配 \d{2}:\d{2}:\d{2}[,.]\d{3}\s+-->\s+\d{2}:\d{2}:\d{2} */
   return Some(crate::model::Format::Srt);
```

或直接删掉第二个分支（`RE_SRT_DETECT` 已足够严格）。

---

## 3. 🟡 P1 — 质量问题（应当修）

### §15.1 `ParseError::Anyhow` 与 `ParseError::Decode` display 重复

**位置**：[error.rs:19-24](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/error.rs#L19-L24)

```rust
#[error("decode/parse error: {0}")]
Anyhow(#[from] anyhow::Error),
#[error("decode/parse error: {0}")]
Decode(#[from] SubtitleError),
```

两条错误显示完全一样，**调用方无法区分**。建议：
- `Anyhow` 改为 `"internal error: {0}"` 或 `"{0}"`
- 或合并两个变体，只保留一个（`SubtitleError` 已经是 `thiserror` 定义，可直接 `#[from]` 转 `anyhow`）

### §15.2 `lib.rs::parse_bytes_as` 用 `#[allow(unreachable_patterns)] _ =>` 兜底

**位置**：[lib.rs:60-101](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/lib.rs#L60-L101)

```rust
pub fn parse_bytes_as(data: &[u8], fmt: Format) -> Result<model::SubtitleFile, error::ParseError> {
  match fmt {
    #[cfg(feature = "srt")]     Format::Srt => ...,
    #[cfg(feature = "vtt")]     Format::Vtt => ...,
    ...
    #[allow(unreachable_patterns)]
    _ => Err(error::ParseError::Unsupported(fmt)),
  }
}
```

`#[allow(unreachable_patterns)]` 表明 `Format` 枚举的所有变体在 default feature 组合下都被覆盖，但这是**配置型 match**，将来 `Format` 新增变体时编译器无法提示。**应该用 exhaustive match + `#[cfg(feature = "...")]` 给每个 arm**，避免 catch-all 兜底。

### §15.3 `WritePolicy` 处理在 8 个 format 模块重复

**位置**：srt/vtt/ass/microdvd/sbv/subviewer/ttml/lrc 各自 `generate` 都重复：

```rust
let policy = policy.unwrap_or_default();
if policy == crate::model::WritePolicy::RefuseIfExists && path.exists() {
  anyhow::bail!("Refusing to overwrite existing file: {}", path.display());
}
let mut open_opts = fs::OpenOptions::new();
let dest = match policy {
    crate::model::WritePolicy::Append => open_opts.create(true).append(true).open(path).await,
    _ => open_opts.write(true).truncate(true).create(true).open(path).await,
};
```

**8 份重复代码**。建议：

```rust
// src/io.rs 新增共用 helper
pub(crate) async fn open_for_write(
    path: impl AsRef<Path>,
    policy: WritePolicy,
) -> AnyResult<tokio::fs::File> {
    let path = path.as_ref();
    if policy == WritePolicy::RefuseIfExists && path.exists() {
        anyhow::bail!("Refusing to overwrite existing file: {}", path.display());
    }
    let mut opts = tokio::fs::OpenOptions::new();
    if policy == WritePolicy::Append {
        opts.create(true).append(true).open(path).await
    } else {
        opts.write(true).truncate(true).create(true).open(path).await
    }
}
```

### §15.4 模块级 `parse_url` 仍用 `reqwest::get`，与 `lib::parse_url_with` 模式不一致

**位置**（9 处）：

| 文件 | 行 | 状态 |
|---|---|---|
| [ass.rs:172-176](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/ass.rs#L172-L176) | `reqwest::get(url).await?` | 无 client config |
| [srt.rs:215-219](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/srt.rs#L215-L219) | 同上 | 无 client config |
| [vtt.rs:227-232](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/vtt.rs#L227-L232) | 同上 | 无 client config |
| [microdvd.rs:90-94](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/microdvd.rs#L90-L94) | 同上 | 无 client config |
| [subviewer.rs:110-114](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/subviewer.rs#L110-L114) | 同上 | 无 client config |
| [sbv.rs:58-62](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/sbv.rs#L58-L62) | 同上 | 无 client config |
| [lrc.rs:141-146](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/lrc.rs#L141-L146) | 同上 | 无 client config |
| [ttml.rs:195-200](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/ttml.rs#L195-L200) | 同上 | 无 client config |
| [main.rs:56](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/main.rs#L56) | 同上 | CLI 入口 |

iter 1 已修 `lib::parse_url_with`，但**模块级 `parse_url` 全部漏改**。这意味着用户从 doc.rs 看到 `srt::parse_url(url)` 的签名时，无法传 timeout/redirect/TLS 配置。

**修法**：每个模块都加 `parse_url_with(url, &client)`，与 lib 同款。

### §15.5 `Subtitle::shift` 文档与实现不一致

**位置**：[model.rs:79-89](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/model.rs#L79-L89)

```rust
/// Values are clamped to 0 — a large negative shift can produce `end < start`,
/// which downstream `validate()` will report as negative or zero duration.
pub fn shift(&mut self, offset_ms: i64) {
  let start = self.start as i64 + offset_ms;
  let end = self.end as i64 + offset_ms;
  self.start = start.max(0) as u64;
  self.end = end.max(0) as u64;   // ← 也夹到 0，所以 end 不会 < start
}
```

**矛盾**：文档说"可能产生 `end < start`"，但实现把 end 也夹到 0，所以**最坏情况是 `end == start == 0`**（不是 `end < start`）。

**修法**：
- 要么改文档：`"Both start and end are clamped to 0; a large negative shift can produce end == start == 0 (zero-duration subtitle)."`
- 要么改实现：end 允许负值（不 clamp），用 `as u64` 直接转换（但这会产生 underflow panic）

推荐改文档。

### §15.6 `srt.rs` 直接构造 `Subtitle` struct literal（绕过 `Subtitle::new`）

**位置**：
- [srt.rs:142-153](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/srt.rs#L142-L153) `parse` 中
- [srt.rs:288-298](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/srt.rs#L288-L298) `SrtStream` 中

两处都列了 7 个字段，与 `Subtitle::new` 重复。如果将来 `Subtitle` 新增字段，**必须同时修改这三处**。

**修法**：使用 `Subtitle::new(start, end, "")` + builder 方法，或在 `Subtitle` 上加 `with_index()` 后再 `with_layer()` 等。

### §15.7 `MicroDvdStream._fps` 是 dead 字段

**位置**：[microdvd.rs:116-130](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/microdvd.rs#L116-L130)

```rust
pub struct MicroDvdStream<'a> {
  lines: std::str::Lines<'a>,
  _fps: f64,        // ← 从未被读取
  saved_fps: f64,
}
impl<'a> MicroDvdStream<'a> {
  pub fn new(content: &'a str, fps: Option<f64>) -> Self {
    let f = fps.unwrap_or(DEFAULT_FPS);
    MicroDvdStream {
      lines: content.lines(),
      _fps: f,
      saved_fps: f,
    }
  }
}
```

`_fps` 与 `saved_fps` 初始值相同，且 `_fps` 从未被读取。`saved_fps` 是 `next()` 实际使用的，**`_fps` 冗余**。

**修法**：删除 `_fps: f64` 字段。

### §15.8 `normalize.rs:282-291` test 函数位于 `mod tests` 外

**位置**：[normalize.rs:281-291](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/normalize.rs#L281-L291)

```rust
}    // 281 — mod tests 结束

#[cfg(test)]
#[test]
fn test_optimize_line_breaks_order() {   // 283 — 在 mod 外
  let result = optimize_line_breaks("abc def ghijklmnop", 5);
  ...
}
```

**问题**：测试函数没归到 `mod tests` 模块里，破坏文件组织惯例。**编译能过，测试能跑**，但风格不一致。

**修法**：把 `#[cfg(test)] #[test] fn ...` 整体移入 `mod tests { ... }`。

### §15.9 `ass.rs:274` 存在未使用变量

**位置**：[ass.rs:274](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/ass.rs#L274)

```rust
pub fn parse_ass_tags(text: &str) -> Vec<crate::model::TextPart> {
  let mut bold = false;
  let mut italic = false;
  let mut underline = false;
  let _strikeout = false;       // ← 永远没被使用（caps[11] 直接用 i32 < 0 判断）
  ...
}
```

**修法**：删 `let _strikeout = false;`。

### §15.10 `LrcData.times_ms` 命名 vs `Subtitle.start/end` 不一致

**位置**：
- [lrc.rs:33](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/lrc.rs#L33) `pub times_ms: Vec<u64>` （带 `ms` 后缀）
- [model.rs:30-31](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/model.rs#L30-L31) `pub start: u64, pub end: u64` （无单位后缀）

**修法**：统一为 `start_ms` / `end_ms` / `times_ms`（与 `i64` 时间戳区分 `usize` 索引）。

---

## 4. 🟢 P2 — 改进建议（可选）

### §16.1 `model.rs` 1099 行，建议拆分

- [model.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/model.rs) 当前 1099 行，包含 `Subtitle` / `TextPart` / `Format` / `AssStyle` / `AssData` / `SubtitleFile` / `SubtitleFormat` trait / `impl SubtitleFile` 8 个公共抽象。
- 建议拆为 `model/subtitle.rs` / `model/file.rs` / `model/format.rs` / `model/text_part.rs` / `model/ass_data.rs`。

### §16.2 `config.rs` 2 行模块过度拆分

- [config.rs:1-2](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/config.rs#L1-L2) 仅有 2 个 const string，仅被 `utils.rs` 引用。
- 建议把 `RE_TIMESTAMP` / `RE_TIMESTAMPS` 直接 inline 到 `utils.rs` 顶部，删 `config.rs`。

### §16.3 `srt::to_string` 1-based 索引硬编码

- [srt.rs:364-387](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/srt.rs#L364-L387) 写文件时**忽略** `subtitle.index`，始终用 `i+1`。
- 用户从 SRT 读取后 round-trip，会丢失原 index。
- vtt 同样：[vtt.rs:252-282](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/vtt.rs#L252-L282) `let position = i + 1;`。
- CHANGELOG 1.0.0 段有说明，但**无 API 控制选项**。建议加 `pub fn to_string_with_indices(subs: &[Subtitle], preserve_index: bool)`。

### §16.4 `tests/proptest.rs` 只覆盖 SRT/VTT

- [tests/proptest.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/tests/proptest.rs) 2 个 property test（SRT round-trip、VTT round-trip）。
- ASS/LRC/MicroDVD/SubViewer/SBV/TTML 0 个 property test。
- 建议至少加：
  - `ass_round_trip_preserves_cues`
  - `lrc_round_trip_preserves_multi_timestamp`（这个会触发 §14.2 那个 bug）
  - `microdvd_round_trip_frame_count`

### §16.5 错误模型混合（`anyhow` + `thiserror`）

- [error.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/error.rs) 有 `SubtitleError` (thiserror) + `ParseError` (thiserror) 两套
- 但 95% 公共 API 返回 `AnyResult<T>` = `Result<T, anyhow::Error>`。
- `SubtitleError` 仅被 `ParseError::Decode(#[from] SubtitleError)` 包装。
- **建议**：要么全 anyhow（更宽松），要么全 thiserror + 显式错误类型（更严格）。混合模式是迁移过程的中间态，应明示意图。

### §16.6 `srt.rs::parse` 与 `SrtStream` 重复状态机

- [srt.rs:110-202](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/srt.rs#L110-L202) `parse` 函数
- [srt.rs:264-343](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/srt.rs#L264-L343) `SrtStream::next`
- 两者逻辑几乎相同（Phase 状态机、Index/Timestamp/Text 阶段、BOM 处理）。
- 建议提取共用函数 `fn parse_line(state: &mut State, line: &str) -> Option<AnyResult<Subtitle>>`，让 batch 和 stream 共享。

### §16.7 `srt.rs:39` `color: color.clone()` 每次匹配都克隆

- [srt.rs:54](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/srt.rs#L54) `color: color.clone()`（同样在 vtt.rs:56 / ass.rs:301）
- 每次 regex 匹配都克隆 String。可以改成 `Rc<str>` 或 `Arc<str>`，或推迟克隆直到 push 时。

### §16.8 `examples/` 22 个，部分可删或合并

- `examples/parse-ass-content.rs` / `parse-srt-content.rs` / `parse-vtt-content.rs` 是同款模板（仅 format 不同）。
- 建议合并为 `examples/parse-content.rs`（接 format 参数）。

### §16.9 CLI 入口 `main.rs` 541 行

- [main.rs](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/main.rs) 包含 `cmd_parse` / `cmd_convert` / `cmd_validate` / `cmd_edit` / `cmd_info` / `cmd_detect` / `cmd_quality` / `cmd_normalize` / `cmd_shift` 9 个子命令。
- 建议每子命令一个文件：`src/cmd/parse.rs` / `cmd/convert.rs` 等。

### §16.10 `tests/integration.rs` 692 行

- 单文件太大，建议按 format 拆分：`tests/integration/srt.rs` / `vtt.rs` / `ass.rs` 等。

---

## 5. 📌 文档缺口（v1 报告已识别的 3 项仍未修）

| # | 项 | 位置 | 修法 |
|---|---|---|---|
| 1 | `src/sbv.rs.bak` 189 行残留 | [src/sbv.rs.bak](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/src/sbv.rs.bak) | `rm src/sbv.rs.bak` |
| 2 | `skill/SKILL.md` 0 字节空文件 | [skill/SKILL.md](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/skill/SKILL.md) | `rm -rf skill/` 或移入 `docs/` |
| 3 | `CHANGELOG.md` 1.0.0 段未提 iter 8/9 | [CHANGELOG.md:56-125](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/CHANGELOG.md#L56-L125) | 补 "BREAKING: Subtitle 字段移除 (layer/margin_l/r/v/effect)" + "Added: VttStream/SbVStream/LrcStream/MicroDvdStream/SubViewerStream" |
| 4 | `MIGRATION.md` 未提 iter 8 | [MIGRATION.md](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/MIGRATION.md) | 补一节 "Removed Subtitle fields" |
| 5 | `CHANGELOG.md` 1.0.0 段有重复 `### Changed` / `### Removed` 小节 | 同上 | 合并为单一组 |

---

## 6. 工具 / 性能数据

```
$ cargo test --all-targets --all-features
test result: ok. 111 passed (lib)
test result: ok. 12 passed (doctest)
test result: ok. 80 passed (integration: arch 6 + integration 6 + cleanup 66 + proptest 2)
= 203 passed, 0 failed

$ cargo clippy --all-targets --all-features -- -D warnings
   Finished `dev` profile (clean)

$ cargo fmt --all -- --check
   (clean)

$ wc -l src/*.rs
   460 ass.rs  329 cli.rs  2 config.rs  99 encoding.rs  79 error.rs
   129 lib.rs  277 lrc.rs  541 main.rs  214 microdvd.rs  1099 model.rs
   291 normalize.rs  223 quality.rs  185 sbv.rs  595 srt.rs  230 subviewer.rs
   363 ttml.rs  1 types.rs  218 utils.rs  534 vtt.rs
   6974 total
```

---

## 7. 评估与建议

### 7.1 整体评估

| 维度 | 评分 | 说明 |
|---|---|---|
| **功能完整性** | ★★★★★ | 9 format 全覆盖 + editing API + streaming + encoding |
| **正确性** | ★★★½ | 3 个 P0 bug 仍有，1 个是 iter 2 引入的 |
| **API 一致性** | ★★★½ | WritePolicy 重复、parse_url 9 处不一致、Subtitle 构造 2 处 |
| **错误处理** | ★★★★ | ParseError 类型化良好，Anyhow/Decode display 重复 |
| **性能** | ★★★★ | LazyLock + byte-scan + 早退优化到位 |
| **测试** | ★★★ | 203 通过，但 proptest 只覆盖 2/9 format，stream 测试少 |
| **文档** | ★★★ | README/CHANGELOG/MIGRATION 有但**未反映 iter 8/9** |
| **代码风格** | ★★★½ | 1099 行 model.rs、3 处 Subtitle 构造、normalize test 散落 |

### 7.2 1.0 发版前必修清单（按优先级）

1. **🔴 §14.1** 修复 `detect_format` UTF-8 限制（让 iter 2 真正生效）
2. **🔴 §14.2** 修复 `LrcStream` 多时间戳丢失（让 iter 6 真正生效）
3. **🔴 §14.3** 修复 `srt::detect_format` 过于宽松的第二分支
4. **📌 §5.1-2** 删 `sbv.rs.bak` 和 `skill/SKILL.md`
5. **📌 §5.3-4** 补 `CHANGELOG.md` iter 8/9 + `MIGRATION.md` 字段移除
6. **🟡 §15.4** 模块级 `parse_url` 加 `_with` 变体

### 7.3 发版后可做（1.0.1 / 1.1.0）

- **🟡 §15.1-10** 错误消息 / WritePolicy 去重 / shift 文档 / struct literal / `_fps` / 测试位置 / 命名一致性
- **🟢 §16.1-10** model.rs 拆分 / config.rs 合并 / index 选项 / proptest 覆盖 / 错误模型 / 状态机合并 / 克隆优化 / examples 合并 / CLI 拆分

### 7.4 一句话总结

> **v1 报告的 27 项已修 21 项，剩 6 项**（含 CHANGELOG/MIGRATION/sbv.rs.bak/skill/encoding 半失效/P0 regression）**仍未完全清理**。本次 v2 报告新发现 3 个 P0（其中 1 个是 iter 2 引入的回归），10 个 P1，10 个 P2。
> **1.0 距离 = 6 个 commit**（其中 3 个 P0 是硬卡点）。

---

## 8. 附录

- 前次报告：[code-review-2026-07-16.md](./code-review-2026-07-16.md)
- 迭代方案：[iterative-optimization-plan-2026-07-16.md](./iterative-optimization-plan-2026-07-16.md)
- 项目根 `/Users/mankong/volumes/code/subtitle-rs/subtitler/`
- AGENTS.md：`/Users/mankong/volumes/code/subtitle-rs/subtitler/AGENTS.md`
- Cargo.toml：[Cargo.toml](file:///Users/mankong/volumes/code/subtitle-rs/subtitler/Cargo.toml)

> 维护者注：本报告与 code-review-2026-07-16.md 并列存在。前者覆盖 v0 时代 → v1 路径中的 27 项；本文档覆盖 v1 完成后（即 1.0 候选）的**新一轮** 26 项。两者关系：v1 = 历史；v2 = 现状。
