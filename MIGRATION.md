# Migration Guide

## 2.3 → 2.4

### Additions (non-breaking)

- **Two new formats**: DFXP (`.dfxp`) and Whisper JSON (feature flags
  `dfxp` and `whisper`, enabled by default).
- **`RemoveDuplicates` PipelineOp**: new op for deduplication.
- **4 normalize functions**: `filter_language`, `merge_short_lines`,
  `remove_all_newlines`, `replace_newlines`.

### Format count

- v2.3: 13 formats. v2.4: **15 formats** (+DFXP, +Whisper).

## 2.2 → 2.3

### No breaking changes

2.3 is a test/CI hardening release. No public API changes. All existing
2.2 code compiles and behaves identically.

### Developer-facing changes

- `criterion` and `proptest` are now gated to non-wasm32 targets; if your
  downstream project depends on these dev-dependencies for wasm32, add them
  explicitly.
- Integration test files now carry `#![cfg(not(target_arch = "wasm32"))]`;
  custom test runners for wasm32 must set up their own entrypoints.

## 2.1 → 2.2

### Additions (non-breaking)

- 11 more formats now expose `generate(subs, path, policy)`. Existing
  SRT/VTT users see no change.
- 7 more formats now expose `parse_stream(content)`.
- SAMI/MPL2/SCC now have `write_stream`.

### Deprecation

- `ttml::write_stream` (sync) is deprecated. Migrate to
  `ttml::write_stream_async`:
  ```rust
  // Before
  ttml::write_stream(&subs, &mut file)?;
  // After
  ttml::write_stream_async(&subs, &mut file).await?;
  ```
  The sync version remains until 3.0.

### Behavior change (TTML)

- `ttml::to_string(subs, header)` no longer ignores `header`. If you
  were passing `Some("...")` expecting it to be dropped, the output
  will now contain a `<head>` block.

## 2.0 → 2.1

### Behavior changes

- **SCC drop-frame timecodes**: previously SCC used the non-drop formula for
  all timecodes, causing NTSC long-form video (`;` separator) to drift ~3.6
  seconds per hour. 2.1 correctly implements SMPTE 12M-1-2014 drop-frame.
  If you relied on the (incorrect) numerical values, re-parse your SCC files.
  Key invariant: `01:00:00;00` (drop) now equals exactly 3600000ms (was
  ~3603604ms under the buggy non-drop handling).

- **EBU STL serialization**: previously `ebu_stl::to_string` wrote corrupted
  TTI timecodes (`ms / 40` instead of milliseconds). 2.1 produces correct
  files that round-trip through `parse_bytes`. Old corrupted files are not
  parseable by 2.1, but were unusable anyway.

- **UTF-16 decoding**: UTF-16-encoded subtitle files now correctly strip the
  U+FEFF BOM (matching UTF-8 path behavior). Downstream code that depended
  on a leading BOM character must be adjusted.

- **`split_long` duration**: when a subtitle's duration is shorter than the
  number of chunks it splits into, the effective end is now stretched so
  every chunk has at least 1ms duration. Subtitles that previously had
  `start == end` (zero-duration, failing `validate()`) are now valid.

### Additions

- `encoding::decode_to_string` now skips the 2-byte BOM before UTF-16 decoding
  and returns an empty string (not a panic/error) for inputs under 2 bytes.
- `scc_timecode_to_ms` now takes a `drop_frame: bool` 6th argument.

---

# Migrating from 1.x to 2.0.0

2.0.0 completes the v2 API unification — all `parse_*` functions now return
`SubtitleFile`, errors are structured, and the internals have been modularized.

## All `parse_content` / `parse_bytes` / `parse_file` return `SubtitleFile`

Every format module (SRT, VTT, ASS, TTML, SBV, LRC, SAMI, MicroDVD,
SubViewer, MPL2, SCC, EBU STL) now consistently returns `SubtitleFile`
from all public parse entry points. Previously some modules returned
`Vec<Subtitle>` (e.g. VTT, MPL2 parse_bytes).

```rust
// Before (1.x)
let subs: Vec<Subtitle> = subtitler::vtt::parse_content(&text)?;
let subs: Vec<Subtitle> = subtitler::mpl2::parse_bytes(data)?;

// After (2.0)
let file: SubtitleFile = subtitler::vtt::parse_content(&text)?;
let file: SubtitleFile = subtitler::mpl2::parse_bytes(data)?;
// Access subtitles via trait:
let subs: &[Subtitle] = file.subtitles();
```

## Accessing subtitles from `SubtitleFile`

`SubtitleFile` is a parsed file, not a `Vec`. Use the `SubtitleFormat`
trait to access shared methods:

```rust
use subtitler::SubtitleFormat; // re-exported from model

let file = subtitler::srt::parse_content(&content)?;
let count = file.subtitles().len();        // not file.len()
let first = &file.subtitles()[0];           // not &file[0]
file.validate();                           // trait method
file.sort();                               // trait method
```

