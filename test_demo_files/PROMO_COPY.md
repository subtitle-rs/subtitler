# 推广文案

---

## 1. Hacker News — Show HN

```
Show HN: Subtitler – a Rust library and CLI for 15 subtitle formats

I built a library that can parse, convert, validate, and manipulate subtitles in 15 formats – all offline, no API key needed, no file upload to a server.

sub·ti·tler: https://github.com/subtitle-rs/subtitler

What it does in one line:

  subtitler convert game_of_thrones.srt game_of_thrones.vtt
  subtitler validate lost_in_translation.srt --max-cps 25
  subtitler edit interstellar.sub --sort --merge 200 --split 42 --output clean.srt
  subtitler pipeline matrix.ass matrix_clean.srt --config ops.json
  subtitler quality pulp_fiction.srt --json | jq

15 formats: SRT, VTT, ASS/SSA, MicroDVD, SubViewer, TTML, SBV, LRC, SAMI, MPL2, SCC, EBU STL, DFXP, Whisper JSON.

Recent fixes after testing with real-world subtitle files (shout out to EditingTools.io for the test files):
- SCC CEA-608 text decoding was completely broken (P1) – rewritten
- SubViewer auto-detection failed (bracket headers were whitelist-only)
- YouTube SBV two-line format wasn't supported
- iTT SMPTE frame timecodes now parse correctly
- DFXP namespace detection fixed

Now at 340+ tests, all passing. No regressions.

Why Rust? A subtitle library needs to handle encoding (UTF-8/16, GBK, Shift_JIS), time math (ms/vs/frames, drop-frame), and complex format parsing – Rust's error handling and performance are a great fit.

crates.io: https://crates.io/crates/subtitler
docs: https://docs.rs/subtitler

Looking for feedback on the API design, format coverage gaps, and use cases I haven't thought of.
```

---

## 2. Reddit r/rust

```
Subtitler – a Rust library for 15 subtitle formats (parse, convert, edit, validate)

I've been working on `subtitler`, a Rust library + CLI for handling subtitles:

- 15 formats: SRT, VTT, ASS, SSA, MicroDVD, SubViewer, TTML, SBV, LRC, SAMI, MPL2, SCC, EBU STL, DFXP, Whisper JSON
- Auto-detect format from content (not just extension)
- Convert between any two formats
- Validate timing (overlaps, negative duration, CPS, text length)
- Normalize text (fix OCR, strip HI, normalize quotes/whitespace, filter language)
- Edit: sort, shift, merge adjacent, split long, transform framerate, remove overlaps, enforce min/max duration
- Declarative pipeline via JSON config
- Streaming parsers for large files
- WASM bindings for browser use
- Encoding detection (UTF-8/16 BOM, chardetng fallback)
- 340+ tests, property-based testing with proptest
- No unsafe code

crates.io: https://crates.io/crates/subtitler
GitHub: https://github.com/subtitle-rs/subtitler
docs: https://docs.rs/subtitler

Usage:
```rust
use subtitler::{parse_bytes, SubtitleFormat};

let data = std::fs::read("sub.srt")?;
let file = parse_bytes(&data)?;
println!("{} subs in {:?} format", file.subtitles().len(), file.format());

let issues = file.validate_extended(42, 5000, 25.0);
```

Or from the CLI:
```
subtitler convert input.srt output.vtt
subtitler validate input.srt --json
subtitler edit input.srt --sort --shift -500 --output clean.srt
```

The crate handles the annoying edge cases: drop-frame timecodes (SCC), centiseconds (SubViewer), SMPTE frame timecodes (iTT), binary STL format, and multiple East Asian encodings.

I'd love feedback on format coverage – are there formats I'm missing that you deal with? Any API design issues that stand out?
```

---

## 3. Reddit r/subtitling / r/ffmpeg

