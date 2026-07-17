# subtitler v2.0：从稳健到极致

**subtiliter** 是一个 Rust 写的字幕处理库兼 CLI 工具，覆盖 12 种字幕格式的解析、转换、校验、编辑和生成。v1.0 完成了 API 统一和架构梳理；v2.0 在此基础上做了三件事：**去掉最后一层不必要的内存分配**、**用结构化错误替代 anyhow**、以及 **引入流式解析 + Builder + Pipeline 三件套**。

这篇文章不罗列 diff，而是从设计决策的角度讲清楚每次重构「为什么做」和「带来什么」。

---

## 一、模块拆分：一个文件太挤了

v1.0 的 `model.rs` 塞了 600+ 行——Subtitle、SubtitleFile、Format、SubtitleFormat trait、ValidationIssue、Timestamp、连 `split_text_chunks` 这种工具函数也放在里面。改一处要翻好久。

v2.0 拆成 9 个子模块：

```
model/
  mod.rs          # re-export
  subtitle.rs     # Subtitle + TextPart + TextFormat
  format.rs       # SubtitleFile enum + Format enum
  trait.rs        # SubtitleFormat trait
  types.rs        # Timestamp + WritePolicy（AssData/AssStyle 移到 format.rs）
  convert.rs      # split_text_chunks、frames_to_ms 等
  builder.rs      # SubtitleFileBuilder
  streaming.rs    # StreamingParser trait
  validation.rs   # ValidationIssue
```

这个拆分基本遵循「一个 struct/trait 一个文件」的原则，每个文件职责清晰，`SubtitleFile` 和 `AssData` 放在一起因为它们在序列化上强耦合。单元测试也分散到各自模块里，不再堆在底部 200 行。

---

## 二、统一返回类型：所有 parse_* 都返回 SubtitleFile

v1.0 有个历史遗留问题：VTT 和 MPL2 的 `parse_bytes` 返回 `Vec<Subtitle>`，其他模块返回 `SubtitleFile`。调用方要写两套代码：

```rust
// v1.0：不一致
let file: SubtitleFile = subtitler::srt::parse_content(&text)?;   // 包装好的
let subs: Vec<Subtitle> = subtitler::vtt::parse_content(&text)?;  // 裸 Vec
```

v2.0 统一为全部返回 `SubtitleFile`：

```rust
// v2.0：一致
let file: SubtitleFile = subtitler::srt::parse_content(&text)?;
let file: SubtitleFile = subtitler::vtt::parse_content(&text)?;
let file: SubtitleFile = subtitler::mpl2::parse_bytes(data)?;
```

这意味着所有模块都可以走同一套 `SubtitleFormat` trait 方法——`validate()`、`merge_adjacent()`、`to_string_with_format()` 等对于任意格式都是同一条调用路径。`main.rs` 里的 `cmd_parse` 也因此去掉了重复的 `SubtitleFile` 构造逻辑。

---

## 三、结构化错误：从 anyhow! 到 SubtitleError

v1.0 的错误处理是 `anyhow!("unexpected line at row {}: {:?}", row, line)`。这在原型期很方便，但有两个问题：

1. **调用方无法做错误恢复**。比如想要尝试用另一个编码重试解析，`anyhow::Error` 需要靠 `downcast_ref` 猜测。
2. **错误消息格式不一致**。每个模块各写各的 `anyhow!`，有的带行号有的不带。

v2.0 的解决方案是扩展 `SubtitleError` 到 11 个变体：

| 变体 | 使用场景 |
|------|---------|
| `InvalidTimestamp { format, value }` | 时间戳解析失败，错误消息带上格式名 |
| `UnexpectedLine { format, row, expected, got }` | SRT/VTT 意外行内容，给出上下文 |
| `InvalidLine { format, line }` | SBV 等无行号格式 |
| `Xml { format, error }` | TTML XML 解析/解码错误 |
| `InvalidFrame { format, role, value }` | MicroDVD/MPL2 帧解析失败 |
| `InvalidEncoding { encoding, error }` | UTF-8/UTF-16 解码失败 |
| `UnsupportedEncoding { encoding }` | 不可识别的编码 |
| `InvalidFormat { format, reason }` | EBU STL 等格式验证失败 |
| `FileExists { path }` | 覆写保护 |
| `InvalidUtf8` | UTF-8 转换错误 |
| `Io` | `std::io::Error` 包装 |

公共 API 仍然返回 `AnyResult`（`Result<T, anyhow::Error>`），因为 `SubtitleError` 实现了 `std::error::Error`，`?` 运算符自动 `Into<anyhow::Error>`。调用方如果不需要精确匹配，可以完全忽略这个变化；如果需要恢复逻辑：

```rust
match subtitler::srt::parse_content(&text) {
  Err(e) => {
    if let Some(SubtitleError::InvalidEncoding { encoding, .. }) = e.downcast_ref() {
      // 尝试其他编码
    }
  }
  _ => {}
}
```

同时 `utils::parse_timestamp` 现在要求传入 `Format` 参数，以便错误消息显示正确格式名（比如 SRT 用逗号分隔毫秒，VTT 用点）：

```rust
// v1.0
parse_timestamp("00:00:01,000")?;

// v2.0
parse_timestamp("00:00:01,000", Format::Srt)?;
```

---

## 四、零拷贝解析：告别逐行 .to_string()

这是 v2.0 性能提升的核心。

一个典型的 1000 条字幕的 SRT 文件大约 2000 行。v1.0 的解析器对每一行做 `line.trim().to_string()`——这意味着 2000 次堆分配 + 2000 次释放，只为了拿到一个临时 `String` 然后立即丢弃。

v2.0 直接操作 `&str` 切片：

