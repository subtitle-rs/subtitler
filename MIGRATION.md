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

The `Ass` variant also changed shape: it now wraps `AssData` (the shared
ASS/SSA struct) instead of inline fields:

```rust
// Before
SubtitleFile::Ass { info, styles, subtitles }
// After
SubtitleFile::Ass(AssData { info, styles, subtitles })
```

## Parsing cores are now sync

`srt::parse_content`, `srt::parse_bytes`, and the `vtt::` equivalents are no
longer `async` (they never did real async I/O — the `.await` was on a
synchronous `Cursor`):

```rust
// Before
let subs = subtitler::srt::parse_content(&text).await?;

// After
let subs = subtitler::srt::parse_content(&text)?;
```

`parse_file` and `parse_url` remain `async` (real I/O).

## New unified entry points (recommended)

```rust
// Auto-detect and parse any format from bytes / file / URL
let file = subtitler::parse_bytes(&data)?;
let file = subtitler::parse_file("path.sub").await?;
#[cfg(feature = "http")]
let file = subtitler::parse_url("https://example.com/sub.vtt").await?;
```

These return `Result<SubtitleFile, subtitler::error::ParseError>`.

## Removed Subtitle fields

The `Subtitle` struct no longer has `layer`, `margin_l`, `margin_r`, `margin_v`, or `effect`
fields. These were ASS/SSA-only fields that every subtitle carried as `Option`, wasting
~80 bytes per subtitle for SRT/VTT files.

If you accessed these fields directly, remove the accesses — they always returned `None`
for non-ASS formats, and the ASS `to_string` output now defaults to `0` for margins
and empty string for effect.

```rust
// Before
let layer = sub.layer.unwrap_or(0);
let margin = sub.margin_l.unwrap_or(10);

// After — use defaults directly
let layer = 0;
let margin = 10;
```

Builder method `Subtitle::with_layer` has been removed. If you need ASS-specific fields,
construct them at the ASS output level.

## Per-format feature flags

If you use `default-features = false`, enable the formats you need:

```toml
[dependencies]
subtitler = { version = "1.0", default-features = false, features = ["srt", "vtt"] }
```

Available flags: `srt`, `vtt`, `ass`, `ssa`, `microdvd`, `subviewer`, `http`.
