# subtitler 1.0 — the most complete subtitle library in Rust (9 formats, unified API, CLI, streaming)

I'm excited to share **subtitler 1.0.0** — a Rust library and CLI for parsing, converting, validating, and generating subtitles. After months of iteration, it supports **9 formats** with a fully unified API.

## What makes it different

Rust already has `subparse`, `aspasia`, and `ass-core`. Here's why I built subtitler:

**One library for the entire subtitle workflow:**

```
parse → validate → fix → convert → generate
```

No other Rust subtitle library covers all five stages.

## 9 formats supported

SRT · WebVTT · ASS/SSA · MicroDVD · SubViewer · **TTML/IMSC** (Netflix/streaming) · **SBV** (YouTube) · **LRC** (lyrics)

## Quick example

```rust
// Auto-detect format, parse in one call
let file = subtitler::parse_bytes(&data)?;

// Validate quality (CPS, overlaps, gaps)
let issues = file.validate();

// Convert to any format
let vtt = file.to_string_with_format(&Format::Vtt);

// Stream-parse large files without allocation
for sub in subtitler::srt::parse_stream(&huge_file) {
    println!("{}", sub?.text);
}
```

## Architecture highlights

- **`SubtitleFormat` trait**: 15 editing methods (validate, shift, merge, split, sort...) with default implementations. Adding a format = implement 4 required methods.
- **Per-format Cargo features**: `default-features = false, features = ["srt", "vtt"]` trims everything you don't need.
- **Format-specific data preserved**: MicroDVD keeps its fps, SubViewer keeps its `[INFORMATION]` header, VTT keeps `WEBVTT` metadata. Round-trips are lossless.
- **Sync parsing, async I/O**: Parsing cores are sync (they never did real async I/O anyway). Only `parse_file`/`parse_url` are async.
- **Hand-written timestamp parser**: The hot path doesn't touch regex — direct byte scanning.
- **Quality reports**: `generate_report()` produces a JSON-serializable per-subtitle analysis (CPS, WPM, duration, issues).

## CLI included

```bash
subtitler parse movie.srt --json
subtitler convert input.srt output.ttml
subtitler validate movie.srt --max-cps 20
subtitler edit input.srt --shift 2000 --output output.vtt
subtitler detect unknown.sub
```

## Stats

- **195 tests**, 0 failures
- 6 feature-subset builds all pass (`--features srt,vtt`, `--features ttml`, etc.)
- Zero `unsafe`, zero `panic!`/`unreachable!` in production code
- Zero clippy warnings (`-D warnings`)
- 74 KiB compressed package

## Links

- **crates.io**: https://crates.io/crates/subtitler
- **Docs**: https://docs.rs/subtitler
- **GitHub**: https://github.com/subtitle-rs/subtitler
- **Migration guide** (from 0.1.x): in the repo

## What's next

- SIMD acceleration for timestamp parsing (targeting `std::simd` when it stabilizes)
- Enhanced LRC (word-level timestamps, metadata parsing)
- More normalize rules (full-width/half-width, punctuation spacing)
- Translation API integration helpers

Feedback and contributions welcome!

---

*Apache-2.0 · Rust 2024 edition · `cargo add subtitler`*
