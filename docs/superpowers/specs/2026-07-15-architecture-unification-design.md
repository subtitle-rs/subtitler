# Architecture Unification: enum + trait + feature flags

**Date:** 2026-07-15
**Status:** Approved (pending implementation)
**Scope:** One cycle. Unify the `SubtitleFile` architecture as the foundation for
future format/performance/quality work.
**Version target:** `1.0.0` (breaking, from `0.1.0`)
**Predecessor spec:** `2026-07-15-cleanup-batch-design.md` (completed)

## Context

A competitive scan of the Rust subtitle ecosystem (aspasia, subparse, ass-core,
rsubs-lib) shows subtitler leads on quality tooling (CPS/overlap/normalize), CLI,
and encoding detection. The gaps are: (1) format coverage (no TTML/SBV/LRC),
(2) inconsistent architecture (`SubtitleFile` enum has 3 variants but
`SubtitleFormat` has 6 — MicroDVD/SubViewer silently degrade into the `Srt`
variant, losing fps/header), and (3) performance headroom (no SIMD/streaming).

This spec addresses the architecture gap only. Unification is the prerequisite
for all three subsequent directions — adding formats, SIMD, and deeper quality
tools each get easier once the format surface is uniform. Per the user's
roadmap decision, this is cycle 1 of a multi-cycle effort toward "strongest in
the Rust ecosystem."

## Goals

1. `SubtitleFile` enum expanded to one variant per format, preserving
   format-specific data (MicroDVD fps, SubViewer header).
2. A `SubtitleFormat` trait that consolidates the ~25 methods currently on
   `impl SubtitleFile`, with default implementations so adding a format means
   implementing 2 methods, not editing 25.
3. Per-format Cargo feature flags so users can trim compile size.
4. A unified parse entry point (`parse_bytes` / `parse_file` / `parse_url`)
   that auto-detects format and routes.
5. Honest sync/async split: parsing cores become sync (they always were —
   `.await` on `Cursor<&str>` is synchronous); only I/O entry points stay async.

## Non-goals (deferred to later cycles)

- New subtitle formats (TTML/SBV/LRC/JSON) — cycle 2.
- SIMD acceleration / streaming parser — performance cycle.
- Translation API, time-axis correction, more normalize rules — quality cycle.
- No change to existing parsing *logic* for already-supported formats. This
  cycle refactors the shell (types, dispatch, features), not the parsers'
  internals. A `#[cfg]`/trait refactor must not alter what a given format
  parses to.

---

## Part 1 — Core type refactor

### 1.1 `SubtitleFile` enum: 3 → 6 variants

Current (information-losing):

```rust
pub enum SubtitleFile {
  Srt(Vec<Subtitle>),
  Vtt { header: Option<String>, subtitles: Vec<Subtitle> },
  Ass { info: HashMap<String, String>, styles: Vec<AssStyle>, subtitles: Vec<Subtitle> },
  // MicroDVD and SubViewer both collapse into Srt / lose their data
}
```

Target:

```rust
pub enum SubtitleFile {
  Srt(Vec<Subtitle>),
  Vtt { header: Option<String>, subtitles: Vec<Subtitle> },
  Ass(AssData),                                       // v4+ (Advanced SubStation)
  Ssa(AssData),                                       // v4 (SubStation Alpha) — shares AssData
  MicroDvd { fps: f64, subtitles: Vec<Subtitle> },    // NEW: preserves fps
  SubViewer { header: Option<String>, subtitles: Vec<Subtitle> }, // NEW: preserves header
}

// Shared by Ass and Ssa variants — identical structure, different format() tag
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AssData {
  #[serde(skip_serializing_if = "HashMap::is_empty", default)]
  pub info: HashMap<String, String>,
  #[serde(skip_serializing_if = "Vec::is_empty", default)]
  pub styles: Vec<AssStyle>,
  pub subtitles: Vec<Subtitle>,
}
```

**Behavior fixes (observable):**
- **MicroDVD fps round-trip.** Today `microdvd::parse_content(content, fps)`
  computes times using a `saved_fps` that may be updated from a `{1}{1}30.000`
  header line, but the returned `SubtitleFile::Srt` discards it. On
  `to_string`, fps falls back to the 23.976 default, corrupting frame numbers
  on re-serialization. The new `MicroDvd { fps, .. }` variant stores it.