```
subtitler – a free, open-source CLI for 15 subtitle formats (works offline, no upload needed)

I made a tool called `subtitler` for anyone who works with subtitles on the command line:

Install: `cargo install subtitler`

What you can do:

Convert formats:
  subtitler convert input.srt output.vtt
  subtitler convert input.srt output.ass
  subtitler convert input.ass output.ttml

Validate timing:
  subtitler validate movie.srt --max-cps 25 --max-chars 42
  # checks: overlaps, negative duration, CPS too high, text too long

Inline edit:
  subtitler edit movie.srt --sort --shift -500 --merge 100 --output clean.srt
  subtitler edit movie.srt --split 42 --output split.srt
  
Normalize text:
  subtitler normalize movie.srt --fix-ocr --strip-hi --output clean.srt

Quality report:
  subtitler quality movie.srt --json

Supports: SRT, VTT, ASS/SSA, MicroDVD, SubViewer, TTML/IMSC, SBV (YouTube), LRC, SAMI, MPL2, SCC, EBU STL, DFXP, Whisper JSON

It auto-detects the format from content, handles UTF-16/Shift_JIS/GBK encodings, and runs fully offline – no files leave your machine.

GitHub: https://github.com/subtitle-rs/subtitler
```

---

## 4. This Week in Rust (TWIR)

### Crate of the Week submission

**subtitler** — Rust library and CLI for parsing, converting, and editing subtitles in 15 formats. Features auto-detection, streaming parsers, pipeline operations, WASM support, and 340+ tests.

`cargo add subtitler`

https://github.com/subtitle-rs/subtitler

---

## 5. Twitter / X (short)

```
subtitler: a Rust CLI that handles 15 subtitle formats offline.

  subtitler convert input.srt output.vtt
  subtitler validate movie.srt --max-cps 25
  subtitler edit movie.srt --sort --split 42

SRT, VTT, ASS, TTML, SCC, EBU STL, DFXP, Whisper JSON, and more.
No upload needed. Works offline.

https://github.com/subtitle-rs/subtitler
#Rust #subtitles
```

---

## 6. V2EX (中文)

```
分享一个用 Rust 写的字幕处理命令行工具：subtitler

支持 15 种字幕格式的解析、转换、校验和编辑：

格式：SRT, VTT, ASS/SSA, MicroDVD, SubViewer, TTML, SBV, LRC, SAMI, MPL2, SCC, EBU STL, DFXP, Whisper JSON

用法示例：

  # 格式转换
  subtitler convert input.srt output.vtt

  # 质量校验
  subtitler validate input.srt --max-cps 25 --max-chars 42

  # 批量编辑并转换
  subtitler edit input.srt --sort --shift -500 --merge 100 --output clean.ass

  # 声明式流水线（JSON 配置多步操作）
  subtitler pipeline input.ass output.srt --config ops.json

  # 质量报告（支持 JSON 输出）
  subtitler quality input.srt --json

  # 文本标准化（OCR 修复、去听力障碍标记、引号标准化等）
  subtitler normalize input.srt --all --output clean.srt

特点：
- 纯离线运行，不上传文件
- 自动检测格式（内容签名优先，不依赖扩展名）
- 自动检测编码（UTF-8/16、Shift_JIS、GBK）
- 支持 WASM，可在浏览器中使用
- 340+ 测试，属性测试保障稳定性
- 完全开源免费

GitHub: https://github.com/subtitle-rs/subtitler
crates.io: https://crates.io/crates/subtitler
```

---

## 7. 知乎 — "有哪些好用的字幕处理工具？"

```
推荐一个我开源的命令行字幕处理工具：subtitler

用 Rust 编写，支持 15 种字幕格式的解析、转换、校验和编辑。

相比其他工具的优势：
1. 完全离线运行 —— 不需要上传文件到服务器
2. 自动检测格式 —— 不用手动指定输入是 SRT 还是 ASS
3. 自动检测编码 —— Shift_JIS、GBK 都支持，日剧字幕直接处理
4. 声 明式流水线 —— 用 JSON 配置多步操作，可复现

支持的命令：
- convert: 格式转换
- validate: 定时校验（重叠、负时长、CPS、字数）
- edit: 排序、偏移、合并、拆分、帧率转换
- normalize: OCR 修复、去听力障碍标记、引号标准化、空白规范化
- quality: 质量报告（JSON 输出）
- detect: 格式检测
- pipeline: 声明式流水线

GitHub: https://github.com/subtitle-rs/subtitler
安装：cargo install subtitler
```
