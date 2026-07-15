# Architecture Unification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify `SubtitleFile` to one variant per format, introduce a `SubtitleFormat` trait that collapses editing methods, add per-format Cargo feature flags and a unified parse entry point, and split parsing cores (sync) from I/O (async) — the foundation for 1.0.

**Architecture:** Three staged commits. Stage 1 expands the enum to 6 variants and fixes MicroDVD/SubViewer silent data loss. Stage 2 extracts a `SubtitleFormat` trait with default method impls. Stage 3 adds feature flags, the unified `parse_bytes/file/url` entry point, and sync-ifies parsing cores. Every stage keeps the existing test suite green and adds its own verification tests.

**Tech Stack:** Rust 2024 edition, thiserror, serde, clap, chardetng. 2-space indent per `rustfmt.toml`.

**Spec:** `docs/superpowers/specs/2026-07-15-architecture-unification-design.md`

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `src/model.rs` | Modify | `SubtitleFile` enum (6 variants), `AssData` struct, `Format` enum (renamed), `SubtitleFormat` trait |
| `src/srt.rs` | Modify | parse core sync, keep `parse_file/url` async, `#[cfg(feature="srt")]` |
| `src/vtt.rs` | Modify | same as srt |
| `src/ass.rs` | Modify | return `AssData`, `parse_content/bytes` sync, `#[cfg(feature="ass")]` |
| `src/microdvd.rs` | Modify | return `(fps, Vec<Subtitle>)`, preserve fps, sync, `#[cfg]` |
| `src/subviewer.rs` | Modify | return `(Option<String>, Vec<Subtitle>)`, preserve header, sync, `#[cfg]` |
| `src/lib.rs` | Modify | wire `pub trait`, unified `parse_bytes/file/url`, `detect_format` gating |
| `src/error.rs` | Modify | add `ParseError` enum |
| `src/main.rs` | Modify | update routing for new variants, remove `.await` on sync calls |
| `tests/arch_unification.rs` | Create | stage verification tests |
| `Cargo.toml` | Modify | feature flags, bump to 1.0.0 |
| `MIGRATION.md` | Create | 0.1 → 1.0 guide |
| `CHANGELOG.md` | Modify | 1.0.0 entry |
| `README.md` | Modify | architecture diagram, feature flags, unified entry |

---

# STAGE 1 — Type expansion (enum 3 → 6 variants)

## Task 1: Add `AssData` struct and `Ssa` variant scaffolding

Introduce the shared `AssData` struct and the `Ssa` variant first (additive, low risk) before touching existing variants. The existing `Ass` variant keeps its inline struct shape for now; we'll migrate it to `AssData` in Task 2.

**Files:** Modify `src/model.rs`

- [ ] **Step 1: Add `AssData` struct**

In `src/model.rs`, immediately before `pub enum SubtitleFile {` (search for `pub enum SubtitleFile`), insert:

```rust
/// Shared ASS/SSA structure. Used by both the `Ass` (v4+) and `Ssa` (v4)
/// variants of `SubtitleFile`, which differ only in their `format()` tag.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AssData {
  #[serde(skip_serializing_if = "std::collections::HashMap::is_empty", default)]
  pub info: std::collections::HashMap<String, String>,
  #[serde(skip_serializing_if = "Vec::is_empty", default)]
  pub styles: Vec<AssStyle>,
  pub subtitles: Vec<Subtitle>,
}
```

- [ ] **Step 2: Add `Ssa(AssData)` variant to the enum**

Find the existing `pub enum SubtitleFile` (currently 3 variants). Replace the whole enum with:

```rust
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum SubtitleFile {
  Srt(Vec<Subtitle>),
  Vtt {
    #[serde(skip_serializing_if = "Option::is_none")]
    header: Option<String>,
    subtitles: Vec<Subtitle>,
  },
  Ass {
    #[serde(skip_serializing_if = "std::collections::HashMap::is_empty", default)]
    info: std::collections::HashMap<String, String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    styles: Vec<AssStyle>,
    subtitles: Vec<Subtitle>,
  },
  Ssa(AssData),
}
```

(We add only `Ssa` here. `MicroDvd`/`SubViewer` variants come in Task 4/5. The `Ass` variant keeps its inline fields for now — migrated in Task 3.)

- [ ] **Step 3: Add `Ssa` arms to all existing `match self` sites**

There are 4 match sites in `impl SubtitleFile`. For each, add an `Ssa` arm. The methods `subtitles()`, `subtitles_mut()`, `format()` currently match `SubtitleFile`. Update each:

`subtitles()` — find the match block and add after the `Ass` arm:
```rust
      SubtitleFile::Ssa(data) => &data.subtitles,
```

`subtitles_mut()` — add:
```rust
      SubtitleFile::Ssa(data) => &mut data.subtitles,
```

`format()` — this returns `SubtitleFormat`. Add:
```rust
      SubtitleFile::Ssa(_) => SubtitleFormat::Ssa,
```

`to_string_with_format` — the `Ass | Ssa` arm in the inner `match format` already handles both; no change to the format-match. But the outer `match self` for extracting `(info, styles)` only handles `SubtitleFile::Ass`. Find:
```rust
        let (info, styles) = match self {
          SubtitleFile::Ass { info, styles, .. } => (info.clone(), styles.clone()),
          _ => (
            std::collections::HashMap::new(),
            vec![crate::model::AssStyle::default_style()],
          ),
        };
```
Replace with:
```rust
        let (info, styles) = match self {
          SubtitleFile::Ass { info, styles, .. } => (info.clone(), styles.clone()),
          SubtitleFile::Ssa(data) => (data.info.clone(), data.styles.clone()),
          _ => (
            std::collections::HashMap::new(),
            vec![crate::model::AssStyle::default_style()],
          ),
        };
```

- [ ] **Step 4: Verify build + tests**