- **SubViewer header round-trip.** Today `subviewer::to_string` hardcodes a
  default `[INFORMATION]\n[TITLE]Subtitles...` block and parse discards the
  original. The new `SubViewer { header, .. }` variant preserves it.
- **SSA vs ASS identity.** Today both parse as `Ass`; `format()` cannot
  distinguish them. The `Ssa(AssData)` variant restores the distinction
  (matters for players/tools that treat SSA v4 differently from ASS v4+).

These three are the *reason* the enum expansion is worth the breaking change —
they are current silent data-loss bugs.

### 1.2 Rename `SubtitleFormat` enum → `Format`

The name `SubtitleFormat` is needed for the trait (§1.3). The existing enum
becomes `Format`:

```rust
pub enum Format {
  Srt, Vtt, Ass, Ssa, MicroDvd, SubViewer,
}
```

This is a pure rename; all match arms and conversions follow. Flagged as a
breaking change.

### 1.3 `SubtitleFormat` trait

Consolidates the methods currently on `impl SubtitleFile`. Required methods
(per-variant) are minimal; editing methods get default implementations via
`subtitles()`/`subtitles_mut()`.

```rust
pub trait SubtitleFormat: Debug + Clone + Send + Sync {
  /// Required: read access to the subtitle list.
  fn subtitles(&self) -> &[Subtitle];
  /// Required: write access to the subtitle list.
  fn subtitles_mut(&mut self) -> &mut Vec<Subtitle>;
  /// Required: which format this file is.
  fn format(&self) -> Format;
  /// Required: serialize to a target format (for cross-format conversion).
  fn to_string_with_format(&self, fmt: &Format) -> String;

  // --- Default implementations: operate only via subtitles()/subtitles_mut() ---
  fn to_string(&self) -> String { self.to_string_with_format(&self.format()) }
  fn shift_all(&mut self, offset_ms: i64) { /* iterates subtitles_mut */ }
  fn map<F: FnMut(&mut Subtitle)>(mut self, f: F) -> Self { ... }
  fn filter<F: FnMut(&Subtitle) -> bool>(mut self, f: F) -> Self { ... }
  fn sort(&mut self) { ... }
  fn validate(&self) -> Vec<ValidationIssue> { ... }
  fn validate_extended(&self, max_chars: usize, max_gap_ms: u64, max_cps: f64) -> Vec<ValidationIssue> { ... }
  fn merge_adjacent(&mut self, max_gap_ms: u64) { ... }
  fn remove_overlaps(&mut self) { ... }
  fn enforce_min_duration(&mut self, min_ms: u64) { ... }
  fn enforce_max_duration(&mut self, max_ms: u64) { ... }
  fn auto_extend_for_cps(&mut self, max_cps: f64) { ... }
  fn extract_range(&self, start_ms: u64, end_ms: u64) -> Vec<Subtitle> { ... }
  fn concatenate(&mut self, other: &dyn SubtitleFormat, gap_ms: u64) { ... }
  fn split_long(&mut self, max_chars: usize) { ... }
  fn transform_framerate(&mut self, in_fps: f64, out_fps: f64) { ... }
}
```

`impl SubtitleFormat for SubtitleFile` — the enum itself implements the trait,
dispatching to per-variant logic in the 4 required methods and inheriting all
default methods. This preserves the existing call site shape
(`file.validate()`, `file.shift_all(...)`) so most user code is unaffected.

**Why this design:** the default methods are *generic over the subtitle list*
and don't care about format-specific fields. The only format-aware operations
are output (`to_string_with_format`) and identity (`format()`). Adding a new
format later means implementing 4 required methods — the 15 editing methods
come for free.

---

## Part 2 — Feature flags + unified parse entry point

### 2.1 Per-format Cargo features

```toml
[features]
default = ["srt", "vtt", "ass", "ssa", "microdvd", "subviewer", "http"]

srt       = []
vtt       = []
ass       = []
ssa       = []
microdvd  = []
subviewer = []
http      = ["reqwest"]
```

Each format module is `#[cfg(feature = "...")]`-gated. The corresponding
`SubtitleFile` variant and `Format` enum variant are gated too — disabling a
format removes it at compile time, so callers can't construct or parse a format
that isn't compiled in. The 4 required trait methods' match arms are `#[cfg]`-
branched to only include enabled formats.

