# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/).

## [1.4.0] - 2026-07-17

### Added

#### New Format Support
- **SCC format (.scc)**: Scenarist Closed Caption format
  - CEA-608 standard for broadcast closed captions
  - SMPTE timecode format (HH:MM:SS;FF or HH:MM:SS:FF)
  - Drop-frame and non-drop-frame timecode support
  - Default 29.97 fps (NTSC)
  - Hex-encoded caption data decoding
  - Streaming parser (SccStream)
  - Widely used in US broadcast television

- **EBU STL format (.stl)**: European Broadcasting Union Standard Transmission Format
  - Professional broadcast-grade binary format
  - GSI (General Subtitle Information) block: 1024-byte file header
  - TTI (Text and Timing Information) blocks: 128-byte subtitle entries
  - SMPTE timecode format (HH:MM:SS:FF)
  - Multi-language subtitle support
  - Rich metadata (program title, language, creator info, etc.)
  - Binary parsing with precise timing
  - Standard in European broadcast industry

#### Format Support Summary
- Format support expanded from 11 to 13 formats
- Total test count: 216 tests (124 unit + 92 integration)
- Support matrix:
  - Web: SRT, VTT, TTML, SAMI
  - Video editing: ASS, SSA
  - DVD: MicroDVD, SubViewer
  - Broadcast: SCC, EBU STL
  - YouTube: SBV
  - Karaoke: LRC
  - Eastern Europe: MPL2

## [1.3.0] - 2026-07-17

### Added

#### New Format Support
- **SAMI format (.smi)**: Microsoft-developed subtitle format
  - HTML-like syntax with `<Sync>` and `<P>` tags
  - Multi-language subtitle support
  - CSS styling extraction
  - Streaming parser (SamiStream)
  - Widely used in Asian markets (Korea, China)

- **MPL2 format (.mpl)**: Frame-based subtitle format
  - Frame-accurate timing with configurable fps
  - Default 23.976 fps support
  - Frame ↔ millisecond conversion utilities
  - Streaming parser (Mpl2Stream)
  - Popular in Eastern Europe

#### Examples
- `parse-sami-content.rs`: SAMI parsing demonstration
- `parse-mpl2-content.rs`: MPL2 parsing with frame conversion
- `create-sami-file.rs`: SAMI generation with multi-language support
- `create-mpl2-content.rs`: MPL2 generation with custom fps
- `example.smi`: Sample SAMI subtitle file
- `example.mpl`: Sample MPL2 subtitle file

### Changed
- Format support expanded from 9 to 11 formats
- Examples expanded from 19 to 23

## [1.2.0] - 2026-07-17

Performance optimizations and streaming write support.

### Added

#### Streaming Write Support
- **write_stream() methods**: Async streaming write for all 9 formats
  - SRT/VTT/ASS/SBV/LRC/MicroDVD/SubViewer: `write_stream<W: AsyncWrite>`
  - TTML: `write_stream<W: std::io::Write>` (sync, quick-xml limitation)
  - Memory-efficient processing of large subtitle files
  - No full-string allocation needed

### Changed

#### Memory Optimizations
- **TextPart bitflags optimization**: Replace 3 bool fields with single bitflags
  - Memory reduction: 3 bytes + padding → 1 byte
  - New `TextFormat` bitflags with BOLD/ITALIC/UNDERLINE flags
  - Add accessor methods: `bold()`, `italic()`, `underline()`
  - Add setter methods: `set_bold()`, `set_italic()`, `set_underline()`
  - Maintain backward compatibility through method-based access

### Dependencies
- Add `bitflags` 2.x with serde feature

## [1.1.0] - 2026-07-17

Performance optimizations, API improvements, and enhanced flexibility.

### Added

#### API Enhancements
- **SubtitleFileBuilder**: Fluent API for constructing SubtitleFile with chainable methods
  - Support for all 9 formats
  - Auto-defaults for optional fields
  - Type-safe construction
- **ParseConfig**: Customizable parsing behavior with 5 configuration options
  - `preserve_indices`: Keep original cue numbers
  - `lenient_mode`: Tolerate format errors
  - `auto_detect_encoding`: Auto-detect text encoding
  - `max_duration_ms`: Maximum subtitle duration
  - `min_duration_ms`: Minimum subtitle duration
- **StreamingParser trait**: Unified interface for incremental parsing
  - `collect_all()`: Parse all remaining subtitles
  - `count_remaining()`: Count without collecting
  - Implemented for all 6 streaming parsers

### Changed

#### Performance Optimizations
- **SmallVec optimization**: Replaced `Vec<TextPart>` with `SmallVec<[TextPart; 4]>`
  - Reduces heap allocations by ~80%
  - Improves cache locality
  - Expected ~10% performance improvement

### Fixed
- Fixed ASS/SSA comment line parsing (regex pattern recognition)
- Fixed TTML timestamp format (using standard '.' separator instead of ',')
- Fixed SBV streaming parser compilation error

## [1.0.0] - 2026-07-16

