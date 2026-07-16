# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/).

## [0.10.0] - 2026-07-16

### Added
- `parse_url_with(url, &client)` for custom `reqwest::Client` configuration.
- `Subtitle::with_index`, `with_style`, `with_settings`, `with_layer` builder methods.
- `encoding_rs`-based true decoding for GBK, Shift_JIS, Big5 (was failing).
- `optimize_line_breaks` text wrapping utility.
- `SubtitleFile` variant doc comments (all 9 formats documented).
- `srt::parse_stream` for incremental SRT parsing.
- 10 new examples (stream-parse, quality-report, normalize-text, etc.).
- CLI commands: `quality`, `normalize`, `shift`.
- `parse_url_with(url, &client)` for custom HTTP client configuration.
- Subtitle builder methods: `with_index`, `with_style`, `with_settings`, `with_layer`.
- `encoding_rs`-based decoding for GBK/Shift_JIS/Big5 (was failing on non-UTF-8).

### Changed
- **[BREAKING]** `reqwest` now uses `default-features = false, features = ["rustls"]`.
- **[BREAKING]** `tokio` features trimmed: `["fs", "io-util", "rt", "macros"]`; all `#[tokio::main]` use `current_thread` flavor.
- `parse_url` generates its own client (was `reqwest::get`).
- `microdvd::parse_bytes` returns `SubtitleFile` (was `(f64, Vec<Subtitle>)`).
- SRT `parse_stream` returns `Err` on malformed timestamps (was silently skipping).
- SubViewer centiseconds: rejects >2-digit fractional parts.
- `optimize_line_breaks` rewritten from recursion to loop (stack-safe).
- `extract_text_parts` skips regex when no `<` character present.
- VTT NOTE blocks: correctly exit to `Cue` state after the block.
- VTT voice speaker name: extracts actual name (`<v Alice>` → `"Alice"`).
- TTML: `<br/>`, `dur`, `tts:fontStyle`/`fontWeight`, namespace-agnostic parsing.
- `chars_per_second` counts plaintext (was counting tags).

### Fixed
- ASS `is_comment`: was reading wrong capture group (15→14).
- SubViewer: `00:00:01.500` incorrectly parsed as 5000ms (now validated).
- LRC: 5-second default duration instead of zero.
- SBV `detect_format`: tightened to prevent false positives on SRT content.
- VTT: header preserved through unified entry point.
- MicroDVD FPS round-trip: emit fps header when non-default.
- `RE_TIMESTAMP` regex bounded (`\d{1,}`→`\d{1,4}`) to prevent ReDoS.
- `main.rs`: tracing subscriber no longer panics on double-init.
- `optimize_line_breaks` line ordering (LIFO→FIFO queue).
- `detect_format` across all modules: now decodes GBK/Shift_JIS/Big5 (was limited to UTF-8).

### Performance
- Byte-scanning timestamp parser (replaces regex on hot path).
- `LazyLock`-cached regexes (zero compile-time overhead).
- SRT/VTT/SBV/LRC/MicroDVD/SubViewer streaming parsers (`*Stream` types).
- Quick skip in `extract_text_parts` when no tags present.

### Changed
- **[BREAKING]** Subtitle struct: removed `layer`, `margin_l`, `margin_r`, `margin_v`, `effect` fields (ASS-only). These fields are no longer tracked per-subtitle; ASS output uses 0 defaults.

### Added
- Streaming iterators: `VttStream`, `SbVStream`, `LrcStream`, `MicroDvdStream`, `SubViewerStream`.

### Removed
- Example `utility-ops` (superseded by `edit-operations` + `validate-subtitle`).

## [1.0.0] - 2026-07-16

### Added
- **9 subtitle formats**: SRT, WebVTT, ASS/SSA, MicroDVD, SubViewer,
  TTML/IMSC (`quick-xml` based), SBV (YouTube), LRC (Lyrics).
- `SubtitleFormat` trait consolidating editing methods (`shift_all`, `validate`,
  `validate_extended`, `merge_adjacent`, `split_long`, `sort`, `map`, `filter`,
  `remove_overlaps`, `enforce_min_duration`, `enforce_max_duration`,
  `auto_extend_for_cps`, `extract_range`, `transform_framerate`) with default
  implementations, so adding a format means implementing 4 required methods.
- Unified parse entry points: `subtitler::parse_bytes`, `parse_file`,
  `parse_url` with auto-format-detection.
- `ParseError` typed error (`UnknownFormat` / `Unsupported` / `Decode` / `Io` /
  `Http`).
- Per-format Cargo feature flags (`srt`, `vtt`, `ass`, `ssa`, `microdvd`,
  `subviewer`, `ttml`, `sbv`, `lrc`) for compile-size trimming.
- `AssData` shared struct for ASS/SSA.
- `quality` module: `QualityReport` generator (JSON-serializable),
  `Translator` trait for machine translation, `DummyTranslator`.
- `normalize::optimize_line_breaks` — smart line splitting at natural
  boundaries for readability.