`detect_format` only probes enabled formats. The unified entry point returns
`ParseError::Unsupported(fmt)` if a file is detected as a format whose feature
is off.

**Backward compat:** `default` enables everything, so `cargo add subtitler`
behaves as before. Size-conscious users opt out: `default-features = false,
features = ["srt", "vtt"]`.

### 2.2 Unified parse entry points (high-level API)

```rust
// src/lib.rs
pub fn parse_bytes(data: &[u8]) -> Result<SubtitleFile, ParseError> {
  let fmt = detect_format(data).ok_or(ParseError::UnknownFormat)?;
  parse_bytes_as(data, fmt)
}

pub fn parse_bytes_as(data: &[u8], fmt: Format) -> Result<SubtitleFile, ParseError> {
  match fmt {
    #[cfg(feature = "srt")]       Format::Srt       => Ok(SubtitleFile::Srt(srt::parse_bytes(data)?)),
    #[cfg(feature = "vtt")]       Format::Vtt       => { let (h, s) = vtt::parse_bytes(data)?; Ok(SubtitleFile::Vtt { header: h, subtitles: s }) },
    #[cfg(feature = "ass")]       Format::Ass       => Ok(SubtitleFile::Ass(ass::parse_bytes(data)?)),
    #[cfg(feature = "microdvd")]  Format::MicroDvd  => { let (fps, s) = microdvd::parse_bytes(data)?; Ok(SubtitleFile::MicroDvd { fps, subtitles: s }) },
    #[cfg(feature = "subviewer")] Format::SubViewer => { let (h, s) = subviewer::parse_bytes(data)?; Ok(SubtitleFile::SubViewer { header: h, subtitles: s }) },
    #[cfg(feature = "ssa")]       Format::Ssa       => Ok(SubtitleFile::Ssa(ass::parse_bytes(data)?)),
    _ => Err(ParseError::Unsupported(fmt)),
  }
}

pub async fn parse_file(path: impl AsRef<Path>) -> Result<SubtitleFile, ParseError> { ... }
#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> Result<SubtitleFile, ParseError> { ... }
```

Per-format modules keep their specific entry points (`srt::parse_content`,
etc.) for advanced users who know the format upfront — these now return the
rich per-variant types.

### 2.3 Sync parsing core, async I/O only

**Problem:** SRT/VTT parsers are `async fn`, but their `.await` points are
`lines.next_line().await` on a `Cursor<&str>` — synchronous, no real I/O
suspension. ASS/MicroDVD/SubViewer are already sync. This inconsistency blocks
a unified entry point.

**Fix:** Parsing cores become sync. Only file/URL I/O entry points stay async.

| Function | Before | After |
|---|---|---|
| `srt::parse_content(&str)` | `async` | sync |
| `srt::parse_bytes(&[u8])` | `async` | sync |
| `srt::parse_file(path)` | async (real I/O) | async (unchanged) |
| `srt::parse_url(url)` | async (real I/O) | async (unchanged) |
| `vtt::parse_*` | same pattern | same fix |
| `ass::parse_content/bytes` | sync | sync (unchanged) |
| `ass::parse_file/url` | async | async (unchanged) |

The implementation change: the inner `parse<R: AsyncBufReadExt>` functions
become `fn parse(content: &str)` operating on `content.lines()` directly. The
`parse_file`/`parse_url` wrappers do `read_to_string`/`resp.text().await`
then call the sync core. This is faster (no async state machine for pure CPU
work) and honest.

**Breaking** — callers drop `.await` on `parse_content`/`parse_bytes`. Documented
in migration guide.

### 2.4 `ParseError`

```rust
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
  #[error("could not detect subtitle format")]
  UnknownFormat,
  #[error("format {0:?} is not enabled (enable its cargo feature)")]
  Unsupported(Format),
  #[error("decode/parse error: {0}")]
  Decode(#[from] SubtitleError),
  #[error("I/O error: {0}")]
  Io(#[from] std::io::Error),
}
```

`SubtitleError` (added in the cleanup batch) is reused for the decode/parse
variant. The legacy `AnyResult<T>` alias is kept for back-compat; new code
should use `Result<_, ParseError>`.

---

## Part 3 — Migration, staging, versioning

### 3.1 Three-stage implementation (each independently testable)

