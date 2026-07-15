# Cleanup Batch: Performance, Correctness, Architecture

**Date:** 2026-07-15
**Status:** Approved (pending implementation)
**Scope:** One cycle. Performance + correctness + conservative architecture cleanup.
**Version target:** `0.1.0` (minor bump from `0.0.5`)

## Context

A full scan of the `subtitler` crate surfaced issues in three areas: performance
hotspots (regex recompilation, redundant scans), correctness bugs (overlap
detection, source-format identity loss, CPS measurement on tagged text), and
architecture debt (unused `thiserror`, oversized tokio features, asymmetric ASS
entry points). This spec addresses all three with a conservative hand.

Out of scope (deferred to future cycles): new formats (SBV/TTML/LRC), streaming
parser, `SubtitleFile` enum expansion (Approach B), async/sync parser internals
unification (Approach C).

## Non-goals

- No change to the `SubtitleFile` enum shape (`Srt | Vtt | Ass` stays).
- No rewrite of ASS parser internals to async.
- No new subtitle formats.
- No public-API breakage beyond behavior fixes that motivate the `0.1.0` bump.

## Part 1 — Performance

### 1.1 Lift per-call regex compilation to `LazyLock`

Seven call sites compile regexes on every call. Each is replaced by a
module-level `static RE_X: LazyLock<Regex>` (the crate already uses this pattern
in `utils.rs`, `srt.rs`, `normalize.rs`).

| Location | File:line | Current | Fix |
|---|---|---|---|
| `Subtitle::strip_tags` | `model.rs:84,86` | 2× `Regex::new` | 2× `LazyLock` |
| `Subtitle::plaintext` | `model.rs:93,95` | 2× `Regex::new` | 2× `LazyLock` |
| `vtt::extract_text_parts` | `vtt.rs:30-34` | `Regex::new` per call | `LazyLock` |
| `ass::parse_ass_tags` | `ass.rs:258` | `Regex::new` | `LazyLock` |
| `ass::ass_to_plaintext` | `ass.rs:331` | `Regex::new` | `LazyLock` |
| `normalize::fix_ocr_errors` | `normalize.rs:76` | 5× `Regex::new` in loop | 5× `LazyLock` (array) |
| `srt::detect_format` | `srt.rs:244` | `Regex::new` | `LazyLock` |

**Behavior:** unchanged. **Verification:** existing round-trip tests + new
micro-benchmark group (§4.2).

### 1.2 Remove redundant double-scan in SRT/VTT parsers

`srt::parse` (`srt.rs:192-204`) and `vtt::parse` (`vtt.rs:207-223`) run
`extract_text_parts` on the trailing subtitle, then **re-run it on every
subtitle** in a second loop. The second loop is redundant: each subtitle's
`text`/`text_parts` is already final after the trailing extract (SRT) or the
trailing extract already covers the last subtitle (VTT).

The current code path: a subtitle gets its `text` accumulated during parsing,
then the trailing handler calls `extract_text_parts` and overwrites `text` and
`text_parts`. The post-loop re-applies the same transformation to all entries,
which for the trailing subtitle is a repeat and for others is the *first* time
they're processed. So only the trailing subtitle is double-processed today.

**Fix:** unify extraction so it runs exactly once per subtitle. Move the
extraction call into the "subtitle finalized" path (the `current_subtitle.take()`
branch on blank line), and apply it to the trailing subtitle at end-of-input.
Remove the post-loop. Net effect: identical output, ~halved extraction work for
the common case where most subtitles end on a blank line.

**Behavior:** identical (parts/`plain` are pure functions of text).
**Verification:** SRT/VTT round-trip tests + tag-parsing tests.

### 1.3 Cache regex in `detect_format`