Run: `cargo test 2>&1 | grep -E "test result:|error" | tail -6`
Expected: all tests pass; no errors. (The new variant is constructible but no test uses it yet — that's fine for this scaffold task.)

- [ ] **Step 5: Commit**

```bash
git add src/model.rs
git commit -m "refactor: add AssData struct and Ssa variant (Stage 1 scaffold)"
```

---

## Task 2: Rename `SubtitleFormat` enum → `Format`

The name `SubtitleFormat` is needed for the trait (Stage 2). Rename the enum now.

**Files:** Modify `src/model.rs`, `src/main.rs`, `src/lib.rs` — wherever `SubtitleFormat` (the enum) appears.

- [ ] **Step 1: Rename the enum definition**

In `src/model.rs`, find:
```rust
pub enum SubtitleFormat {
  Srt,
  Vtt,
  Ass,
  Ssa,
  MicroDvd,
  SubViewer,
}
```
Rename `SubtitleFormat` → `Format`:
```rust
pub enum Format {
  Srt,
  Vtt,
  Ass,
  Ssa,
  MicroDvd,
  SubViewer,
}
```

- [ ] **Step 2: Update all references in model.rs**

The `format()` method return type and `to_string_with_format(&self, format: &SubtitleFormat)` use the old name. In `src/model.rs`:

Find `pub fn format(&self) -> SubtitleFormat {` → change to `pub fn format(&self) -> Format {`.

Find `pub fn to_string_with_format(&self, format: &SubtitleFormat) -> String {` → change to `pub fn to_string_with_format(&self, format: &Format) -> String {`.

The inner match arms `SubtitleFormat::Srt =>` etc. → change all to `Format::Srt =>`, `Format::Vtt =>`, `Format::Ass`, `Format::Ssa`, `Format::MicroDvd`, `Format::SubViewer`. There are 6 of them in `to_string_with_format`.

- [ ] **Step 3: Update main.rs**

Run: `grep -n "SubtitleFormat" src/main.rs`
Replace every `SubtitleFormat` with `Format`. Specifically:
- `use subtitler::model::{SubtitleFile, SubtitleFormat};` → `use subtitler::model::{SubtitleFile, Format};`
- `fn format_to_subtitle_format(f: &Format) -> SubtitleFormat {` → `fn format_to_subtitle_format(f: &Format) -> Format {`
- All `SubtitleFormat::Srt` etc. in `resolve_format` and `cmd_detect` → `Format::Srt` etc.
- `let target_fmt = format_to_subtitle_format(&to);` stays (function name unchanged).

- [ ] **Step 4: Update lib.rs**

Run: `grep -n "SubtitleFormat" src/lib.rs`
The `detect_format` returns `Option<SubtitleFormat>`. Change to `Option<Format>` and update the return values `Some(SubtitleFormat::Srt)` → `Some(Format::Srt)` etc. across the chained `or_else` calls (they call `srt::detect_format` etc. which return `Option<crate::model::SubtitleFormat>` — those module functions also need updating, see Step 5).

- [ ] **Step 5: Update per-format detect_format return types**

Each format module's `detect_format` returns `Option<crate::model::SubtitleFormat>`. Run: `grep -rn "SubtitleFormat" src/srt.rs src/vtt.rs src/ass.rs src/microdvd.rs src/subviewer.rs`
Change every `crate::model::SubtitleFormat` → `crate::model::Format` and `Some(crate::model::SubtitleFormat::X)` → `Some(crate::model::Format::X)`.

- [ ] **Step 6: Verify build + tests**

Run: `cargo test 2>&1 | grep -E "test result:|error" | tail -6`
Expected: all pass. Run `grep -rn "SubtitleFormat" src/` → should return **zero** matches (the name is fully gone).

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor: rename SubtitleFormat enum to Format"
```

---

## Task 3: Migrate `Ass` variant to use `AssData`

Now collapse the `Ass { info, styles, subtitles }` inline variant into `Ass(AssData)` for consistency with `Ssa`. This is a breaking change to the enum shape but simplifies all match arms.

**Files:** Modify `src/model.rs`, `src/ass.rs`, `src/main.rs`

- [ ] **Step 1: Change the enum variant**

In `src/model.rs`, find the `Ass { ... }` variant in `pub enum SubtitleFile` and replace it (and keep `Ssa`):

```rust
  Ass(AssData),
  Ssa(AssData),
```

(Remove the inline `Ass { info, styles, subtitles }` struct variant entirely.)

- [ ] **Step 2: Update the 4 match sites in model.rs**

`subtitles()` — replace `SubtitleFile::Ass { subtitles: subs, .. } => subs,` with `SubtitleFile::Ass(data) | SubtitleFile::Ssa(data) => &data.subtitles,` (merge the two arms). Remove the separate `Ssa` arm added in Task 1.

`subtitles_mut()` — similarly: `SubtitleFile::Ass(data) | SubtitleFile::Ssa(data) => &mut data.subtitles,`. Remove separate `Ssa` arm.

`format()`:
```rust
      SubtitleFile::Ass(_) => Format::Ass,
      SubtitleFile::Ssa(_) => Format::Ssa,
```

`to_string_with_format` — the `(info, styles)` extraction:
```rust
        let (info, styles) = match self {
          SubtitleFile::Ass(data) | SubtitleFile::Ssa(data) => (data.info.clone(), data.styles.clone()),
          _ => (
            std::collections::HashMap::new(),
            vec![crate::model::AssStyle::default_style()],
          ),
        };
```

- [ ] **Step 3: Update ass.rs to construct/return AssData**

In `src/ass.rs`, `parse_content` returns `SubtitleFile::Ass { info, styles, subtitles }`. Find that return (search `Ok(SubtitleFile::Ass`) and replace with:
```rust
  Ok(SubtitleFile::Ass(AssData {
    info,
    styles,
    subtitles,
  }))
```
Add `AssData` to the `use crate::model::{...}` import at the top of ass.rs: `use crate::model::{AssData, AssStyle, Subtitle, SubtitleFile};`

- [ ] **Step 4: Update main.rs**

Run `grep -n "SubtitleFile::Ass" src/main.rs`. There shouldn't be direct construction in main.rs (it uses `ass::parse_content`), but `parse_to_file` has `Format::Ass | Format::Ssa => ass::parse_content(&text)` which is fine. Verify no `SubtitleFile::Ass {` inline construction remains.

- [ ] **Step 5: Verify build + tests**

Run: `cargo test 2>&1 | grep -E "test result:|error" | tail -6`
Expected: all pass. Run `grep -rn "SubtitleFile::Ass {" src/` → zero matches.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: collapse Ass variant to use AssData"
```

---

## Task 4: Add `MicroDvd` variant preserving fps

Currently `microdvd::parse_content` returns `SubtitleFile::Srt(subs)` — losing the fps. Add a `MicroDvd { fps, subtitles }` variant and preserve fps.

**Files:** Modify `src/model.rs`, `src/microdvd.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/arch_unification.rs`:

```rust
use subtitler::model::{Subtitle, SubtitleFile};

#[test]
fn microdvd_roundtrips_fps() {
  // {1}{1}30.000 declares fps=30; frames {30}{60} at 30fps = 1000-2000ms.
  // After round-trip the fps must be preserved (not fall back to 23.976).
  let content = "{1}{1}30.000\n{30}{60}Hello\n";
  let file = subtitler::microdvd::parse_content(content, None).unwrap();
  // Re-serialize via the format-aware path
  let out = file.to_string();
  assert!(
    out.contains("30.000"),
    "fps header lost in round-trip; got:\n{out}"
  );
}

#[test]
fn microdvd_variant_preserves_fps_field() {
  let content = "{1}{1}30.000\n{30}{60}Hello\n";
  let file = subtitler::microdvd::parse_content(content, None).unwrap();
  match file {
    SubtitleFile::MicroDvd { fps, .. } => assert!((fps - 30.0).abs() < 0.001, "fps={fps}"),
    other => panic!("expected MicroDvd variant, got {other:?}"),
  }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test arch_unification 2>&1 | tail -10`
Expected: FAIL — `parse_content` currently returns `SubtitleFile::Srt(...)`, so `match file { SubtitleFile::MicroDvd {..} => ... }` panics with "expected MicroDvd variant".

- [ ] **Step 3: Add the variant to the enum**

In `src/model.rs`, add to `pub enum SubtitleFile` (after `Ssa(AssData)`):
```rust
  MicroDvd {
    fps: f64,
    subtitles: Vec<Subtitle>,
  },
```

- [ ] **Step 4: Add match arms in model.rs**

`subtitles()`: add
```rust
      SubtitleFile::MicroDvd { subtitles: subs, .. } => subs,
```
`subtitles_mut()`: add
```rust
      SubtitleFile::MicroDvd { subtitles: subs, .. } => subs,
```
`format()`: add
```rust
      SubtitleFile::MicroDvd { .. } => Format::MicroDvd,
```
`to_string_with_format` — the inner `match format` already has a `Format::MicroDvd => crate::microdvd::to_string(subs, None)` arm. Change it to pass the stored fps. Replace that arm with:
```rust
      Format::MicroDvd => {
        let fps = match self {
          SubtitleFile::MicroDvd { fps, .. } => Some(*fps),
          _ => None,
        };
        crate::microdvd::to_string(subs, fps)
      }
```

- [ ] **Step 5: Update microdvd.rs parse_content to return the new variant**

In `src/microdvd.rs`, `parse_content` currently ends with `Ok(SubtitleFile::Srt(subtitles))`. The `saved_fps` variable holds the final fps. Change the return to:
```rust
  Ok(SubtitleFile::MicroDvd {
    fps: saved_fps,
    subtitles,
  })
```

- [ ] **Step 6: Update main.rs parse_to_file**

In `src/main.rs` `parse_to_file`, the `Format::MicroDvd` arm currently does:
```rust
    Format::MicroDvd => {
      let file = subtitler::microdvd::parse_content(&text, None)?;
      Ok(file)
    }
```
This still works (parse_content now returns `SubtitleFile::MicroDvd`). No change needed — verify.

- [ ] **Step 7: Run tests to verify pass**

Run: `cargo test --test arch_unification 2>&1 | tail -6`
Expected: both MicroDvd tests PASS.

Run full suite: `cargo test 2>&1 | grep -E "test result:" | tail -5`
Expected: all green. (The existing microdvd unit tests in `src/microdvd.rs` may construct `SubtitleFile::Srt(...)` — check: run `grep -n "SubtitleFile::Srt" src/microdvd.rs`. The test `test_round_trip` etc. call `parse_content` which now returns `MicroDvd`, so `file.subtitles()` still works. But if a test does `SubtitleFile::Srt(vec![...])` directly it must change to `MicroDvd`. Inspect and fix any.)

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "fix: MicroDvd variant preserves fps (was silently lost)"
```

---

## Task 5: Add `SubViewer` variant preserving header

Currently `subviewer::parse_content` returns `Vec<Subtitle>` and `to_string` hardcodes a default header. Add a `SubViewer { header, subtitles }` variant.

**Files:** Modify `src/model.rs`, `src/subviewer.rs`, `src/main.rs`

- [ ] **Step 1: Write the failing test**

Append to `tests/arch_unification.rs`:

```rust
#[test]
fn subviewer_variant_preserves_header() {
  // Custom [TITLE]/[AUTHOR] in the [INFORMATION] block must survive parse.
  let content = "[INFORMATION]\n[TITLE]My Film\n[AUTHOR]Me\n[END INFORMATION]\n[SUBTITLE]\n\n00:00:01.00,00:00:03.50\nHello\n";
  let result = subtitler::subviewer::parse_content(content);
  // parse_content currently returns Vec<Subtitle>; after the change it returns
  // (Option<String>, Vec<Subtitle>) or SubtitleFile::SubViewer. This test will
  // need updating once the signature changes — see Step 3.
  let _ = result;
}
```

(This test is a placeholder that we'll rewrite in Step 3 once the new return type exists. For now it just compiles and documents intent.)

- [ ] **Step 2: Add the variant to the enum**

In `src/model.rs`, add to `pub enum SubtitleFile`:
```rust
  SubViewer {
    #[serde(skip_serializing_if = "Option::is_none")]
    header: Option<String>,
    subtitles: Vec<Subtitle>,
  },
```

- [ ] **Step 3: Add match arms in model.rs**

`subtitles()`: add
```rust
      SubtitleFile::SubViewer { subtitles: subs, .. } => subs,
```
`subtitles_mut()`: add
```rust
      SubtitleFile::SubViewer { subtitles: subs, .. } => subs,
```
`format()`: add
```rust
      SubtitleFile::SubViewer { .. } => Format::SubViewer,
```
`to_string_with_format` — the `Format::SubViewer` arm currently calls `crate::subviewer::to_string(subs)`. Update to pass stored header:
```rust
      Format::SubViewer => {
        let header = match self {
          SubtitleFile::SubViewer { header, .. } => header.as_deref(),
          _ => None,
        };
        crate::subviewer::to_string(subs, header)
      }
```

- [ ] **Step 4: Change subviewer.rs to parse and preserve header**

In `src/subviewer.rs`, change `parse_content` signature and body. Currently:
```rust
pub fn parse_content(content: &str) -> AnyResult<Vec<Subtitle>> {
```
Change to capture header lines. The `[INFORMATION]...[END INFORMATION]` block (lines between `[INFORMATION]` and `[END INFORMATION]`) plus `[SUBTITLE]`/`[COLF]...` lines form the header. Replace the function with a version that collects bracket-header lines into an optional String before parsing subtitles. Concretely, add a `header_lines: Vec<String>` accumulator; lines matching `RE_SUBVIEWER_BRACKET` that appear before the first timestamp go into header; join them. Return `(Option<String>, Vec<Subtitle>)`:

```rust
pub fn parse_content(content: &str) -> AnyResult<(Option<String>, Vec<Subtitle>)> {
  let mut subtitles = Vec::new();
  let mut pending_timestamp: Option<(u64, u64)> = None;
  let mut header_lines: Vec<String> = Vec::new();
  let mut saw_timestamp = false;

  for line in content.lines() {
    let trimmed = line.trim();

    if trimmed.is_empty() {
      continue;
    }

    if RE_SUBVIEWER_BRACKET.is_match(trimmed) {
      if !saw_timestamp {
        header_lines.push(trimmed.to_string());
      }
      continue;
    }

    if let Some(caps) = RE_SUBVIEWER_LINE.captures(trimmed) {
      saw_timestamp = true;
      let start = parse_subviewer_time(&caps[1])?;
      let end = parse_subviewer_time(&caps[2])?;
      pending_timestamp = Some((start, end));
    } else if let Some((start, end)) = pending_timestamp.take() {
      subtitles.push(Subtitle::new(start, end, trimmed));
    }
  }

  let header = if header_lines.is_empty() {
    None
  } else {
    Some(header_lines.join("\n"))
  };

  Ok((header, subtitles))
}
```

Change `to_string` to accept an optional header:
```rust
pub fn to_string(subtitles: &[Subtitle], header: Option<&str>) -> String {
  let mut buf = match header {
    Some(h) => format!("{h}\n\n"),
    None => String::from("[INFORMATION]\n[TITLE]Subtitles\n[AUTHOR]subtitler\n[SOURCE]\n[FILEPATH]\n[DELAY]0\n[COMMENT]\n[END INFORMATION]\n[SUBTITLE]\n[COLF]&HFFFFFF,[STYLE]bd,[SIZE]18,[FONT]Arial\n\n"),
  };

  for sub in subtitles {
    let start = format_subviewer_time(sub.start);
    let end = format_subviewer_time(sub.end);
    buf.push_str(&format!("{},{}\n{}\n\n", start, end, sub.text));
  }

  buf
}
```

- [ ] **Step 5: Update subviewer.rs tests**

The subviewer unit tests call `parse_content(content).unwrap()` and treat the result as `Vec<Subtitle>` (e.g. `result.len()`, `result[0].text`). Now it returns a tuple. Update each test to destructure: `let (_, subs) = parse_content(content).unwrap();` then use `subs`. Run `grep -n "parse_content" src/subviewer.rs` to find all call sites in the test module and update them.

- [ ] **Step 6: Update main.rs**

In `src/main.rs`:
- `parse_to_file` `Format::SubViewer` arm: change `let subs = subtitler::subviewer::parse_content(&text)?; Ok(SubtitleFile::Srt(subs))` to:
  ```rust
    Format::SubViewer => {
      let (header, subs) = subtitler::subviewer::parse_content(&text)?;
      Ok(SubtitleFile::SubViewer { header, subtitles: subs })
    }
  ```
- `cmd_parse` `Format::SubViewer` arm: change `subtitler::subviewer::parse_content(&content)?` (treated as Vec) to destructure: `subtitler::subviewer::parse_content(&content)?.1`.

- [ ] **Step 7: Rewrite the test from Step 1 properly**

Replace the placeholder test in `tests/arch_unification.rs`:

```rust
#[test]
fn subviewer_variant_preserves_header() {
  let content = "[INFORMATION]\n[TITLE]My Film\n[AUTHOR]Me\n[END INFORMATION]\n[SUBTITLE]\n[COLF]&HFFFFFF,[STYLE]bd,[SIZE]18,[FONT]Arial\n\n00:00:01.00,00:00:03.50\nHello\n";
  let (header, subs) = subtitler::subviewer::parse_content(content).unwrap();
  assert!(header.as_deref().unwrap().contains("My Film"));
  assert_eq!(subs.len(), 1);
  assert_eq!(subs[0].text, "Hello");
}
```

- [ ] **Step 8: Run tests**

Run: `cargo test 2>&1 | grep -E "test result:|error\[" | tail -8`
Expected: all pass. If a subviewer test breaks because of the tuple change, fix it (Step 5 should have covered all).

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "fix: SubViewer variant preserves header (was silently lost)"
```

---

## Task 6: Stage 1 verification gate

- [ ] **Step 1: Format check**

Run: `cargo fmt -- --check`
If diff, run `cargo fmt` then re-check. Commit formatting if needed: `git commit -am "style: rustfmt"`.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --all-targets -- -D warnings 2>&1 | tail -5`
Expected: no warnings. Fix any.

- [ ] **Step 3: Full test suite**

Run: `cargo test 2>&1 | grep -E "test result:" | tail -5`
Expected: all green. Note the lib test count — it should have grown by the arch_unification tests.

- [ ] **Step 4: Confirm 6 variants exist**

Run: `grep -A8 "pub enum SubtitleFile" src/model.rs`
Expected: shows `Srt`, `Vtt`, `Ass(AssData)`, `Ssa(AssData)`, `MicroDvd { fps, subtitles }`, `SubViewer { header, subtitles }`.

- [ ] **Step 5: Confirm no `SubtitleFile::Srt` misuse for MicroDvd/SubViewer**

Run: `grep -rn "SubtitleFile::Srt" src/microdvd.rs src/subviewer.rs`
Expected: zero matches.

---

# STAGE 2 — `SubtitleFormat` trait

## Task 7: Define the `SubtitleFormat` trait with default method impls

Extract a trait that holds the 15 editing methods as defaults (operating only via `subtitles()`/`subtitles_mut()`), plus 4 required methods.

**Files:** Modify `src/model.rs`

- [ ] **Step 1: Define the trait**

In `src/model.rs`, immediately before `impl SubtitleFile {`, add the trait. The default method bodies are taken verbatim from the current `impl SubtitleFile` — they already use `self.subtitles()` / `self.subtitles_mut()` and so work unchanged inside the trait:

```rust
pub trait SubtitleFormat: Debug + Clone + Send + Sync {
  fn subtitles(&self) -> &[Subtitle];
  fn subtitles_mut(&mut self) -> &mut Vec<Subtitle>;
  fn format(&self) -> Format;
  fn to_string_with_format(&self, fmt: &Format) -> String;

  fn to_string(&self) -> String {
    self.to_string_with_format(&self.format())
  }

  fn shift_all(&mut self, offset_ms: i64) {
    for sub in self.subtitles_mut().iter_mut() {
      sub.shift(offset_ms);
    }
  }

  fn map<F: FnMut(&mut Subtitle)>(mut self, mut f: F) -> Self {
    for sub in self.subtitles_mut().iter_mut() {
      f(sub);
    }
    self
  }

  fn filter<F: FnMut(&Subtitle) -> bool>(mut self, mut f: F) -> Self {
    self.subtitles_mut().retain(|s| f(s));
    self
  }

  fn sort(&mut self) {
    self.subtitles_mut().sort_by_key(|s| (s.start, s.end));
  }

  fn validate(&self) -> Vec<ValidationIssue> {
    let subs = self.subtitles();
    let mut issues = Vec::new();

    for (i, sub) in subs.iter().enumerate() {
      if sub.end < sub.start {
        issues.push(ValidationIssue::NegativeDuration {
          index: i,
          start: sub.start,
          end: sub.end,
        });
      }
      if sub.start == sub.end {
        issues.push(ValidationIssue::ZeroDuration {
          index: i,
          time: sub.start,
        });
      }
    }

    let mut order: Vec<usize> = (0..subs.len()).collect();
    order.sort_by_key(|&i| (subs[i].start, subs[i].end));
    for w in order.windows(2) {
      let (a, b) = (w[0], w[1]);
      if subs[b].start < subs[a].end {
        issues.push(ValidationIssue::Overlap {
          index_a: a,
          index_b: b,
          end_a: subs[a].end,
          start_b: subs[b].start,
        });
      }
    }

    for i in 1..subs.len() {
      if subs[i].start < subs[i - 1].start {
        issues.push(ValidationIssue::DecreasingStartTime {
          index: i,
          prev_start: subs[i - 1].start,
          curr_start: subs[i].start,
        });
      }
    }

    issues
  }

  fn validate_extended(
    &self,
    max_chars: usize,
    max_gap_ms: u64,
    max_cps: f64,
  ) -> Vec<ValidationIssue> {
    let mut issues = self.validate();
    let subs = self.subtitles();

    for (i, sub) in subs.iter().enumerate() {
      let char_count = sub.text.chars().count();
      if char_count > max_chars {
        issues.push(ValidationIssue::TextTooLong {
          index: i,
          chars: char_count,
          max_chars,
        });
      }

      let cps = sub.chars_per_second();
      if cps > max_cps {
        issues.push(ValidationIssue::CpsTooHigh {
          index: i,
          cps,
          max_cps,
        });
      }
    }

    for i in 1..subs.len() {
      let gap = subs[i].start.saturating_sub(subs[i - 1].end);
      if gap > max_gap_ms {
        issues.push(ValidationIssue::TooLongGap {
          index: i,
          prev_end: subs[i - 1].end,
          curr_start: subs[i].start,
          gap_ms: gap,
        });
      }
    }

    issues
  }

  fn merge_adjacent(&mut self, max_gap_ms: u64) {
    self.sort();
    let subs = self.subtitles_mut();
    let mut i = 0;
    while i + 1 < subs.len() {
      let gap = subs[i + 1].start.saturating_sub(subs[i].end);
      if gap <= max_gap_ms {
        let next_text = subs[i + 1].text.clone();
        subs[i].end = subs[i + 1].end;
        subs[i].text.push('\n');
        subs[i].text.push_str(&next_text);
        subs.remove(i + 1);
      } else {
        i += 1;
      }
    }
  }

  fn remove_overlaps(&mut self) {
    self.sort();
    let subs = self.subtitles_mut();
    for i in 0..subs.len().saturating_sub(1) {
      if subs[i + 1].start < subs[i].end {
        subs[i + 1].start = subs[i].end;
      }
    }
  }

  fn enforce_min_duration(&mut self, min_ms: u64) {
    self.sort();
    let subs = self.subtitles_mut();
    for i in 0..subs.len() {
      let dur = subs[i].duration_ms();
      if dur < min_ms {
        let max_end = if i + 1 < subs.len() {
          subs[i + 1].start
        } else {
          u64::MAX
        };
        let desired_end = subs[i].start + min_ms;
        subs[i].end = desired_end.min(max_end);
      }
    }
  }

  fn enforce_max_duration(&mut self, max_ms: u64) {
    for sub in self.subtitles_mut().iter_mut() {
      let dur = sub.duration_ms();
      if dur > max_ms {
        sub.end = sub.start + max_ms;
      }
    }
  }

  fn auto_extend_for_cps(&mut self, max_cps: f64) {
    self.sort();
    let subs = self.subtitles_mut();
    for i in 0..subs.len() {
      let chars = subs[i].plaintext().chars().count() as f64;
      let needed_ms = (chars / max_cps * 1000.0).ceil() as u64;
      let current = subs[i].duration_ms();
      if current < needed_ms {
        let max_end = if i + 1 < subs.len() {
          subs[i + 1].start
        } else {
          u64::MAX
        };
        subs[i].end = (subs[i].start + needed_ms).min(max_end);
      }
    }
  }

  fn extract_range(&self, start_ms: u64, end_ms: u64) -> Vec<Subtitle> {
    self
      .subtitles()
      .iter()
      .filter(|s| s.start < end_ms && s.end > start_ms)
      .map(|s| {
        let mut clone = s.clone();
        if clone.start < start_ms {
          clone.start = start_ms;
        }
        if clone.end > end_ms {
          clone.end = end_ms;
        }
        clone
      })
      .collect()
  }

  fn split_long(&mut self, max_chars: usize) {
    self.sort();
    let subs = self.subtitles_mut();
    let mut i = 0;
    while i < subs.len() {
      let char_count = subs[i].text.chars().count();
      if char_count <= max_chars {
        i += 1;
        continue;
      }

      let start = subs[i].start;
      let end = subs[i].end;
      let duration = end - start;
      let text = std::mem::take(&mut subs[i].text);

      let chunks = split_text_chunks(&text, max_chars);
      let num_chunks = chunks.len() as u64;
      let chunk_duration = duration / num_chunks;

      subs[i].text = chunks[0].clone();
      subs[i].end = start + chunk_duration;

      for (chunk_idx, chunk) in chunks.iter().enumerate().skip(1) {
        let new_start = start + (chunk_idx as u64) * chunk_duration;
        let new_end = if chunk_idx + 1 == chunks.len() as usize {
          end
        } else {
          start + ((chunk_idx + 1) as u64) * chunk_duration
        };
        let mut new_sub = Subtitle::new(new_start, new_end, chunk);
        new_sub.style = subs[i].style.clone();
        new_sub.actor = subs[i].actor.clone();
        new_sub.layer = subs[i].layer;
        new_sub.margin_l = subs[i].margin_l;
        new_sub.margin_r = subs[i].margin_r;
        new_sub.margin_v = subs[i].margin_v;
        new_sub.effect = subs[i].effect.clone();
        i += 1;
        subs.insert(i, new_sub);
      }
      i += 1;
    }
  }

  fn transform_framerate(&mut self, in_fps: f64, out_fps: f64) {
    let ratio = out_fps / in_fps;
    for sub in self.subtitles_mut().iter_mut() {
      sub.start = ((sub.start as f64) * ratio).round() as u64;
      sub.end = ((sub.end as f64) * ratio).round() as u64;
    }
  }
}
```

`concatenate` is omitted from the trait (it takes `&SubtitleFile` — keep it on `impl SubtitleFile` as an inherent method; the spec's `&dyn SubtitleFormat` form can come later).

- [ ] **Step 2: Remove the moved methods from `impl SubtitleFile`**

In `impl SubtitleFile`, delete the bodies of: `shift_all`, `map`, `filter`, `to_string`, `sort`, `validate`, `validate_extended`, `merge_adjacent`, `remove_overlaps`, `enforce_min_duration`, `enforce_max_duration`, `auto_extend_for_cps`, `extract_range`, `split_long`, `transform_framerate`. Keep `subtitles`, `subtitles_mut`, `format`, `to_string_with_format`, `concatenate` (these stay as inherent methods or get re-exposed).

- [ ] **Step 3: Implement the trait for SubtitleFile**

Add after the trait definition:
```rust
impl SubtitleFormat for SubtitleFile {
  fn subtitles(&self) -> &[Subtitle] {
    match self {
      SubtitleFile::Srt(subs) => subs,
      SubtitleFile::Vtt { subtitles: subs, .. } => subs,
      SubtitleFile::Ass(data) | SubtitleFile::Ssa(data) => &data.subtitles,
      SubtitleFile::MicroDvd { subtitles: subs, .. } => subs,
      SubtitleFile::SubViewer { subtitles: subs, .. } => subs,
    }
  }
  fn subtitles_mut(&mut self) -> &mut Vec<Subtitle> {
    match self {
      SubtitleFile::Srt(subs) => subs,
      SubtitleFile::Vtt { subtitles: subs, .. } => subs,
      SubtitleFile::Ass(data) | SubtitleFile::Ssa(data) => &mut data.subtitles,
      SubtitleFile::MicroDvd { subtitles: subs, .. } => subs,
      SubtitleFile::SubViewer { subtitles: subs, .. } => subs,
    }
  }
  fn format(&self) -> Format {
    match self {
      SubtitleFile::Srt(_) => Format::Srt,
      SubtitleFile::Vtt { .. } => Format::Vtt,
      SubtitleFile::Ass(_) => Format::Ass,
      SubtitleFile::Ssa(_) => Format::Ssa,
      SubtitleFile::MicroDvd { .. } => Format::MicroDvd,
      SubtitleFile::SubViewer { .. } => Format::SubViewer,
    }
  }
  fn to_string_with_format(&self, format: &Format) -> String {
    let subs = self.subtitles();
    match format {
      Format::Srt => crate::srt::to_string(subs),
      Format::Vtt => crate::vtt::to_string(subs, None),
      Format::Ass | Format::Ssa => {
        let (info, styles) = match self {
          SubtitleFile::Ass(data) | SubtitleFile::Ssa(data) => (data.info.clone(), data.styles.clone()),
          _ => (
            std::collections::HashMap::new(),
            vec![crate::model::AssStyle::default_style()],
          ),
        };
        crate::ass::to_string(&info, &styles, subs)
      }
      Format::MicroDvd => {
        let fps = match self {
          SubtitleFile::MicroDvd { fps, .. } => Some(*fps),
          _ => None,
        };
        crate::microdvd::to_string(subs, fps)
      }
      Format::SubViewer => {
        let header = match self {
          SubtitleFile::SubViewer { header, .. } => header.as_deref(),
          _ => None,
        };
        crate::subviewer::to_string(subs, header)
      }
    }
  }
}
```

The 4 `match self` arms now live here; `impl SubtitleFile` retains `concatenate` and nothing else conflicts. Remove the now-duplicated `subtitles`/`subtitles_mut`/`format`/`to_string_with_format` from `impl SubtitleFile` (they moved to the trait impl).

- [ ] **Step 4: Export the trait**

In `src/lib.rs`, ensure the trait is reachable. The trait is in `model.rs` which is `pub mod model;`. Users get it via `use subtitler::model::SubtitleFormat;`. Add a re-export at the crate root for convenience: in `src/lib.rs` add `pub use model::SubtitleFormat;` near the top.

- [ ] **Step 5: Verify build + tests**

Run: `cargo test 2>&1 | grep -E "test result:|error" | tail -8`
Expected: all pass. All existing call sites (`file.validate()`, `file.shift_all()`, etc.) resolve via the trait impl — Rust allows calling trait methods on the concrete type without explicit `use SubtitleFormat` *as long as the trait is in scope at the call site*. For the library's own code (main.rs, tests), add `use subtitler::model::SubtitleFormat;` where needed if the compiler complains about method not found.

If the compiler says "method `validate` not found", add `use subtitler::model::SubtitleFormat;` to the top of `src/main.rs` and any test file that calls these methods.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: extract SubtitleFormat trait with default method impls"
```

---

## Task 8: Stage 2 verification — editing methods work across all variants

- [ ] **Step 1: Write parametrized tests**

Append to `tests/arch_unification.rs`:

```rust
use subtitler::model::{Format, SubtitleFile, SubtitleFormat};

fn sample_of_each_variant() -> Vec<SubtitleFile> {
  vec![
    SubtitleFile::Srt(vec![sub(0, 2000, "a"), sub(3000, 5000, "b")]),
    SubtitleFile::Vtt { header: None, subtitles: vec![sub(0, 2000, "a"), sub(3000, 5000, "b")] },
    SubtitleFile::Ass(subtitler::model::AssData {
      info: Default::default(),
      styles: vec![],
      subtitles: vec![sub(0, 2000, "a"), sub(3000, 5000, "b")],
    }),
    SubtitleFile::Ssa(subtitler::model::AssData {
      info: Default::default(),
      styles: vec![],
      subtitles: vec![sub(0, 2000, "a"), sub(3000, 5000, "b")],
    }),
    SubtitleFile::MicroDvd { fps: 25.0, subtitles: vec![sub(0, 2000, "a"), sub(3000, 5000, "b")] },
    SubtitleFile::SubViewer { header: None, subtitles: vec![sub(0, 2000, "a"), sub(3000, 5000, "b")] },
  ]
}

fn sub(start: u64, end: u64, text: &str) -> subtitler::model::Subtitle {
  subtitler::model::Subtitle::new(start, end, text)
}

#[test]
fn shift_all_works_for_every_variant() {
  for (i, mut file) in sample_of_each_variant().into_iter().enumerate() {
    file.shift_all(1000);
    let first_start = file.subtitles()[0].start;
    assert_eq!(first_start, 1000, "variant {i} shift_all failed");
  }
}

#[test]
fn format_reports_correctly_for_every_variant() {
  let expected = [
    Format::Srt, Format::Vtt, Format::Ass, Format::Ssa, Format::MicroDvd, Format::SubViewer,
  ];
  for (i, file) in sample_of_each_variant().into_iter().enumerate() {
    assert_eq!(file.format(), expected[i], "variant {i} format wrong");
  }
}

#[test]
fn validate_clean_for_every_variant() {
  for (i, file) in sample_of_each_variant().into_iter().enumerate() {
    let issues = file.validate();
    assert!(issues.is_empty(), "variant {i} reported {issues:?}");
  }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test arch_unification 2>&1 | tail -8`
Expected: all pass — proves the trait default methods work on all 6 variants.

- [ ] **Step 3: Full suite**

Run: `cargo test 2>&1 | grep -E "test result:" | tail -5`
Expected: green.

- [ ] **Step 4: Commit**

```bash
git add tests/arch_unification.rs
git commit -m "test: parametrized editing-method coverage across 6 variants"
```

---

# STAGE 3 — Feature flags, unified entry point, sync cores

## Task 9: Add per-format Cargo feature flags

**Files:** Modify `Cargo.toml`, `src/lib.rs`, format modules

- [ ] **Step 1: Add features to Cargo.toml**

In `Cargo.toml`, replace the `[features]` block:
```toml
[features]
default = ["srt", "vtt", "ass", "ssa", "microdvd", "subviewer", "http"]

srt = []
vtt = []
ass = []
ssa = []
microdvd = []
subviewer = []
http = ["reqwest"]
```

- [ ] **Step 2: Gate the format modules in lib.rs**

In `src/lib.rs`, replace the module declarations:
```rust
#[cfg(feature = "srt")]
pub mod srt;
#[cfg(feature = "vtt")]
pub mod vtt;
#[cfg(feature = "ass")]
pub mod ass;
#[cfg(feature = "microdvd")]
pub mod microdvd;
#[cfg(feature = "subviewer")]
pub mod subviewer;
pub mod config;
pub mod encoding;
pub mod error;
pub mod model;
pub mod normalize;
pub mod types;
pub mod utils;

pub use model::SubtitleFormat;
```

(`ssa` shares the `ass` module — no separate module, only a feature that gates the `Format::Ssa` variant; handled in Step 4.)

- [ ] **Step 3: Gate Format enum variants**

In `src/model.rs`, gate the `Format` enum variants and `SubtitleFile` variants with `#[cfg(feature = "...")]`. Replace the `Format` enum:
```rust
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum Format {
  #[cfg(feature = "srt")]
  Srt,
  #[cfg(feature = "vtt")]
  Vtt,
  #[cfg(feature = "ass")]
  Ass,
  #[cfg(feature = "ssa")]
  Ssa,
  #[cfg(feature = "microdvd")]
  MicroDvd,
  #[cfg(feature = "subviewer")]
  SubViewer,
}
```
And gate `SubtitleFile` variants the same way (each variant gets a `#[cfg(feature = "...")]` attribute above it). `Ass`/`Ssa` both gate on their respective features.

- [ ] **Step 4: Gate match arms**

Every `match self` (in the trait impl) and `match format` (in `to_string_with_format`) arm must be `#[cfg]`-gated to match the enabled variants. This is the tedious part. For the trait impl's `subtitles()`:
```rust
  fn subtitles(&self) -> &[Subtitle] {
    match self {
      #[cfg(feature = "srt")]
      SubtitleFile::Srt(subs) => subs,
      #[cfg(feature = "vtt")]
      SubtitleFile::Vtt { subtitles: subs, .. } => subs,
      #[cfg(feature = "ass")]
      SubtitleFile::Ass(data) => &data.subtitles,
      #[cfg(feature = "ssa")]
      SubtitleFile::Ssa(data) => &data.subtitles,
      #[cfg(feature = "microdvd")]
      SubtitleFile::MicroDvd { subtitles: subs, .. } => subs,
      #[cfg(feature = "subviewer")]
      SubtitleFile::SubViewer { subtitles: subs, .. } => subs,
    }
  }
```
Apply the same `#[cfg]` to every arm in `subtitles_mut`, `format`, and `to_string_with_format`'s inner `match format`. **Important:** if ALL features are on (the default), this compiles identically to before. The `#[cfg]` attributes only matter when features are off.

For `to_string_with_format`'s `Format::MicroDvd` arm that reads `fps` via `match self`, also gate that inner match's arms.

- [ ] **Step 5: Gate detect_format in lib.rs**

`detect_format` chains `srt::detect_format(data).or_else(...)`. Each call must be `#[cfg]`-gated:
```rust
pub fn detect_format(data: &[u8]) -> Option<Format> {
  #[cfg(feature = "srt")]
  let f = srt::detect_format(data);
  #[cfg(not(feature = "srt"))]
  let f: Option<Format> = None;

  #[cfg(feature = "vtt")]
  let f = f.or_else(|| vtt::detect_format(data));
  #[cfg(feature = "ass")]
  let f = f.or_else(|| ass::detect_format(data));
  #[cfg(feature = "microdvd")]
  let f = f.or_else(|| microdvd::detect_format(data));
  #[cfg(feature = "subviewer")]
  let f = f.or_else(|| subviewer::detect_format(data));
  f
}
```
(Ssa detection is folded into ass::detect_format which returns `Format::Ass` or `Format::Ssa`; that module needs `#[cfg(feature="ssa")]` on the Ssa-returning branch — see Step 6.)

- [ ] **Step 6: Gate main.rs format routing**

In `src/main.rs`, the `Format` match in `parse_to_file`, `cmd_parse`, `resolve_format`, `cmd_detect`, `format_to_subtitle_format` all need `#[cfg]` arms. For each `match` over `Format`, add `#[cfg(feature="x")]` above each arm. The `_ =>` fallback arm catches disabled formats.

- [ ] **Step 7: Verify default build (all features)**

Run: `cargo build 2>&1 | tail -3` then `cargo test 2>&1 | grep -E "test result:" | tail -5`
Expected: builds, all tests pass (default features unchanged).

- [ ] **Step 8: Verify a subset build**

Run: `cargo build --no-default-features --features srt,vtt 2>&1 | tail -5`
Expected: compiles. If it fails with "variant not found" or "unreachable pattern", a `#[cfg]` gate is missing — find and add it.

Run: `cargo build --no-default-features --features ass,ssa 2>&1 | tail -5`
Expected: compiles.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat: per-format cargo feature flags"
```

---

## Task 10: Convert SRT/VTT parsing cores to sync

SRT and VTT `parse_content`/`parse_bytes` are currently `async` but only `.await` on a synchronous `Cursor`. Make the cores sync; keep `parse_file`/`parse_url` async (real I/O).

**Files:** Modify `src/srt.rs`, `src/vtt.rs`, `src/main.rs`

- [ ] **Step 1: Make srt::parse_content sync**

In `src/srt.rs`, the inner `async fn parse<R>(reader: R)` uses `reader.lines()` + `.await`. Replace it with a sync version operating on `&str`. Concretely, change `parse_content` to call a sync helper:

```rust
pub fn parse_content(content: &str) -> AnyResult<Vec<Subtitle>> {
  parse_lines(content)
}

fn parse_lines(content: &str) -> AnyResult<Vec<Subtitle>> {
  // body is the same as the old async parse(), but:
  //   - `while let Some(line) = lines.next_line().await?` becomes `for line in content.lines()`
  //   - `let mut trimmed = line.trim().to_string();` stays
  //   - all other logic identical
}
```

Copy the body of the old `async fn parse` into `fn parse_lines`, changing only the line-iteration construct (`for line in content.lines()` with `line` already being `&str`, so `line.trim()` instead of `line.trim().to_string()` where ownership isn't needed — but to minimize risk, keep `.to_string()`).

- [ ] **Step 2: Make srt::parse_bytes sync**

```rust
pub fn parse_bytes(data: &[u8]) -> AnyResult<Vec<Subtitle>> {
  let text = String::from_utf8(data.to_vec()).map_err(|e| anyhow!("Invalid UTF-8: {}", e))?;
  parse_content(&text)
}
```
(Remove `async` and the `Cursor`/`BufReader` dance.)

- [ ] **Step 3: Keep srt::parse_file and parse_url async**

`parse_file` does real file I/O — keep `async`, but it now calls the sync `parse_content`:
```rust
pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<Vec<Subtitle>> {
  let text = tokio::fs::read_to_string(path).await?;
  parse_content(&text)
}

#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<Vec<Subtitle>> {
  let response = reqwest::get(url).await?;
  let content = response.text().await?;
  parse_content(&content)
}
```
Remove now-unused imports (`Cursor`, `AsyncBufReadExt`, `BufReader`, `File`) if the compiler warns.

- [ ] **Step 4: Apply the same to vtt.rs**

Repeat Steps 1–3 for `vtt.rs`: `parse_content`, `parse_content_full`, `parse_bytes` become sync (returning `Vec<Subtitle>` / `(Option<String>, Vec<Subtitle>)`); `parse_file`/`parse_url` stay async calling the sync cores. VTT has `parse_content_full` returning `(Option<String>, Vec<Subtitle>)` — keep that signature sync.

- [ ] **Step 5: Update main.rs call sites**

In `src/main.rs`, every `.await?` on `srt::parse_content`/`parse_bytes`/`vtt::parse_content`/`parse_content_full` must drop `.await`. Run: `grep -n "srt::parse\|vtt::parse" src/main.rs`. For each, remove `.await`. E.g. `let subs = srt::parse_content(&text).await?;` → `let subs = srt::parse_content(&text)?;`.

- [ ] **Step 6: Update call sites in srt.rs/vtt.rs tests**

The unit tests in `src/srt.rs` and `src/vtt.rs` use `#[tokio::test] async fn` and call `.await` on `parse_content`. Change those tests to plain `#[test] fn` and drop `.await`. Run `grep -n "async fn test_\|\.await" src/srt.rs src/vtt.rs` and convert each. The `test_round_trip` tests call `generate(...).await` (file I/O) — those STAY async (generate writes a file). Only the `parse_content(...).await` calls become sync.

- [ ] **Step 7: Verify build + tests**

Run: `cargo test 2>&1 | grep -E "test result:|error" | tail -8`
Expected: all pass. If a test still has `.await` on a sync call, the compiler will flag it — remove.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor: sync parsing cores (srt/vtt), keep I/O async"
```

---

## Task 11: Add unified `parse_bytes` / `parse_file` / `parse_url` entry points

**Files:** Modify `src/lib.rs`, `src/error.rs`

- [ ] **Step 1: Add ParseError to error.rs**

In `src/error.rs`, append:
```rust
use crate::model::Format;

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
Note: `Format` must derive `Debug` (it does). The `Unsupported(Format)` arm needs `Format: Debug` — already true.

- [ ] **Step 2: Add unified entry points to lib.rs**

In `src/lib.rs`, add (each arm gated by feature):
```rust
use model::Format;

pub fn parse_bytes(data: &[u8]) -> Result<SubtitleFile, error::ParseError> {
  let fmt = detect_format(data).ok_or(error::ParseError::UnknownFormat)?;
  parse_bytes_as(data, fmt)
}

pub fn parse_bytes_as(data: &[u8], fmt: Format) -> Result<SubtitleFile, error::ParseError> {
  match fmt {
    #[cfg(feature = "srt")]
    Format::Srt => Ok(SubtitleFile::Srt(srt::parse_bytes(data)?)),
    #[cfg(feature = "vtt")]
    Format::Vtt => {
      let (header, subs) = vtt::parse_bytes(data)?;
      Ok(SubtitleFile::Vtt { header, subtitles: subs })
    }
    #[cfg(feature = "ass")]
    Format::Ass => Ok(SubtitleFile::Ass(ass::parse_bytes(data)?)),
    #[cfg(feature = "ssa")]
    Format::Ssa => Ok(SubtitleFile::Ssa(ass::parse_bytes(data)?)),
    #[cfg(feature = "microdvd")]
    Format::MicroDvd => {
      let (fps, subs) = microdvd::parse_bytes(data)?;
      Ok(SubtitleFile::MicroDvd { fps, subtitles: subs })
    }
    #[cfg(feature = "subviewer")]
    Format::SubViewer => {
      let (header, subs) = subviewer::parse_bytes(data)?;
      Ok(SubtitleFile::SubViewer { header, subtitles: subs })
    }
    _ => Err(error::ParseError::Unsupported(fmt)),
  }
}

pub async fn parse_file(path: impl AsRef<std::path::Path>) -> Result<SubtitleFile, error::ParseError> {
  let data = tokio::fs::read(path).await?;
  parse_bytes(&data)
}

#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> Result<SubtitleFile, error::ParseError> {
  let response = reqwest::get(url).await?;
  let bytes = response.bytes().await?;
  parse_bytes(&bytes)
}
```

Note: this requires `microdvd::parse_bytes` and `subviewer::parse_bytes` and `ass::parse_bytes` to exist. `ass::parse_bytes` exists (added in cleanup batch). Add `microdvd::parse_bytes` and `subviewer::parse_bytes`:
- In `src/microdvd.rs` add:
  ```rust
  pub fn parse_bytes(data: &[u8], fps: Option<f64>) -> AnyResult<(f64, Vec<Subtitle>)> {
    let text = String::from_utf8(data.to_vec()).map_err(|e| anyhow!("Invalid UTF-8: {}", e))?;
    let file = parse_content(&text, fps)?;
    match file {
      SubtitleFile::MicroDvd { fps, subtitles } => Ok((fps, subtitles)),
      _ => unreachable!(),
    }
  }
  ```
- In `src/subviewer.rs` add:
  ```rust
  pub fn parse_bytes(data: &[u8]) -> AnyResult<(Option<String>, Vec<Subtitle>)> {
    let text = String::from_utf8(data.to_vec()).map_err(|e| anyhow!("Invalid UTF-8: {}", e))?;
    parse_content(&text)
  }
  ```
- The lib.rs unified entry calls `microdvd::parse_bytes(data)?` — but it takes `fps: Option<f64>`. Pass `None`: change the arm to `microdvd::parse_bytes(data, None)?`. Fix the snippet above.

- [ ] **Step 3: Write tests for unified entry**

Append to `tests/arch_unification.rs`:
```rust
#[test]
fn unified_parse_bytes_detects_srt() {
  let data = b"1\n00:00:01,000 --> 00:00:03,500\nHello\n\n";
  let file = subtitler::parse_bytes(data).unwrap();
  assert!(matches!(file, subtitler::model::SubtitleFile::Srt(_)));
}

#[test]
fn unified_parse_bytes_detects_vtt() {
  let data = b"WEBVTT\n\n00:00:01.000 --> 00:00:03.500\nHello\n\n";
  let file = subtitler::parse_bytes(data).unwrap();
  assert!(matches!(file, subtitler::model::SubtitleFile::Vtt { .. }));
}

#[test]
fn unified_parse_bytes_unknown_format_errors() {
  let result = subtitler::parse_bytes(b"not a subtitle at all\nnope\n");
  assert!(matches!(result, Err(subtitler::error::ParseError::UnknownFormat)));
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test 2>&1 | grep -E "test result:|error" | tail -8`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: unified parse_bytes/file/url entry points + ParseError"
```

---

## Task 12: Bump to 1.0.0, MIGRATION.md, CHANGELOG, README

**Files:** `Cargo.toml`, `MIGRATION.md` (create), `CHANGELOG.md`, `README.md`

- [ ] **Step 1: Bump version**

In `Cargo.toml`, change `version = "0.1.0"` → `version = "1.0.0"`.

- [ ] **Step 2: Create MIGRATION.md**

```markdown
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

## New `SubtitleFile` variants: `MicroDvd`, `SubViewer`, `Ssa`

MicroDVD and SubViewer no longer collapse into `Srt`. If you pattern-matched on `SubtitleFile`, add arms:

```rust
// Before: MicroDVD parsed as SubtitleFile::Srt(...)
// After:
match file {
  subtitler::model::SubtitleFile::MicroDvd { fps, subtitles } => { /* fps now preserved */ }
  subtitler::model::SubtitleFile::SubViewer { header, subtitles } => { /* header now preserved */ }
  subtitler::model::SubtitleFile::Ssa(data) => { /* same shape as Ass(AssData) */ }
  _ => {}
}
```

## Parsing cores are now sync

`parse_content` / `parse_bytes` are no longer `async` (they never did real async I/O):

```rust
// Before
let subs = subtitler::srt::parse_content(&text).await?;

// After
let subs = subtitler::srt::parse_content(&text)?;
```

`parse_file` and `parse_url` remain async (real I/O).

## New unified entry points (recommended)

```rust
// Auto-detect and parse any format
let file = subtitler::parse_bytes(&data)?;
let file = subtitler::parse_file("path.sub").await?;
```

## Per-format feature flags

If you use `default-features = false`, enable the formats you need:
```toml
[dependencies]
subtitler = { version = "1.0", default-features = false, features = ["srt", "vtt"] }
```

## New `SubtitleFormat` trait

Methods like `validate()`, `shift_all()`, `merge_adjacent()` are now on the `SubtitleFormat` trait. Bring it into scope:
```rust
use subtitler::model::SubtitleFormat;
file.validate(); // works
```
```

- [ ] **Step 3: Add CHANGELOG 1.0.0 entry**

Prepend to `CHANGELOG.md` (above `[Unreleased]`):
```markdown
## [1.0.0] - 2026-07-15

### Added
- `SubtitleFormat` trait consolidating editing methods (shift_all, validate,
  merge_adjacent, split_long, etc.) with default implementations.
- Unified parse entry points: `subtitler::parse_bytes`, `parse_file`, `parse_url`
  with auto-format-detection.
- `ParseError` typed error.
- Per-format Cargo feature flags (`srt`, `vtt`, `ass`, `ssa`, `microdvd`,
  `subviewer`) for compile-size trimming.
- `AssData` shared struct for ASS/SSA.

### Changed
- **[BREAKING]** `SubtitleFormat` enum renamed to `Format`.
- **[BREAKING]** `SubtitleFile` enum expanded: `MicroDvd` and `SubViewer` are now
  first-class variants (previously collapsed into `Srt`, losing fps/header).
  `Ssa(AssData)` variant added; `Ass` now wraps `AssData`.
- **[BREAKING]** `srt::parse_content`, `parse_bytes` and `vtt:: equivalents are
  now sync (not `async`). `parse_file`/`parse_url` remain async.
- MicroDVD round-trip now preserves fps; SubViewer round-trip now preserves the
  `[INFORMATION]` header. (Previously silently lost.)

### Removed
- Implicit degradation of MicroDVD/SubViewer into the Srt variant.
```

- [ ] **Step 4: Update README**

In `README.md`, update: version references to `1.0`, add a "Feature flags" section, add the unified entry point to the quick-start, and update the `SubtitleFile` description to mention 6 variants. (Read the current README quick-start section first and edit accordingly — don't rewrite wholesale.)

- [ ] **Step 5: Verify build + tests**

Run: `cargo build 2>&1 | tail -3` (expect `Compiling subtitler v1.0.0`).
Run: `cargo test 2>&1 | grep -E "test result:" | tail -5` (all pass).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "chore: bump to 1.0.0; add MIGRATION.md and CHANGELOG"
```

---

## Task 13: Final verification gate

- [ ] **Step 1: fmt**

Run: `cargo fmt -- --check` (fix with `cargo fmt` if needed).

- [ ] **Step 2: clippy**

Run: `cargo clippy --all-targets -- -D warnings 2>&1 | tail -5`
Expected: clean. (The `#[allow(clippy::inherent_to_string)]` on the old inherent `to_string` may now be on a trait method — if clippy complains differently, adjust.)

- [ ] **Step 3: full test suite**

Run: `cargo test --all-targets 2>&1 | grep -E "test result:" | tail -6`
Expected: all green.

- [ ] **Step 4: feature-matrix builds**

Run:
```
cargo build --no-default-features --features srt,vtt 2>&1 | tail -2
cargo build --no-default-features --features ass,ssa 2>&1 | tail -2
cargo build --no-default-features --features microdvd 2>&1 | tail -2
cargo build --no-default-features --features http,srt 2>&1 | tail -2
cargo build --no-default-features 2>&1 | tail -2
```
Expected: each compiles. The last (no features) should still compile an empty-ish crate. If any fails, a `#[cfg]` gate is missing — locate and fix.

- [ ] **Step 5: CLI smoke**

Run: `cargo run -- parse examples/example.srt 2>&1 | tail -3` (parses, prints).
Run: `cargo run -- detect examples/example.vtt 2>&1 | tail -1` (prints `vtt`).

- [ ] **Step 6: Confirm 6 variants + trait + rename**

Run: `grep -rn "SubtitleFormat" src/ | grep -v "trait SubtitleFormat\|use.*SubtitleFormat\|pub use" | head`
Expected: zero references to `SubtitleFormat` as an enum (only as the trait or imports).

- [ ] **Step 7: Final commit if any fixes**

If Steps 1–6 required fixes, commit them: `git commit -am "fix: final verification adjustments"`.

---

## Notes for the implementer

- **2-space indent** per `rustfmt.toml` — match it in all hand-written code.
- **Stage boundaries are commit boundaries.** Stage 1 (Tasks 1–6), Stage 2 (Tasks 7–8), Stage 3 (Tasks 9–13). Run the full test suite at the end of each stage.
- **`#[cfg]` gating is the highest-risk part** (Task 9). The discipline: every `match` over `SubtitleFile` or `Format` must gate each arm with the matching feature. When all features are on (default), `#[cfg]` is inert — use that to verify the "happy path" compiles before testing subsets.
- **The trait methods must be brought into scope** at call sites: `use subtitler::model::SubtitleFormat;`. If the compiler reports "method not found", this is the cause.
- **Don't change parsing *logic*.** The sync conversion (Task 10) is mechanical: `while let Some(line) = lines.next_line().await?` → `for line in content.lines()`, nothing else. If a parser's behavior changes, something went wrong.
- **Test round-trips carefully** for MicroDVD (fps) and SubViewer (header) — these are the bug fixes that justify the breaking change.
