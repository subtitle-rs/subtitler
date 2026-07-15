# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [1.0.0] - 2026-07-15

### Added
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
  `subviewer`) for compile-size trimming.
- `AssData` shared struct for ASS/SSA, and `SubtitleFile::Ssa` variant.
- `microdvd::parse_bytes`, `subviewer::parse_bytes` per-format entry points.

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