## `parse_timestamp` / `parse_timestamps` now require `Format`

```rust
// Before
subtitler::utils::parse_timestamp("00:00:01,000")?;

// After
subtitler::utils::parse_timestamp("00:00:01,000", Format::Srt)?;
subtitler::utils::parse_timestamps("... --> ...", Format::Vtt)?;
```

## Structured errors: `SubtitleError` replaces `anyhow` in internals

Format modules now use structured `SubtitleError` variants instead of
`anyhow!()` macros. Public API still returns `AnyResult` via `?` coercion.

New error variants give you format-aware context:

```rust
match subtitler::srt::parse_content(&text) {
  Ok(file) => { /* ... */ }
  Err(e) => {
    // e is anyhow::Error, but the source is a SubtitleError:
    if let Some(se) = e.downcast_ref::<subtitler::error::SubtitleError>() {
      match se {
        SubtitleError::InvalidTimestamp { format, value } => { /* ... */ }
        SubtitleError::UnexpectedLine { format, row, expected, got } => { /* ... */ }
        _ => {}
      }
    }
  }
}
```

## `encoding::decode_to_string` returns `Result<_, SubtitleError>`

```rust
// Before
let text: anyhow::Result<String> = subtitler::encoding::decode_to_string(data);

// After
let text: Result<String, subtitler::error::SubtitleError> =
  subtitler::encoding::decode_to_string(data);
```

## `SCC::to_string` now accepts `drop_frame` parameter

```rust
// Before
let scc = subtitler::scc::to_string(&subs);

// After
let scc = subtitler::scc::to_string(&subs, true);  // drop-frame
let scc = subtitler::scc::to_string(&subs, false); // non-drop-frame
```

## Data type `to_string()` renamed to `render()`

`LrcData::to_string`, `SamiData::to_string`, `Mpl2Data::to_string`,
`SccData::to_string` renamed to `render()` to avoid shadowing
`std::string::ToString`.

```rust
// Before
data.to_string();

// After
data.render();
```

## Available feature flags

```toml
subtitler = { version = "2.0", default-features = false,
  features = ["srt", "vtt", "ass", "ssa", "microdvd", "subviewer",
              "ttml", "sbv", "lrc", "sami", "mpl2", "scc", "ebu_stl", "http"] }
```

---

# Migrating from 0.1.x to 1.0.0

1.0.0 unifies the subtitle architecture. Here's how to update.

## `SubtitleFormat` enum renamed to `Format`

```rust
// Before
use subtitler::model::SubtitleFormat;
let f: SubtitleFormat = file.format();

// After
use subtitler::model::Format;
let f: Format = file.format();
```

(`SubtitleFormat` is now the name of a **trait** — see below.)

## New `SubtitleFormat` trait

Methods like `validate()`, `shift_all()`, `merge_adjacent()`, `sort()`,
`map()`, `filter()` etc. moved from inherent methods on `SubtitleFile` to the
`SubtitleFormat` trait (with default implementations). Bring the trait into
scope at call sites:

```rust
use subtitler::model::SubtitleFormat; // or: use subtitler::SubtitleFormat;
file.validate();   // works
file.shift_all(1000);
```

## New `SubtitleFile` variants: `MicroDvd`, `SubViewer`, `Ssa`

MicroDVD and SubViewer no longer collapse into `Srt`. If you pattern-match on
`SubtitleFile`, add arms (and note they now preserve data that was previously
silently lost):

```rust
// Before: MicroDVD parsed as SubtitleFile::Srt(...), fps discarded.
// After:
match file {
  subtitler::model::SubtitleFile::MicroDvd { fps, subtitles } => { /* fps preserved */ }
  subtitler::model::SubtitleFile::SubViewer { header, subtitles } => { /* header preserved */ }
  subtitler::model::SubtitleFile::Ssa(data) => { /* shares AssData shape with Ass */ }
  _ => {}
}
```

## Parsing cores are now sync

`srt::parse_content`, `srt::parse_bytes`, and the `vtt::` equivalents are no
longer `async`:

```rust
// Before
let subs = subtitler::srt::parse_content(&text).await?;

// After
let subs = subtitler::srt::parse_content(&text)?;
```

`parse_file` and `parse_url` remain `async`.

## New unified entry points (recommended)

```rust
let file = subtitler::parse_bytes(&data)?;
let file = subtitler::parse_file("path.sub").await?;
#[cfg(feature = "http")]
let file = subtitler::parse_url("https://example.com/sub.vtt").await?;
```

## Removed Subtitle fields

The `Subtitle` struct no longer has `layer`, `margin_l`, `margin_r`, `margin_v`, or `effect` fields.

## Per-format feature flags

```toml
subtitler = { version = "1.0", default-features = false, features = ["srt", "vtt"] }
```

Available flags: `srt`, `vtt`, `ass`, `ssa`, `microdvd`, `subviewer`, `ttml`, `sbv`, `lrc`, `sami`, `mpl2`, `scc`, `ebu_stl`, `http`.