This release marks the first stable version with unified architecture, complete format support, and production-ready quality.

### Added

#### Core Features
- **9 subtitle formats**: SRT, WebVTT, ASS/SSA, MicroDVD, SubViewer, TTML/IMSC (`quick-xml` based), SBV (YouTube), LRC (Lyrics).
- `SubtitleFormat` trait consolidating editing methods (`shift_all`, `validate`, `validate_extended`, `merge_adjacent`, `split_long`, `sort`, `map`, `filter`, `remove_overlaps`, `enforce_min_duration`, `enforce_max_duration`, `auto_extend_for_cps`, `extract_range`, `transform_framerate`) with default implementations.
- Unified parse entry points: `subtitler::parse_bytes`, `parse_file`, `parse_url` with auto-format-detection.
- `ParseError` typed error (`UnknownFormat` / `Unsupported` / `Decode` / `Io` / `Http`).
- Per-format Cargo feature flags (`srt`, `vtt`, `ass`, `ssa`, `microdvd`, `subviewer`, `ttml`, `sbv`, `lrc`) for compile-size trimming.
- Encoding detection via `chardetng`.

#### Streaming Parsers (Iter 9)
- **Streaming iterators for large-file incremental parsing**:
  - `SrtStream` — SRT streaming parser
  - `VttStream` — WebVTT streaming parser
  - `SbVStream` — YouTube SBV streaming parser
  - `LrcStream` — LRC lyrics streaming parser
  - `MicroDvdStream` — MicroDVD streaming parser
  - `SubViewerStream` — SubViewer streaming parser
- `srt::parse_stream` for incremental SRT parsing without allocating a `Vec`.

#### Quality & Normalization
- `quality` module: `QualityReport` generator (JSON-serializable), `Translator` trait for machine translation, `DummyTranslator`.
- `normalize::optimize_line_breaks` — smart line splitting at natural boundaries for readability.
- Text normalization: whitespace, quotes, punctuation, OCR error fixing, hearing-impaired tag stripping.
- `normalize_subtitle`, `strip_tags`, `plaintext` helpers.

#### CLI
- Full-featured CLI with subcommands: `parse`, `convert`, `validate`, `edit`, `info`, `detect`, `quality`, `normalize`, `shift`.
- 19 example programs covering all major use cases.

#### Builder Methods
- `Subtitle::with_index`, `with_style`, `with_settings`, `with_layer` builder methods.
- `parse_url_with(url, &client)` for custom `reqwest::Client` configuration.

### Changed

#### Breaking Changes
- **[BREAKING]** `SubtitleFormat` enum renamed to `Format`.
- **[BREAKING]** `SubtitleFile` enum expanded to 9 variants. `MicroDvd` and `SubViewer` are now first-class variants (previously collapsed into `Srt`, losing fps/header). `Ass`/`Ssa` wrap `AssData`. Added `Ttml`, `Sbv`, `Lrc`.
- **[BREAKING]** `srt::parse_content`, `parse_bytes` and the `vtt::` equivalents are now sync (not `async`). `parse_file`/`parse_url` remain async.
- **[BREAKING]** Subtitle struct: removed `layer`, `margin_l`, `margin_r`, `margin_v`, `effect` fields (ASS-only, Iter 8). These fields are no longer tracked per-subtitle; ASS output uses 0 defaults.
- **[BREAKING]** `reqwest` now uses `default-features = false, features = ["rustls"]`.
- **[BREAKING]** `tokio` features trimmed: `["fs", "io-util", "rt", "macros"]`; all `#[tokio::main]` use `current_thread` flavor.

#### Improvements
- MicroDVD round-trip now preserves fps (emits `{1}{1}fps` header when non-default).
- SubViewer round-trip preserves the `[INFORMATION]` header block.
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
- `encoding_rs`-based true decoding for GBK, Shift_JIS, Big5 (was failing).

### Fixed

#### Format-Specific Fixes
- ASS `is_comment`: was reading wrong capture group (15→14).
- SubViewer: `00:00:01.500` incorrectly parsed as 5000ms (now validated).
- LRC: 5-second default duration instead of zero.
- SBV `detect_format`: tightened to prevent false positives on SRT content.
- VTT: header preserved through unified entry point.
- MicroDVD FPS round-trip: emit fps header when non-default.
- `microdvd::parse_bytes`: replaced `unreachable!()` with proper `Err`.
- `sbv::detect_format`: tightened to require `H:MM:SS.mmm` time format.
- `vtt::parse_bytes` / unified `parse_bytes_as`: preserves the WEBVTT header.
- VTT NOTE blocks: correctly skipped, subtitles after them no longer lost.
- VTT voice tags: speaker name now extracted (was hardcoded to `"v"`).
- TTML: namespace-prefixed tags (`tt:p`), `<br/>` line breaks, `dur` attribute, `tts:fontStyle`/`tts:fontWeight` span styling now parsed.
- LRC parser: 5s default display duration instead of zero.