`srt::detect_format` (`srt.rs:244`) compiles a regex each call. Lift to
`LazyLock` (already counted in §1.1's table).

## Part 2 — Correctness

### 2.1 Fix `validate()` overlap false-negatives

`SubtitleFile::validate` (`model.rs:418-431`) assumes subtitles are sorted by
start time: the inner loop `break`s on the first non-overlapping pair. On
unsorted input (common after `filter`, `concatenate`, or manual edits) it
produces false negatives.

**Fix:** `validate` does not mutate input and should not. Build an index-sorted
view (`Vec<(orig_idx, &Subtitle)>` sorted by `(start, end)`), then scan adjacent
pairs in that view. Report each adjacent overlap with the **original** indices so
users see the subtitles' positions in their file.

```text
// pseudocode
let mut order: Vec<usize> = (0..subs.len()).collect();
order.sort_by_key(|&i| (subs[i].start, subs[i].end));
for w in order.windows(2) {
    let (a, b) = (w[0], w[1]);  // original indices
    if subs[b].start < subs[a].end {
        issues.push(ValidationIssue::Overlap {
            index_a: a, index_b: b,
            end_a: subs[a].end, start_b: subs[b].start,
        });
    }
}
```

This is O(n log n) rather than the prior O(n²), and detects overlaps regardless
of input order. It reports *adjacent* overlaps in time order — same intent as the
original, just correct.

`DecreasingStartTime` keeps its current semantics: it reports disorder in the
**input file** as authored (untouched by the internal sort).

**Verification:** new tests for (a) unsorted input with one overlap, (b) sorted
clean input (unchanged), (c) sorted input with one overlap (unchanged).

### 2.2 SubViewer/MicroDvd storage — known limitation, accepted (no change)

SubViewer parses to `Vec<Subtitle>` and MicroDvd parses directly to
`SubtitleFile::Srt(...)` (`subviewer.rs:49`, `microdvd.rs:67`). Under Approach A
we keep the enum unchanged, so `SubtitleFile::format()` returns `Srt` for these
files. **This is a library-API inaccuracy only**, not a user-visible bug:

- `cmd_info` (`main.rs:297`) and `cmd_parse` (`main.rs:114`) display the format
  from `resolve_format(&data, ...)` — content detection — *not* `file.format()`.
  Verified: display is correct for SubViewer/MicroDvd inputs.
- `cmd_convert` routes output through `to_string_with_format(&target_fmt)` where
  `target_fmt` comes from the output extension, so SubViewer→VTT etc. work.

**No code change this cycle.** Documented as a limitation; full symmetry is
deferred to the enum-expansion cycle (Approach B).

### 2.3 `chars_per_second` measures plaintext

`Subtitle::chars_per_second` (`model.rs:64-71`) counts `self.text.chars()` —
**including tags**. A subtitle `<b>Hi</b>` over 1 s reads as 9 cps, not 2 cps.
All callers over-count: `validate_extended`'s `CpsTooHigh`, `cmd_info`'s
`max_cps`, and `auto_extend_for_cps`.

**Fix:** count `self.plaintext().chars()`. Update the existing
`test_chars_per_second` (`model.rs:825`) expectation and add a tagged-input
case.

This is a behavior change → part of the `0.1.0` motivation.

**Verification:** unit test with tagged input; confirm `validate_extended` and
`auto_extend_for_cps` now use effective text length.

### 2.4 Positional indices in output

`srt::to_string` (`srt.rs:261-264`) and `vtt::to_string` emit `subtitle.index`
when present. After `merge_adjacent`, `split_long`, or `filter`, stored indices
no longer correspond to output position. Output then reads `[1], [3], [99]`,
which breaks players and tooling.

**Fix:** in both `to_string` implementations, ignore stored `index` and emit the
positional 1-based ordinal (`i + 1`). Round-trip tests continue to pass (input
indices were already sequential in fixtures). Add a test with deliberately
non-sequential stored indices asserting sequential output.

**Behavior change** → part of the `0.1.0` motivation.

## Part 3 — Architecture cleanup

### 3.1 Typed errors (`thiserror`)

The crate declares `thiserror` but never uses it. Add `src/error.rs`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum SubtitleError {
  #[error("invalid timestamp format: {0}")]
  InvalidTimestamp(String),
  #[error("expected {expected} at row {row}, got: {got}")]
  UnexpectedLine { row: usize, expected: &'static str, got: String },
  #[error("invalid UTF-8: {0}")]
  InvalidUtf8(#[from] std::string::FromUtf8Error),
  #[error("I/O error: {0}")]
  Io(#[from] std::io::Error),
}
```

Conservative integration: existing `AnyResult<T>` (= `Result<T, anyhow::Error>`)
public signatures stay unchanged. New `error.rs` types are available for new
code and for gradual migration. `SubtitleError` derives `Into<anyhow::Error>`
via `thiserror`, so any future function returning `Result<_, SubtitleError>`
interops with `AnyResult`. Wire `pub mod error;` into `lib.rs`.

**Non-goal:** rewriting every existing signature to `SubtitleError` — that's a
larger API change and out of scope.

**Verification:** unit test constructing each variant and asserting `Display`
output.

### 3.2 Trim tokio features

`Cargo.toml:38` uses `features = ["full"]`. Used APIs: `tokio::fs::{File,
OpenOptions}`, `AsyncBufReadExt`, `AsyncWriteExt`, `BufReader`, `#[tokio::test]`,
`#[tokio::main]` (multi-thread default in `main.rs`). This needs `["fs",
"io-util", "rt-multi-thread", "macros"]` — not `full`.

**Change:**
```toml
tokio = { version = "^1.52.3", features = ["fs", "io-util", "rt-multi-thread", "macros"] }
```

**Verification:** `cargo build`, `cargo build --no-default-features`, full test
suite.

### 3.3 ASS symmetric entry points

ASS only has `parse_content(&str)`. SRT/VTT also expose `parse_bytes`,
`parse_file` (async), `parse_url` (http-gated). Add to `ass.rs`:

- `pub async fn parse_file(path) -> AnyResult<SubtitleFile>` via
  `tokio::fs::read_to_string` + `parse_content`.
- `pub fn parse_bytes(&[u8]) -> AnyResult<SubtitleFile>` — UTF-8 decode then
  `parse_content`.
- `#[cfg(feature = "http")] pub async fn parse_url(url) -> AnyResult<SubtitleFile>`
  via `reqwest` + `parse_content`.

Internal sync `parse_content` stays sync (Approach C rejected). `main.rs:96-98`
can drop its hardcoded `ass::parse_content` special-case once `parse_bytes` /
`parse_file` exist and `parse_to_file` calls them uniformly.

**Verification:** tests mirroring SRT's `parse_bytes`/round-trip structure.

### 3.4 `types.rs`

`src/types.rs` (58 bytes) holds only `AnyResult`. Kept as-is. Renaming/merging is
churn with no benefit.

## Part 4 — Tests, CI, versioning, docs

### 4.1 Tests per change

| Change | Test |
|---|---|
| `validate()` overlap fix | unsorted input w/ overlap detected; sorted cases unchanged |
| `chars_per_second()` plaintext | tagged input → effective cps |
| positional output indices | stored `[99, 1]` → output `1`, `2` |
| ASS `parse_bytes`/`parse_file`/`parse_url` | mirror SRT tests |
| tokio trim | `cargo build --no-default-features` + default build green |
| typed errors | per-variant `Display` assertions |

The double-scan removal (Part 1) is covered by the existing
`tests/integration.rs` suite — must remain green.

### 4.2 Benchmark

Add a benchmark group to `benches/subtitler_benchmark.rs` isolating
`strip_tags`, `plaintext`, `fix_ocr_errors` on tag-heavy input. Target: ≥2×
speedup vs. baseline on that group after Part 1.

### 4.3 Version + changelog

- Bump `Cargo.toml` `version` to `0.1.0`.
- Create `CHANGELOG.md` (none exists today) with `[Unreleased]` and `[0.1.0]`
  sections listing every change above, grouped Added / Changed / Fixed /
  Performance.
- Update any `0.0.x` references in `README.md` to `0.1.0`.

### 4.4 Docs

- Document new `ass::parse_file` / `parse_bytes` / `parse_url` in README format
  table (parity with SRT/VTT).
- Note the `chars_per_second()` semantics change (plaintext, not raw) as a
  Changed entry.
- Note positional output indices as a Changed entry.

### 4.5 Verification gate (per AGENTS.md)

Before every commit:

```
cargo fmt -- --check       # 2-space indent per rustfmt.toml
cargo clippy -- -D warnings
cargo test --all-targets
cargo build --no-default-features
```

## Behavior changes (motivate `0.1.0`)

1. `validate()` overlap detection works on unsorted input (was: false negatives).
2. `chars_per_second()` counts plaintext, not raw tagged text (was: over-count).
3. SRT/VTT `to_string` emits positional 1-based indices (was: echoed stored
   indices, which could be stale/non-sequential after edits).

All three were bugs; the bump signals the corrected semantics.

## Deferred (future cycles)

- New formats: SBV, TTML, LRC.
- Streaming parser.
- `SubtitleFile` enum expansion (Approach B) — symmetric MicroDvd/SubViewer
  variants.
- Async/sync parser-internal unification (Approach C).
- Full migration of existing signatures from `anyhow` to `SubtitleError`.