The refactor is one coherent design but ships as three commit groups so a
failure can be localized to types / trait / features.

| Stage | Scope | Verification gate |
|---|---|---|
| **S1: Types** | enum 3→6 variants; `AssData` struct; `SubtitleFormat`→`Format` rename; MicroDvd/SubViewer data preservation in their modules | existing tests green + new "MicroDvd round-trips fps" + "SubViewer round-trips header" tests |
| **S2: Trait** | define `SubtitleFormat` trait; move 15 editing methods to default impls; `impl SubtitleFormat for SubtitleFile` dispatches | editing methods (merge/split/shift/validate) work on all 6 variants — new parametrized tests |
| **S3: Features + entry points + sync** | per-format features; unified `parse_bytes/file/url`; parsing cores sync; `ParseError` | `--no-default-features --features srt,vtt` compiles + works; unified-entry tests |

### 3.2 Breaking API change inventory (1.0 declaration)

| Change | Before | After | Migration |
|---|---|---|---|
| Enum rename | `SubtitleFormat` (enum) | `Format` | global rename |
| New trait | — | `SubtitleFormat` trait | — |
| Enum variants | 3 | 6 | MicroDvd no longer degrades to Srt |
| Parse signatures | `srt::parse_content(s).await` | `srt::parse_content(s)` | drop `.await` |
| Unified entry | none | `subtitler::parse_bytes/file/url` | new, recommended |
| Feature model | `http` only | + 6 format flags | default unchanged |
| MicroDvd storage | `SubtitleFile::Srt` | `SubtitleFile::MicroDvd { fps, .. }` | pattern-match update |
| SubViewer storage | `Vec<Subtitle>` (no header) | `SubtitleFile::SubViewer { header, .. }` | pattern-match update |

**Compatibility layer (reduces pain, not permanent):**
- `pub type AnyResult<T> = Result<T, anyhow::Error>` kept.
- Per-format `parse_content`/`parse_bytes`/`generate` retained (signatures sync).
- Users migrate at their own pace to unified entry + `ParseError`.
- No blanket `#[deprecated]` spam in 1.0 — the migration guide is the primary
  signal; deprecation attributes can come in 1.1 if needed.

### 3.3 Version + docs

- Bump `0.1.0` → `1.0.0`.
- Create `MIGRATION.md`: 0.1 → 1.0, one section per breaking change with
  before/after code snippets.
- `CHANGELOG.md`: 1.0.0 entry, Added/Changed/Removed, `[BREAKING]` markers.
- README: redraw architecture diagram (two-tier — high-level `SubtitleFile` +
  per-format low-level modules), document feature flags and unified entry.
- The 1.0.0 release is the "stable API" commitment point per the user's
  decision to go straight to 1.0.

---

## Verification

Each stage's gate is its own test group; final gate (per `AGENTS.md`):

```
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --no-default-features --features srt,vtt   # NEW: feature-matrix check
cargo build --no-default-features --features ass,ssa
```

New tests required by this spec:
- MicroDVD round-trip preserves `fps` (regression for current data loss).
- SubViewer round-trip preserves header.
- `format()` returns the correct `Format` for all 6 variants (esp. Ssa vs Ass).
- Editing methods (`merge_adjacent`, `split_long`, `shift_all`, `validate`)
  work identically across all variants — parametrized via a test matrix.
- `parse_bytes`/`parse_file` auto-detect and return the right variant.
- `ParseError::Unsupported` when a detected format's feature is disabled.
- Feature-matrix: each `--features` subset builds and its formats parse.

## Risk notes

- **Trait object safety:** if any future method needs `Self`-sized return or
  generic methods, default impls may need adjustment. The current 15 editing
  methods are all `&self`/`&mut self` returning owned/`Vec` data — object-safe.
  `concatenate` takes `&dyn SubtitleFormat` to stay object-safe.
- **Serde:** enum variant additions are backward-compatible for deserialization
  (old Srt-only files still parse). `AssData` struct is `#[derive(Serialize,
  Deserialize)]`.
- **`#[cfg]` on enum variants + match arms** requires careful feature gating to
  avoid "unreachable pattern" / "missing variant" errors. Plan stage will use a
  consistent `#[cfg(feature = "x")]` discipline and macro helper if the
  repetition becomes unwieldy.