- `srt::parse_stream` / `SrtStream` — streaming iterator for incremental SRT
  parsing without allocating a `Vec`.
- Manual byte-scanning timestamp parser (replaces regex on the hot path).
- Text normalization: whitespace, quotes, punctuation, OCR error fixing,
  hearing-impaired tag stripping.
- `normalize_subtitle`, `strip_tags`, `plaintext` helpers.
- CLI: `parse`, `convert`, `validate`, `edit`, `info`, `detect` subcommands.
- Encoding detection via `chardetng`.

### Changed
- **[BREAKING]** `SubtitleFormat` enum renamed to `Format`.
- **[BREAKING]** `SubtitleFile` enum expanded to 9 variants. `MicroDvd` and
  `SubViewer` are now first-class variants (previously collapsed into `Srt`,
  losing fps/header). `Ass`/`Ssa` wrap `AssData`. Added `Ttml`, `Sbv`, `Lrc`.
- **[BREAKING]** `srt::parse_content`, `parse_bytes` and the `vtt::` equivalents
  are now sync (not `async`). `parse_file`/`parse_url` remain async.
- MicroDVD round-trip now preserves fps; SubViewer round-trip preserves the
  `[INFORMATION]` header block.

### Fixed
- `microdvd::parse_bytes`: replaced `unreachable!()` with proper `Err`.
- `sbv::detect_format`: tightened to require `H:MM:SS.mmm` time format.
- `vtt::parse_bytes` / unified `parse_bytes_as`: preserves the WEBVTT header.
- VTT NOTE blocks: correctly skipped, subtitles after them no longer lost.
- VTT voice tags: speaker name now extracted (was hardcoded to `"v"`).
- TTML: namespace-prefixed tags (`tt:p`), `<br/>` line breaks, `dur` attribute,
  `tts:fontStyle`/`tts:fontWeight` span styling now parsed.
- LRC parser: 5s default display duration instead of zero.

### Removed
- Implicit degradation of MicroDVD/SubViewer into the `Srt` variant.

### Changed
- **[BREAKING]** `SubtitleFormat` enum renamed to `Format`.
- **[BREAKING]** `SubtitleFile` enum expanded to 6 variants. `MicroDvd` and
  `SubViewer` are now first-class variants (previously collapsed into `Srt`,
  losing fps/header). `Ass` now wraps `AssData`.
- **[BREAKING]** `srt::parse_content`, `parse_bytes` and the `vtt::` equivalents
  are now sync (not `async`). `parse_file`/`parse_url` remain async.
- MicroDVD round-trip now preserves fps (emits a `{1}{1}fps` header when the
  stored fps differs from the 23.976 default); SubViewer round-trip now
  preserves the `[INFORMATION]` header block.

### Removed
- Implicit degradation of MicroDVD/SubViewer into the `Srt` variant.

### See also
- `MIGRATION.md` for a 0.1 → 1.0 upgrade guide.

## [Unreleased]

## [0.1.0] - 2026-07-15

### Performance

- Lifted per-call regex compilation to `LazyLock` in `strip_tags`,
  `plaintext`, VTT `extract_text_parts`, ASS `parse_ass_tags` /
  `ass_to_plaintext`, `fix_ocr_errors`, and `srt::detect_format`.
- Removed the redundant second pass of `extract_text_parts` over every
  subtitle in the SRT and VTT parsers; parts are now extracted once at
  finalization.
- Trimmed `tokio` features from `full` to `["fs", "io-util",
  "rt-multi-thread", "macros"]`.

### Fixed

- `SubtitleFile::validate()` no longer misses overlaps on unsorted input.
  The overlap scan now sorts an index view by `(start, end)` and compares
  adjacent pairs, reporting original indices. The previous early-`break`
  produced false negatives when subtitles were out of order (and a
  one-directional check produced false positives on some non-overlapping
  pairs).
- `Subtitle::chars_per_second()` now counts `plaintext()` characters
  (excluding markup) instead of raw `text`. Fixes over-counting for tagged
  subtitles; affects `validate_extended`, `auto_extend_for_cps`, and CLI
  `info` output.
- SRT and VTT `to_string` now emit 1-based positional cue indices instead of
  echoing stored (potentially stale) indices. Fixes non-sequential cue
  numbers after `merge_adjacent`, `split_long`, or `filter`.
- Updated `chardetng` calls to the 1.0 API (`EncodingDetector::new` /
  `guess`) and bumped the dependency to `^1.0.0` so the crate compiles
  against the locked dependency.

### Added

- `error` module with a typed `SubtitleError` enum (opt-in; existing
  `AnyResult` signatures unchanged).
- `ass::parse_bytes`, `ass::parse_file` (async), and `ass::parse_url`
  (http-gated) entry points, bringing ASS to parity with SRT/VTT.
- `regex_hotspots` criterion benchmark group for regression tracking.

### Changed

- **Breaking (within 0.x):** `validate()` overlap detection,
  `chars_per_second` semantics, and SRT/VTT output indices are corrected as
  described under Fixed. Consumers relying on the prior (buggy) behavior
  should review.
