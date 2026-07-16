---
name: subtitler
description: Parse, convert, validate, edit, and generate subtitles using the subtitler CLI. Supports 9 formats (SRT, VTT, ASS/SSA, MicroDVD, SubViewer, TTML/IMSC, SBV, LRC).
---

# subtitler CLI

Process subtitles from the command line — parse, convert, validate, edit, detect, and generate quality reports across 9 subtitle formats.

## Basic Commands

### Parse & Display

```bash
# Display subtitles from a file (auto-detect format)
subtitler parse movie.srt

# Force a specific format
subtitler parse movie.sub --format microdvd

# JSON output
subtitler parse movie.srt --json

# From stdin
cat movie.srt | subtitler parse -
```

### Convert Formats

```bash
# Auto-detect source, infer target from extension
subtitler convert input.srt output.vtt
subtitler convert input.srt output.ttml
subtitler convert input.srt output.ass

# Explicit format spec
subtitler convert input.srt output.vtt --from srt --to vtt
subtitler convert input.srt output.ass --to ass
```

### Validate Quality

```bash
# Basic timing checks (overlaps, negative durations)
subtitler validate movie.srt

# Extended: CPS, text length, gaps
subtitler validate movie.srt --max-cps 25 --max-chars 42 --max-gap 5000

# JSON report
subtitler validate movie.srt --json
```

### Edit Operations

```bash
# Sort by start time
subtitler edit input.srt --sort --output sorted.srt

# Shift all timestamps (+2000ms = delay 2s)
subtitler edit input.srt --shift 2000 --output delayed.vtt

# Merge adjacent subtitles (gap ≤ 500ms)
subtitler edit input.srt --merge 500 --output merged.srt

# Split long subtitles (max 42 chars per line)
subtitler edit input.srt --split 42 --output split.srt

# Transform framerate
subtitler edit input.srt --transform-fps 23.976 25.0 --output 25fps.srt

# Combine operations
subtitler edit input.srt --sort --shift -500 --output shifted.srt
```

### Quick Time Shift

```bash
# Shift by fixed offset (positive = delay, negative = advance)
subtitler shift input.srt 2000 --output delayed.srt
subtitler shift input.srt -500 --output advanced.srt
```

### Normalize Text

```bash
# Apply all normalizations
subtitler normalize input.srt --all --output clean.srt

# Individual normalizations
subtitler normalize input.srt --fix-ocr --output fixed.srt
subtitler normalize input.srt --strip-hi --output no-hi.srt
subtitler normalize input.srt --quotes --output cleanquotes.srt
subtitler normalize input.srt --whitespace --output trimmed.srt
```

### Quality Report

```bash
# Human-readable report
subtitler quality movie.srt

# JSON report (for programmatic use)
subtitler quality movie.srt --json
```

### Information & Detection

```bash
# Show subtitle statistics
subtitler info movie.srt

# Detect format only
subtitler detect unknown.sub
```

## Global Options

| Flag | Description |
|------|-------------|
| `-h`, `--help` | Print help |
| `-V`, `--version` | Print version |

## Supported Formats

| Format | Extension | Detect | CLI Name |
|--------|-----------|--------|----------|
| SubRip | `.srt` | ✅ | `srt` |
| WebVTT | `.vtt` | ✅ | `vtt` |
| ASS | `.ass` | ✅ | `ass` |
| SSA | `.ssa` | ✅ | `ssa` |
| MicroDVD | `.sub` | ✅ | `microdvd` |
| SubViewer | (none) | ✅ | `subviewer` |
| TTML/IMSC | `.ttml`, `.xml` | ✅ | `ttml` |
| YouTube SBV | `.sbv` | ✅ | `sbv` |
| LRC Lyrics | `.lrc` | ✅ | `lrc` |

## stdin/stdout Support

All commands that read input accept `-` for stdin.
All commands that write output accept `-` for stdout.

```bash
# Pipe: stdin → parse → stdout (STDERR for info)
cat movie.srt | subtitler parse - --json 2>/dev/null

# Pipe with detect
cat movie.sub | subtitler detect -
```

## Pipe: Normalize → Convert

```bash
subtitler normalize input.srt --all --output - | subtitler convert - output.vtt
```

## Environment

- `SUB_RUST_LOG` — set tracing level (default: WARN)