```rust
// v1.0
for line in content.lines() {
  let trimmed = line.trim().to_string();  // 每次分配
  // ... 用完就丢
}

// v2.0
for line in content.lines() {
  let trimmed = line.trim();             // 零分配，只是指针偏移
  // ...
  sub.text.push_str(trimmed);            // 只在需要时拷贝到 Subtitle.text
}
```

VTT 的 `header_lines` 也从 `Vec<String>` 改为 `Vec<&str>`，只在最后需要输出时才 `join("\n")`。

### Vec 预分配

另一个容易被忽视的优化：所有格式模块的 `Vec::new()` 都改成了 `Vec::with_capacity`，根据内容大小估算：

```rust
// v1.0
let mut subtitles = Vec::new();  // capacity=0，触发 2-3 次 realloc

// v2.0
let estimated = (content.len() / 200).max(16);
let mut subtitles: Vec<Subtitle> = Vec::with_capacity(estimated);
// EBU STL 更是从 GSI 头读取精确的 TTI 数量
```

对于 1000 条字幕的文件，这避免了大约 3 次重新分配和 `memcpy`。

---

## 五、流式解析器：大到放不进内存也能处理

v1.0 的 `SrtStream` 用 `u8` 表示解析阶段、没有 BOM 处理、逐行 `.to_string()`。v2.0 彻底重写：

```rust
pub struct SrtStream<'a> {
  lines: std::str::Lines<'a>,   // 零拷贝迭代器
  phase: Phase,                  // 用 enum 替代 u8
  current_subtitle: Option<Subtitle>,
  row: usize,
}
```

`VttStream` 同样升级：从 `phase: u8` 改为 `Phase` 枚举，增加了 header 追踪（`header_lines: Vec<&str>` + `header()` 方法），且正确解析 cue 索引、时间戳和 text_parts。

两个流式解析器都实现了 `StreamingParser` trait，可以这样用：

```rust
let content = tokio::fs::read_to_string("huge.srt").await?;
let mut parser = subtitler::srt::parse_stream(&content);

while let Some(result) = parser.next() {
  let subtitle = result?;
  // 逐个处理，不分配完整 Vec
  writer.write_subtitle(&subtitle).await?;
}
```

---

## 六、SubtitleBuilder：链式编辑

v1.0 的编辑操作是直接调用 `SubtitleFormat` trait 方法，每次返回 `()` 而非 `Self`：

```rust
// v1.0：不能链式调用
let mut file = parse_file("input.srt").await?;
file.sort();
file.shift_all(500);
file.merge_adjacent(200);
file.split_long(42);
```

v2.0 新增 `SubtitleBuilder`，每个方法返回 `Self`：

```rust
// v2.0：链式
let file = SubtitleBuilder::from(file)
  .sort()
  .shift(500)
  .merge_adjacent(200)
  .split_long(42)
  .build();
```

支持的 11 个操作：`sort`、`shift`、`merge_adjacent`、`split_long`、`transform_fps`、`remove_overlaps`、`enforce_min_duration`、`enforce_max_duration`、`auto_extend_cps`、`map`、`filter`。

---

## 七、Pipeline DSL：声明式转换链

Builder 适合代码调用，但 CLI 用户需要可配置的批量处理。v2.0 新增 `Pipeline` DSL，操作列表可序列化为 JSON：

```json
{
  "operations": [
    {"op": "Sort"},
    {"op": "Shift", "offset_ms": 500},
    {"op": "SplitLong", "max_chars": 42},
    {"op": "MergeAdjacent", "max_gap_ms": 200},
    {"op": "FilterEmpty"}
  ]
}
```

CLI 新命令：

```bash
subtitler pipeline input.srt output.vtt --config pipeline.json
```

`Pipeline::apply()` 内部调用 `SubtitleBuilder`，所以代码路径保持一致。`FilterEmpty` 是 Pipeline 专属操作（去除空白字幕），在 Builder 里没有单独的方法——你可以用 `builder.filter(|s| !s.text.trim().is_empty())` 实现相同效果，但 Pipeline 把它封装成一个标准操作符以便 JSON 配置。

---

## 八、其他修复

| 问题 | v1.0 | v2.0 |
|------|------|------|
| `to_string` shadow | `LrcData::to_string()` 四个类型用 `#[allow(clippy)]` | 全部改为 `render()` |
| SCC drop_frame | 硬编码 `true` | 从 `SccData.drop_frame` 继承 |
| EBU STL 空操作 | `tti_timecode_to_ms` 值已经是 ms | 移除函数，内联 `as u64` |
| EBU STL 检测 | 只检查大小和 code_page | 额外验证 TTI 数量匹配头信息 |
| `split_text_chunks` | 每步 `format!()` O(n²) | `String::with_capacity` + `push_str` O(n) |

---

## 升级要点

1. **`parse_content` / `parse_bytes` 返回** `SubtitleFile` 而非 `Vec<Subtitle>`，用 `file.subtitles()` 访问。
2. **`parse_timestamp`** 现在需要 `Format` 参数。
3. **`data.to_string()`** 改为 `data.render()`（针对 LrcData/SamiData/Mpl2Data/SccData）。
4. **SCC `to_string`** 需要 `drop_frame: bool`。
5. **Builder/Pipeline** 是纯新增，不影响已有代码。

---

## 写在最后

subtiliter v2.0 没有新增格式、没有改变核心模型结构。它的主题是**精益求精**——去掉不必要的分配、统一不一致的返回类型、用结构化错误替代字符串拼接、用链式 API 改善人机工程、用声明式 Pipeline 赋能批量处理。229 个测试全部通过，`cargo clippy -D warnings` 零警告。

如果你在 v1.x 上有生产代码，升级指南在 `MIGRATION.md` 的「Migrating from 1.x to 2.0.0」章节——大部分情况下只需改几个函数签名。
