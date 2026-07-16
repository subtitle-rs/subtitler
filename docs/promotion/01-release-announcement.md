# subtitler 1.0: Rust 生态最完整的字幕处理库

> 9 种格式 · 统一 API · 流式解析 · 质量报告 · 编码检测 · 全功能 CLI

经过多个迭代周期的打磨，`subtitler` 1.0.0 正式发布。这是 Rust 生态中支持格式最多、功能最全面的字幕解析与处理库。

## 为什么需要另一个字幕库？

Rust 生态已有 `subparse`、`aspasia`、`ass-core` 等库，但它们各有局限：

| 能力 | subtitler | aspasia | subparse | ass-core |
|------|-----------|---------|----------|----------|
| 格式数 | **9** | ~6 | ~5 | 1 |
| CLI 工具 | **完整** | 无 | 无 | 无 |
| 质量报告 | **有** | 无 | 无 | 无 |
| 流式解析 | **有** | 无 | 无 | 有 |
| 编码检测 | **有** | 有 | 无 | 无 |
| Feature flags | **每格式** | 有 | 有 | 有 |

`subtitler` 的目标是：**一个库覆盖字幕处理的全流程**——从解析、转换、质量分析到生成，且每种格式可独立裁剪。

## 支持的 9 种格式

| 格式 | 场景 | 扩展名 |
|------|------|--------|
| **SRT** | 通用字幕 | `.srt` |
| **WebVTT** | HTML5 视频 | `.vtt` |
| **ASS/SSA** | 动画/卡拉OK | `.ass` / `.ssa` |
| **MicroDVD** | 老式帧基字幕 | `.sub` |
| **SubViewer** | 法国/欧洲标准 | `.sub` |
| **TTML/IMSC** | Netflix/流媒体 | `.ttml` / `.xml` |
| **SBV** | YouTube | `.sbv` |
| **LRC** | 歌词同步 | `.lrc` |

## 三行代码上手

```rust
use subtitler::SubtitleFormat;

// 自动检测格式，一行解析
let data = std::fs::read("subtitle.srt")?;
let file = subtitler::parse_bytes(&data)?;

// 验证质量
let issues = file.validate();
println!("Found {} issues", issues.len());

// 转换格式
let vtt = file.to_string_with_format(&subtitler::model::Format::Vtt);
std::fs::write("output.vtt", vtt)?;
```

## 核心特性

### 统一 API

所有格式共享 `SubtitleFormat` trait——15 个编辑方法（`validate`、`shift_all`、`merge_adjacent`、`split_long`...）自动适用于全部 9 种格式。加新格式只需实现 4 个必需方法。

### 格式互转

任意格式之间无损转换：

```rust
let file = subtitler::parse_file("input.srt").await?;
// SRT → ASS → TTML → VTT，一条链
let ass = file.to_string_with_format(&Format::Ass);
let ttml = file.to_string_with_format(&Format::Ttml);
```

### 质量报告

生成结构化的字幕质量分析（JSON 可序列化）：

```rust
use subtitler::quality::generate_report;

let report = generate_report(&subs, 42, 5000, 25.0);
println!("平均 CPS: {:.1}", report.avg_cps);
// 每条字幕都有 CPS、WPM、时长、问题列表
```

### 流式解析

大文件不分配完整 Vec，逐条处理：

```rust
for sub in subtitler::srt::parse_stream(large_content) {
    let sub = sub?;
    println!("{}", sub.text);
}
```

### 文本规范化

OCR 修复、听力障碍标签移除、引号统一、换行优化：

```rust
use subtitler::normalize::{fix_ocr_errors, strip_hearing_impaired, optimize_line_breaks};

let clean = fix_ocr_errors("12O456");     // → "120456"
let clean = strip_hearing_impaired("(LAUGHS) Hello"); // → "Hello"
let wrapped = optimize_line_breaks(long_text, 42);     // 智能断行
```

### 编码检测

自动识别 UTF-8/UTF-16/GBK 等：

```rust
use subtitler::encoding::decode_to_string;
let text = decode_to_string(&raw_bytes)?; // 自动检测编码
```

## CLI 工具

不只是库——还附带完整命令行工具：

```bash
# 解析
subtitler parse movie.srt
subtitler parse movie.srt --json

# 格式转换
subtitler convert input.srt output.vtt
subtitler convert input.srt output.ttml

# 质量验证
subtitler validate movie.srt --max-cps 20

# 编辑操作
subtitler edit input.srt --shift 1000 --sort --output shifted.srt

# 统计信息
subtitler info movie.srt

# 格式检测
subtitler detect unknown.sub
```

## 编译裁剪

只用 SRT + VTT？关掉其余格式减小体积：

```toml
[dependencies]
subtitler = { version = "1.0", default-features = false, features = ["srt", "vtt"] }
```

## 性能

- 手写字节扫描时间戳解析器（替代 regex 热路径）
- `LazyLock` 缓存所有 regex（零运行时编译）
- `SrtStream` 流式迭代器（大文件零分配）
- tokio features 精简为 `["fs", "io-util", "rt-multi-thread", "macros"]`

## 安装

```bash
# 库
cargo add subtitler

# CLI
cargo install subtitler
```

## 链接

- **crates.io**: https://crates.io/crates/subtitler
- **文档**: https://docs.rs/subtitler
- **仓库**: https://github.com/subtitle-rs/subtitler
- **迁移指南**: 从 0.1.x 升级请阅读 `MIGRATION.md`

## 致谢

感谢 Rust 社区的 `quick-xml`、`regex`、`chardetng`、`clap` 等优秀库，它们是 subtitler 的基石。

---

*Apache-2.0 许可 · 195 个测试 · 9 种格式 · Rust 2024 edition*
