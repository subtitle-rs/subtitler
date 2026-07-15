# Subtitler

> A Rust library for parsing, manipulating, and generating subtitles in multiple formats.

[![Crates.io](https://img.shields.io/crates/v/subtitler?style=flat-square)](https://crates.io/crates/subtitler)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue?style=flat-square)](LICENSE)

- SRT, WebVTT, ASS/SSA, **MicroDVD**, and **SubViewer** format support
- Rich text extraction (bold, italic, underline, color, voice tags)
- Encoding detection and auto-decoding (UTF-8, UTF-16, BOM, chardetng fallback)
- Format detection, conversion, and validation
- Frame-based timecode support
- Utility operations: sort, merge, split, validate, framerate transform
- Async I/O powered by `tokio`
- Serialize/Deserialize via `serde`

## Installation

```sh
cargo add subtitler
```

Or as a CLI tool:

```sh
cargo install subtitler
```

## Quick Start

### Parse SRT content

```rust
use subtitler::srt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let content = "1\n00:00:01,000 --> 00:00:03,500\nHello, world!\n\n";
    let subtitles = srt::parse_content(content).await?;
    println!("{:?}", subtitles);
    Ok(())
}
```

### Parse WebVTT content

```rust
use subtitler::vtt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let content = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\nHello, world!\n\n";
    let subtitles = vtt::parse_content(content).await?;
    println!("{:?}", subtitles);
    Ok(())
}
```

### Parse ASS/SSA content

```rust
use subtitler::ass;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let content = "[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: Name, Fontname, ...\nStyle: Default,Arial,20,...\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,Hello!\n";
    let file = ass::parse_content(content)?;
    println!("{}", file.to_string());
    Ok(())
}
```

### Convert between formats

```rust
use subtitler::{srt, vtt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let content = std::fs::read_to_string("input.srt")?;
    let subtitles = srt::parse_content(&content).await?;

    // Convert to VTT string
    let vtt_str = vtt::to_string(&subtitles, None);
    std::fs::write("output.vtt", vtt_str)?;
    Ok(())
}
```

### Generate subtitle files

```rust
use subtitler::model::Subtitle;
use subtitler::srt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subtitles = vec![
        Subtitle::new(1000, 3500, "Hello!"),
        Subtitle::new(4000, 6500, "World!"),
    ];
    srt::generate(&subtitles, "output.srt").await?;
    Ok(())
}
```

### Auto-detect format

```rust
use subtitler::detect_format;
use subtitler::model::SubtitleFormat;

let data = std::fs::read("unknown.sub")?;
match detect_format(&data) {
    Some(SubtitleFormat::Srt) => println!("SRT detected"),
    Some(SubtitleFormat::Vtt) => println!("WebVTT detected"),
    Some(SubtitleFormat::Ass) => println!("ASS/SSA detected"),
    None => println!("Unknown format"),
}
```

## API Reference

### Data Model

```rust
pub struct Subtitle {
    pub index: Option<usize>,     // subtitle number
    pub start: u64,               // start time in milliseconds
    pub end: u64,                 // end time in milliseconds
    pub text: String,             // subtitle text (stripped of tags)
    pub settings: Option<String>, // VTT cue settings
    pub text_parts: Vec<TextPart>, // structured rich text parts

    // ASS/SSA fields
    pub style: Option<String>,    // style name reference
    pub actor: Option<String>,    // speaker/actor name
    pub layer: Option<i32>,       // z-ordering
    pub margin_l: Option<i32>,    // left margin
    pub margin_r: Option<i32>,    // right margin
    pub margin_v: Option<i32>,    // vertical margin
    pub effect: Option<String>,   // effect name
    pub is_comment: bool,         // comment flag
}

pub struct TextPart {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: Option<String>,
    pub voice: Option<String>,
}

pub struct AssStyle {
    pub name: String,
    pub fontname: String,
    pub fontsize: u32,
    pub primary_color: String,
    pub secondary_color: String,
    // ... 23 fields total
}

pub enum SubtitleFile {
    Srt(Vec<Subtitle>),
    Vtt { header: Option<String>, subtitles: Vec<Subtitle> },
    Ass { info: HashMap<String, String>, styles: Vec<AssStyle>, subtitles: Vec<Subtitle> },
}
```

### SRT Module (`subtitler::srt`)

| Function | Description |
|----------|-------------|
| `parse_file(path)` | Parse SRT from file |
| `parse_bytes(data)` | Parse SRT from bytes |
| `parse_content(content)` | Parse SRT from string |
| `parse_url(url)` | Parse SRT from HTTP URL (requires `http` feature) |
| `generate(subtitles, path)` | Write SRT to file |
| `to_string(subtitles)` | Format subtitles as SRT string |
| `detect_format(data)` | Detect if data is SRT |

### WebVTT Module (`subtitler::vtt`)

| Function | Description |
|----------|-------------|
| `parse_file(path)` | Parse VTT from file |
| `parse_bytes(data)` | Parse VTT from bytes |
| `parse_content(content)` | Parse VTT from string |
| `parse_content_full(content)` | Parse VTT returning header + subtitles |
| `parse_url(url)` | Parse VTT from HTTP URL (requires `http` feature) |
| `generate(subtitles, path)` | Write VTT to file |
| `to_string(subtitles, header)` | Format subtitles as VTT string |
| `detect_format(data)` | Detect if data is VTT |

### ASS Module (`subtitler::ass`)

| Function | Description |
|----------|-------------|
| `parse_content(content)` | Parse ASS/SSA from string, returns `SubtitleFile` |
| `parse_file(path)` | Parse ASS/SSA from file (async) |
| `parse_bytes(data)` | Parse ASS/SSA from byte slice |
| `parse_url(url)` | Parse ASS/SSA from HTTP URL (requires `http` feature) |
| `to_string(info, styles, subtitles)` | Format as ASS string |
| `detect_format(data)` | Detect if data is ASS/SSA |

### MicroDVD Module (`subtitler::microdvd`)

| Function | Description |
|----------|-------------|
| `parse_content(content, fps)` | Parse MicroDVD frame-based content |
| `to_string(subtitles, fps)` | Format as MicroDVD string |
| `to_string_with_fps_header(subtitles, fps)` | Format with FPS declaration header |
| `detect_format(data)` | Detect if data is MicroDVD |

### SubViewer Module (`subtitler::subviewer`)

| Function | Description |
|----------|-------------|
| `parse_content(content)` | Parse SubViewer 1.0/2.0 content |
| `to_string(subtitles)` | Format as SubViewer 2.0 with headers |
| `detect_format(data)` | Detect if data is SubViewer |

### Encoding Utilities (`subtitler::encoding`)

| Function | Description |
|----------|-------------|
| `detect_encoding(data)` | Auto-detect character encoding (UTF-8/16/BOM/chardetng) |
| `decode_to_string(data)` | Decode bytes to string using detected encoding |

### Timestamp Utilities (`subtitler::utils`)

| Function | Description |
|----------|-------------|
| `parse_timestamp(ts)` | Parse `"00:00:01,500"` → `1500` ms |
| `parse_timestamps(ts)` | Parse `"00:00:01,000 --> 00:00:03,500"` → `Timestamp` |
| `format_timestamp(ms, fmt)` | Format ms to `"00:00:01,000"` (SRT) or `"00:00:01.000"` (VTT) |
| `pad_left(value, length)` | Zero-pad integer to fixed width |

### Frame Utilities (`subtitler::model`)

| Function | Description |
|----------|-------------|
| `ms_to_frames(ms, fps)` | Convert milliseconds to frame count |
| `frames_to_ms(frames, fps)` | Convert frame count to milliseconds |

### SubtitleFile Methods (`subtitler::model::SubtitleFile`)

| Method | Description |
|--------|-------------|
| `subtitles()` | Get reference to subtitle list |
| `subtitles_mut()` | Get mutable reference |
| `format()` | Get detected format enum |
| `shift_all(offset_ms)` | Shift all timestamps |
| `sort()` | Sort by start time |
| `validate()` | Check for timing issues |
| `validate_extended(max_chars, max_gap, max_cps)` | Extended validation |
| `merge_adjacent(max_gap_ms)` | Merge subtitles within gap threshold |
| `remove_overlaps()` | Fix overlapping subtitles by adjusting start times |
| `split_long(max_chars)` | Split long subtitles at word boundaries |
| `transform_framerate(in_fps, out_fps)` | Rescale timestamps for framerate change |
| `map(fn)` | Transform each subtitle (consuming) |
| `filter(fn)` | Filter subtitles (consuming) |
| `to_string()` | Format to appropriate format string |

### Subtitle Methods (`subtitler::model::Subtitle`)

| Method | Description |
|--------|-------------|
| `new(start, end, text)` | Create new subtitle |
| `shift(offset_ms)` | Shift this subtitle's timing |
| `duration_ms()` | Get duration in milliseconds |
| `chars_per_second()` | Calculate characters-per-second rate |
| `reading_speed_wpm()` | Calculate reading speed in words per minute |
| `strip_tags()` | Remove HTML/ASS formatting tags from subtitle text |

### Validation Issues (`subtitler::model::ValidationIssue`)

| Variant | Description |
|---------|-------------|
| `Overlap` | Two subtitles have overlapping time ranges |
| `NegativeDuration` | End time is before start time |
| `ZeroDuration` | Start and end times are equal |
| `DecreasingStartTime` | Start times are not monotonically increasing |
| `TooLongGap` | Gap between subtitles exceeds threshold |
| `TextTooLong` | Subtitle text exceeds character limit |
| `CpsTooHigh` | Characters-per-second exceeds threshold |

### Format Detection (`subtitler::detect_format`)

```rust
pub fn detect_format(data: &[u8]) -> Option<SubtitleFormat>
```

Auto-detects SRT (by index+timestamp pattern), WebVTT (by `WEBVTT` header), or ASS/SSA (by `[Script Info]` section).

## CLI Usage

### Parse subtitles

```sh
# Parse SRT file and display contents
subtitler parse movie.srt

# Parse with JSON output
subtitler parse movie.vtt --json

# Parse from URL
subtitler parse https://example.com/subtitles.srt

# Parse from stdin
cat movie.srt | subtitler parse -

# Force format
subtitler parse data.txt --format srt
```

### Convert between formats

```sh
# Auto-detect source, infer target from extension
subtitler convert input.srt output.vtt

# Explicit source and target
subtitler convert input.srt output.ass --from srt --to ass

# Convert with time shift
subtitler convert input.srt output.vtt --shift -500

# Pipe to stdout
subtitler convert input.srt -
```

### Validate subtitles

```sh
# Basic timing validation
subtitler validate movie.srt

# Extended validation with custom thresholds
subtitler validate movie.srt --max-chars 42 --max-gap 5000 --max-cps 25

# Basic checks only (no text limits)
subtitler validate movie.srt --basic

# JSON output
subtitler validate movie.srt --json
```

### Edit & transform

```sh
# Sort by time
subtitler edit input.srt --output output.srt --sort

# Shift all timestamps (+500ms delay, -200ms advance)
subtitler edit input.srt --output output.srt --shift 500
subtitler edit input.srt --output output.srt --shift=-200

# Merge adjacent subtitles (gap <= 300ms)
subtitler edit input.srt --output output.srt --merge 300

# Split long subtitles (max 42 chars per line)
subtitler edit input.srt --output output.srt --split 42

# Multiple operations
subtitler edit input.srt --output output.vtt --sort --shift -300 --merge 100

# Framerate conversion
subtitler edit input.srt --output output.srt --transform-fps 23.976 25.0
```

### File info & statistics

```sh
subtitler info movie.srt
# Output:
#   File:         movie.srt
#   Format:       srt
#   Subtitles:    150
#   Time range:   0ms -> 5400000ms
#   Duration:     5400000ms (5400.0s)
#   Avg duration: 2500ms
#   Min duration: 500ms
#   Max duration: 8000ms
#   Total chars:  4500
#   Max CPS:      22.5
#   Timing issues: 0
```

### Detect format

```sh
subtitler detect unknown.sub   # prints: srt, vtt, ass, or ssa
```

## Features

| Feature | Default | Description |
|---------|---------|-------------|
| `http` | Yes | Enable `parse_url()` via `reqwest` |

## Examples

See the [examples](https://github.com/subtitle-rs/subtitler/tree/main/examples) directory for more usage patterns:

- `parse-srt-file` — Parse SRT from file
- `parse-srt-content` — Parse SRT from inline content
- `parse-srt-http` — Parse SRT from URL
- `create-srt-file` — Generate SRT file
- `parse-vtt-file` — Parse VTT from file
- `parse-vtt-content` — Parse VTT from inline content
- `parse-vtt-http` — Parse VTT from URL
- `create-vtt-file` — Generate VTT file
- `parse-ass-content` — Parse ASS from inline content
- `format-convert` — Convert between SRT/VTT/ASS formats
- `utility-ops` — Sort, validate, merge, and split operations
- `frame-conversion` — Frame-based timecode conversion

## License

Apache 2.0