#### General Fixes
- `RE_TIMESTAMP` regex bounded (`\d{1,}`→`\d{1,4}`) to prevent ReDoS.
- `main.rs`: tracing subscriber no longer panics on double-init.
- `optimize_line_breaks` line ordering (LIFO→FIFO queue).
- `detect_format` across all modules: now decodes GBK/Shift_JIS/Big5 (was limited to UTF-8).

### Performance

#### Streaming & Memory Efficiency
- **Streaming parsers** for SRT/VTT/SBV/LRC/MicroDVD/SubViewer — no full-file allocation needed.
- `SrtStream` — incremental SRT parsing without allocating a `Vec`.
- Quick skip in `extract_text_parts` when no tags present.

#### Optimization Techniques
- Byte-scanning timestamp parser (replaces regex on hot path).
- `LazyLock`-cached regexes (zero compile-time overhead).
- Manual byte-scanning timestamp parser (replaces regex on the hot path).

### Removed

- Implicit degradation of MicroDVD/SubViewer into the `Srt` variant.
- Example `utility-ops` (superseded by `edit-operations` + `validate-subtitle`).
- Subtitle fields: `layer`, `margin_l`, `margin_r`, `margin_v`, `effect` (ASS-only, Iter 8).

### See also

- `MIGRATION.md` for a 0.1 → 1.0 upgrade guide.

## [Unreleased] — v2.0.0

### Breaking Changes

- All format modules (`srt`, `vtt`, `ass`, `ttml`, `sbv`, `lrc`, `sami`,
  `microdvd`, `subviewer`, `mpl2`, `scc`, `ebu_stl`) now consistently return
  `SubtitleFile` from `parse_content` / `parse_bytes` / `parse_file` / `parse_url`.
  Previously some modules returned `Vec<Subtitle>`.
- `utils::parse_timestamp` and `utils::parse_timestamps` now require a `Format`
  parameter for format-specific error messages.
- `encoding::decode_to_string` returns `Result<_, SubtitleError>` instead of
  `anyhow::Result`.
- `LrcData::to_string`, `SamiData::to_string`, `Mpl2Data::to_string`,
  `SccData::to_string` renamed to `render()` (avoid shadowing `std::ToString`).
- `SCC::to_string` now accepts `drop_frame: bool` parameter.

### Added

- Structured `SubtitleError` enum (11 variants) replacing `anyhow!()` macros
  in all format internals. Format-aware error messages with `format` context.
- `SubtitleFile` now derives `Deserialize` / `Serialize` for all variants.
- `LrcData` strong type with `LrcLine` structs preserving multi-timestamp fidelity.
- EBU STL `detect_format` strengthened: validates TTI block count matches
  header metadata in addition to size/structure checks.
- **Streaming parsers**: SRT `parse_stream` and VTT `parse_stream` yield
  subtitles one at a time without allocating a full `Vec`. VttStream upgraded
  from raw `u8` phases to proper enum with header tracking.
- **`SubtitleBuilder`**: chainable builder API wrapping `SubtitleFile`.
  Methods: `sort()`, `shift()`, `merge_adjacent()`, `split_long()`,
  `transform_fps()`, `remove_overlaps()`, `enforce_min/max_duration()`,
  `auto_extend_cps()`, `map()`, `filter()`.
- **`Pipeline` DSL** (`subtitler::pipeline`): declarative transformation
  pipeline with JSON serialization support. `Pipeline::new().sort().shift(500)`
  `.apply(file)`; or deserialize from JSON config.
- **CLI `pipeline` command**: `subtitler pipeline input.srt output.vtt --config ops.json`
  supports 10 operation types via JSON config files.
- Throughput benchmarks: 10k-subtitle SRT/VTT/ASS parse + round-trip.

### Changed

- `model.rs` split into `model/` sub-modules: `format.rs`, `trait.rs`,
  `subtitle.rs`, `types.rs`, `convert.rs`, `builder.rs`, `streaming.rs`,
  `validation.rs`, `mod.rs`.
- `main.rs` format dispatch simplified: all arms delegate directly to
  `format::parse_content`, removing duplicate `SubtitleFile` construction.
- `split_text_chunks` optimized: avoids O(n²) intermediate `format!()`
  allocations by pre-allocating `String::with_capacity` and byte-counting.
- `cmd_edit` refactored to use `SubtitleBuilder` internally (was direct
  `SubtitleFormat` method calls).

### Performance

- **Zero-copy parsing**: SRT/VTT parsers work directly on `&str` slices;
  removed per-line `.to_string()` calls in both main and streaming paths.
- All 12 format modules pre-allocate `Vec::with_capacity` based on content
  size estimates; EBU STL uses exact TTI count from header.
- VTT `header_lines` changed from `Vec<String>` to `Vec<&str>` with deferred
  `.join("\n")`.

### Fixed

- SCC `to_string` no longer hardcodes `drop_frame: true`; inherited from
  parsed input for round-trip correctness.
- Removed EBU STL `tti_timecode_to_ms` no-op function (timecode values are
  already in milliseconds from `parse_smpte_timecode`).

## [1.4.0] - 2026-07-15

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
