# Cleanup Batch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix performance hotspots (regex recompilation, redundant parser scans), correctness bugs (validate() overlap false-negatives, chars_per_second on tagged text, stale output indices), and architecture debt (unused thiserror, oversized tokio features, asymmetric ASS entry points) — plus a blocking compile fix — then release as `0.1.0`.

**Architecture:** Conservative (Approach A from the spec): no change to the `SubtitleFile` enum shape, no async/sync parser rewrites, no new formats. Performance wins come from lifting regexes to `LazyLock`. Correctness comes from a sorted-view overlap scan and plaintext-based counting. Architecture comes from a new opt-in `error.rs`, trimmed tokio features, and ASS parser entry points mirroring SRT/VTT.

**Tech Stack:** Rust 2024 edition, regex, tokio (trimmed features), thiserror, clap, chardetng 1.0.

**Spec:** `docs/superpowers/specs/2026-07-15-cleanup-batch-design.md`

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `src/encoding.rs` | Modify | Fix chardetng 1.0 API call (blocking compile fix) |
| `src/model.rs` | Modify | LazyLock regexes in `strip_tags`/`plaintext`; `chars_per_second` plaintext; `validate` sorted-view overlap; positional indices helper |
| `src/srt.rs` | Modify | Remove double-scan; LazyLock in `detect_format`; positional indices in `to_string` |
| `src/vtt.rs` | Modify | LazyLock in `extract_text_parts`; positional indices in `to_string` |
| `src/ass.rs` | Modify | LazyLock in `parse_ass_tags`/`ass_to_plaintext`; add `parse_bytes`/`parse_file`/`parse_url` |
| `src/normalize.rs` | Modify | LazyLock array for `fix_ocr_errors` |
| `src/error.rs` | Create | `SubtitleError` enum (typed errors) |
| `src/lib.rs` | Modify | Wire `pub mod error;` |
| `Cargo.toml` | Modify | Trim tokio features; bump version to 0.1.0 |
| `tests/cleanup_batch.rs` | Create | Integration tests for behavior changes |
| `CHANGELOG.md` | Create | Release notes |
| `README.md` | Modify | Document changes, version refs |
| `benches/subtitler_benchmark.rs` | Modify | Add regex-hotspot benchmark group |

---

## Task 0: Fix blocking compile error (chardetng 1.0 API)

This must be done first — nothing else can be verified until the crate compiles. `chardetng 1.0.0` changed `EncodingDetector::new()` to take an `Iso2022JpDetection` arg and removed `guess_assess()` in favor of `guess()`.

**Files:**
- Modify: `src/encoding.rs:17-20`

- [ ] **Step 1: Fix the chardetng API calls**

In `src/encoding.rs`, find this block (lines 17-20):

```rust
  let mut detector = chardetng::EncodingDetector::new();
  detector.feed(data, true);
  let (encoding, _confident) = detector.guess_assess(None, true);
  encoding.name()
```

Replace with:

```rust
  let mut detector = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Allow);
  detector.feed(data, true);
  let encoding = detector.guess(None, chardetng::Utf8Detection::Allow);
  encoding.name()
```

- [ ] **Step 2: Verify the crate compiles**

Run: `cargo build 2>&1 | tail -5`
Expected: `Finished` with no errors.

- [ ] **Step 3: Verify the full test suite passes**

Run: `cargo test 2>&1 | tail -5`
Expected: all test results `ok`.

- [ ] **Step 4: Commit**

```bash
git add src/encoding.rs
git commit -m "fix: update chardetng calls to 1.0 API (new + guess)"
```

---

## Task 1: Lift regexes to LazyLock in `model.rs` (Performance §1.1)

**Files:**
- Modify: `src/model.rs` (top of file imports + `strip_tags` at ~line 83 + `plaintext` at ~line 91)

- [ ] **Step 1: Add LazyLock statics at the top of `model.rs`**

After the existing `use serde::{Deserialize, Serialize};` line (line 1), add:

```rust
use std::sync::LazyLock;

static RE_HTML_TAG: LazyLock<regex::Regex> = LazyLock::new(|| {
  regex::Regex::new(r"</?(?:b|i|u|s|font|v|c)(?:\.[^>]*)?(?:\s[^>]*)?>").unwrap()
});

static RE_ASS_TAG: LazyLock<regex::Regex> =
  LazyLock::new(|| regex::Regex::new(r"\{[^}]*\}").unwrap());
```

- [ ] **Step 2: Replace `strip_tags` regex usage**

Find `strip_tags` (currently lines 83-89):

```rust
  pub fn strip_tags(&mut self) {
    let re = regex::Regex::new(r"</?(?:b|i|u|s|font|v|c)(?:\.[^>]*)?(?:\s[^>]*)?>").unwrap();
    self.text = re.replace_all(&self.text, "").to_string();
    let re_ass = regex::Regex::new(r"\{[^}]*\}").unwrap();
    self.text = re_ass.replace_all(&self.text, "").to_string();
    self.text_parts.clear();
  }
```

Replace with:

```rust
  pub fn strip_tags(&mut self) {
    self.text = RE_HTML_TAG.replace_all(&self.text, "").to_string();
    self.text = RE_ASS_TAG.replace_all(&self.text, "").to_string();
    self.text_parts.clear();
  }
```

- [ ] **Step 3: Replace `plaintext` regex usage**

Find `plaintext` (currently lines 91-98):

```rust
  pub fn plaintext(&self) -> String {
    let mut text = self.text.clone();
    let re_html = regex::Regex::new(r"</?(?:b|i|u|s|font|v|c)(?:\.[^>]*)?(?:\s[^>]*)?>").unwrap();
    text = re_html.replace_all(&text, "").to_string();
    let re_ass = regex::Regex::new(r"\{[^}]*\}").unwrap();
    text = re_ass.replace_all(&text, "").to_string();
    text.replace("\\N", "\n").replace("\\n", "\n").replace("\\h", " ")
  }
```

Replace with:

```rust
  pub fn plaintext(&self) -> String {
    let mut text = self.text.clone();
    text = RE_HTML_TAG.replace_all(&text, "").to_string();
    text = RE_ASS_TAG.replace_all(&text, "").to_string();
    text.replace("\\N", "\n").replace("\\n", "\n").replace("\\h", " ")
  }
```

- [ ] **Step 4: Verify build + tests**

Run: `cargo test 2>&1 | tail -5`
Expected: compiles, all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/model.rs
git commit -m "perf: lift strip_tags/plaintext regexes to LazyLock"
```

---

## Task 2: Make `chars_per_second` count plaintext (Correctness §2.3)

This changes `chars_per_second` so it counts the *visible* text, not the raw tagged text. All three callers (`validate_extended`, `auto_extend_for_cps`, `cmd_info`) benefit.

**Files:**
- Modify: `src/model.rs` (`chars_per_second` at ~line 64, test at ~line 825)

- [ ] **Step 1: Write the failing test**

In the `#[cfg(test)] mod tests` block at the bottom of `src/model.rs`, find `test_chars_per_second` (currently line 825):

```rust
  #[test]
  fn test_chars_per_second() {
    let sub = Subtitle::new(0, 2000, "Hello World"); // 11 chars / 2s = 5.5
    assert!((sub.chars_per_second() - 5.5).abs() < 0.01);
  }
```

Replace it with these two tests:

```rust
  #[test]
  fn test_chars_per_second() {
    let sub = Subtitle::new(0, 2000, "Hello World"); // 11 chars / 2s = 5.5
    assert!((sub.chars_per_second() - 5.5).abs() < 0.01);
  }

  #[test]
  fn test_chars_per_second_counts_plaintext() {
    // Tags must NOT count toward cps. "<b>Hi</b>" = 2 visible chars.
    let sub = Subtitle::new(0, 1000, "<b>Hi</b>");
    assert!((sub.chars_per_second() - 2.0).abs() < 0.01);
  }
```

- [ ] **Step 2: Run the new test to verify it fails**

Run: `cargo test test_chars_per_second_counts_plaintext 2>&1 | tail -15`
Expected: FAIL — assertion fails because current code counts 9 chars (`<b>Hi</b>`) → 9.0 cps, not 2.0.

- [ ] **Step 3: Implement the fix**

Find `chars_per_second` (currently lines 64-71):

```rust
  pub fn chars_per_second(&self) -> f64 {
    let dur = self.duration_ms() as f64 / 1000.0;
    if dur > 0.0 {
      self.text.chars().count() as f64 / dur
    } else {
      f64::INFINITY
    }
  }
```

Replace with:

```rust
  pub fn chars_per_second(&self) -> f64 {
    let dur = self.duration_ms() as f64 / 1000.0;
    if dur > 0.0 {
      self.plaintext().chars().count() as f64 / dur
    } else {
      f64::INFINITY
    }
  }
```

- [ ] **Step 4: Run tests to verify both pass**

Run: `cargo test test_chars_per_second 2>&1 | tail -10`
Expected: both `test_chars_per_second` and `test_chars_per_second_counts_plaintext` PASS.

- [ ] **Step 5: Run full suite to check for regressions**

Run: `cargo test 2>&1 | tail -5`
Expected: all tests pass. (No other test asserts raw-tag cps, so none should break.)

- [ ] **Step 6: Commit**

```bash
git add src/model.rs
git commit -m "fix: chars_per_second counts plaintext, not tagged text"
```

---

## Task 3: Fix `validate()` overlap false-negatives (Correctness §2.1)

`validate()` assumes sorted input and `break`s early — false negatives on unsorted input. Fix: scan adjacent pairs in a start-sorted index view while reporting original indices.

**Files:**
- Modify: `src/model.rs` (`validate` at ~line 398)
- Test: `tests/cleanup_batch.rs` (create)

- [ ] **Step 1: Write the failing integration test**

Create `tests/cleanup_batch.rs`:

```rust
use subtitler::model::{Subtitle, SubtitleFile, ValidationIssue};

#[test]
fn validate_detects_overlap_in_unsorted_input() {
  // Subtitle B (index 1) ends at 4000, but subtitle A (index 0) starts at 2000
  // and ends at 5000 — they overlap. Input is in a non-sorted-by-start order
  // so the old early-break loop missed it.
  let file = SubtitleFile::Srt(vec![
    Subtitle::new(2000, 5000, "A overlaps B"), // index 0, starts 2000
    Subtitle::new(1000, 4000, "B first"),      // index 1, starts 1000
  ]);
  let issues = file.validate();
  assert!(
    issues.iter().any(|i| matches!(i, ValidationIssue::Overlap { .. })),
    "expected an Overlap issue on unsorted input, got: {:?}",
    issues
  );
}

#[test]
fn validate_sorted_clean_input_has_no_overlap() {
  let file = SubtitleFile::Srt(vec![
    Subtitle::new(1000, 3000, "first"),
    Subtitle::new(4000, 6000, "second"),
  ]);
  let overlaps: Vec<_> = issues_of_type(&file);
  assert!(overlaps.is_empty());
}

#[test]
fn validate_sorted_overlapping_input_detected() {
  let file = SubtitleFile::Srt(vec![
    Subtitle::new(1000, 3000, "first"),
    Subtitle::new(2000, 4000, "overlaps"),
  ]);
  assert!(!issues_of_type(&file).is_empty());
}

fn issues_of_type(file: &SubtitleFile) -> Vec<&ValidationIssue> {
  file.validate()
    .iter()
    .filter(|i| matches!(i, ValidationIssue::Overlap { .. }))
    .collect()
}
```

- [ ] **Step 2: Run tests to verify the unsorted case fails**

Run: `cargo test --test cleanup_batch 2>&1 | tail -15`
Expected: `validate_detects_overlap_in_unsorted_input` FAILS (current code misses the overlap). The two sorted tests PASS.

- [ ] **Step 3: Implement the sorted-view overlap scan**

Find the overlap-detection loop in `validate` (currently lines 418-431):

```rust
    for i in 0..subs.len() {
      for j in (i + 1)..subs.len() {
        if subs[j].start < subs[i].end {
          issues.push(ValidationIssue::Overlap {
            index_a: i,
            index_b: j,
            end_a: subs[i].end,
            start_b: subs[j].start,
          });
        } else {
          break;
        }
      }
    }
```

Replace with:

```rust
    // Overlap scan is independent of input ordering: sort a copy of the indices
    // by (start, end) and compare only adjacent pairs in that order. Report the
    // *original* indices so callers see their file's actual positions.
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
```

- [ ] **Step 4: Run the integration tests to verify all pass**

Run: `cargo test --test cleanup_batch 2>&1 | tail -10`
Expected: all three PASS.

- [ ] **Step 5: Run the existing model unit tests to check for regressions**

Run: `cargo test --lib model 2>&1 | tail -10`
Expected: `test_validate_overlap`, `test_validate_decreasing_start`, `test_validate_clean` still PASS. (`test_validate_overlap` uses sorted input `[1000-3000, 2000-4000]` → still detected. `test_validate_decreasing_start` input `[3000-5000, 1000-2000]` → the subtitles do NOT overlap in time, so no Overlap; DecreasingStartTime still fires. Confirm assertion count is unchanged.)

- [ ] **Step 6: Commit**

```bash
git add src/model.rs tests/cleanup_batch.rs
git commit -m "fix: validate() detects overlaps on unsorted input"
```

---

## Task 4: Positional indices in SRT/VTT output (Correctness §2.4)

`srt::to_string` and `vtt::to_string` echo `subtitle.index`, which goes stale after edits. Emit 1-based positional ordinals instead.

**Files:**
- Modify: `src/srt.rs` (`to_string` at ~line 258)
- Modify: `src/vtt.rs` (`to_string` at ~line 275)
- Test: `tests/cleanup_batch.rs` (append)

- [ ] **Step 1: Write the failing integration test**

Append to `tests/cleanup_batch.rs`:

```rust
use subtitler::model::Subtitle;

#[test]
fn srt_output_uses_positional_indices() {
  // Stored indices are deliberately non-sequential; output must be 1, 2.
  let mut a = Subtitle::new(1000, 2000, "first");
  a.index = Some(99);
  let mut b = Subtitle::new(3000, 4000, "second");
  b.index = Some(1);
  let out = subtitler::srt::to_string(&[a, b]);
  assert!(
    out.starts_with("1\n00:00:01,000 --> 00:00:02,000\nfirst"),
    "expected positional index 1 first, got:\n{out}"
  );
  assert!(
    out.contains("\n2\n00:00:03,000 --> 00:00:04,000\nsecond"),
    "expected positional index 2 second, got:\n{out}"
  );
}

#[test]
fn vtt_output_uses_positional_indices() {
  let mut a = Subtitle::new(1000, 2000, "first");
  a.index = Some(42);
  let out = subtitler::vtt::to_string(&[a], None);
  // VTT cue identifier line should be "1", not "42".
  assert!(
    out.contains("\n1\n00:00:01.000 --> 00:00:02.000\nfirst"),
    "expected positional index 1, got:\n{out}"
  );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cleanup_batch srt_output_uses_positional 2>&1 | tail -15`
Expected: FAIL — current output starts with `99\n...`.

- [ ] **Step 3: Fix `srt::to_string`**

In `src/srt.rs`, find `to_string` (currently lines 258-282). Replace the whole function body:

```rust
pub fn to_string(subtitles: &[Subtitle]) -> String {
  let mut content = String::new();
  for (i, subtitle) in subtitles.iter().enumerate() {
    let position = i + 1;
    content.push_str(&position.to_string());
    content.push('\n');
    let timestamp = format!(
      "{} --> {}",
      format_timestamp(subtitle.start, "srt"),
      format_timestamp(subtitle.end, "srt")
    );
    content.push_str(&timestamp);
    content.push('\n');
    content.push_str(&subtitle.text);
    if i != subtitles.len() - 1 {
      content.push('\n');
      content.push('\n');
    }
  }
  if !subtitles.is_empty() {
    content.push('\n');
  }
  content
}
```

(The change: `if let Some(index) = subtitle.index { ... }` becomes unconditional `position = i + 1`.)

- [ ] **Step 4: Fix `vtt::to_string`**

In `src/vtt.rs`, find `to_string` (currently lines 275-306). Replace the body's index block. Find:

```rust
  for (i, subtitle) in subtitles.iter().enumerate() {
    if let Some(index) = subtitle.index {
      content.push_str(&index.to_string());
      content.push('\n');
    }
    let mut timestamp = format!(
```

Replace with:

```rust
  for (i, subtitle) in subtitles.iter().enumerate() {
    let position = i + 1;
    content.push_str(&position.to_string());
    content.push('\n');
    let mut timestamp = format!(
```

- [ ] **Step 5: Run the positional-index tests**

Run: `cargo test --test cleanup_batch 2>&1 | tail -10`
Expected: `srt_output_uses_positional_indices` and `vtt_output_uses_positional_indices` PASS.

- [ ] **Step 6: Run the round-trip tests to confirm no regression**

Run: `cargo test --lib srt::tests::test_round_trip 2>&1 | tail -5` and `cargo test --lib vtt::tests::test_round_trip 2>&1 | tail -5`
Expected: both PASS (round-trip fixtures use sequential indices already).

- [ ] **Step 7: Commit**

```bash
git add src/srt.rs src/vtt.rs tests/cleanup_batch.rs
git commit -m "fix: emit positional 1-based indices in SRT/VTT output"
```

---

## Task 5: Remove redundant double-scan in SRT parser (Performance §1.2)

`srt::parse` calls `extract_text_parts` on the trailing subtitle, then a post-loop re-applies it to *all* subtitles. Move extraction into the finalize path (the blank-line `take()` branch) so each subtitle is processed exactly once; remove the post-loop.

**Files:**
- Modify: `src/srt.rs` (`parse` at ~line 102)

- [ ] **Step 1: Refactor the finalize path**

In `src/srt.rs`, find the blank-line handler in `parse` (currently lines 125-131):

```rust
    if trimmed.is_empty() {
      if let Some(sub) = current_subtitle.take() {
        subtitles.push(sub);
      }
      phase = Phase::Index;
      continue;
    }
```

Replace with (extract text parts exactly once, at finalize):

```rust
    if trimmed.is_empty() {
      if let Some(mut sub) = current_subtitle.take() {
        let (plain, parts) = extract_text_parts(&sub.text);
        sub.text = plain;
        sub.text_parts = parts;
        subtitles.push(sub);
      }
      phase = Phase::Index;
      continue;
    }
```

- [ ] **Step 2: Remove the redundant post-loop**

Find the trailing + post-loop block (currently lines 192-206):

```rust
  if let Some(mut sub) = current_subtitle {
    let (plain, parts) = extract_text_parts(&sub.text);
    sub.text = plain;
    sub.text_parts = parts;
    subtitles.push(sub);
  }

  // Post-process: extract tags from all subtitles
  for sub in &mut subtitles {
    let (plain, parts) = extract_text_parts(&sub.text);
    sub.text = plain;
    sub.text_parts = parts;
  }

  Ok(subtitles)
```

Replace with (keep only the trailing finalize; the post-loop is gone because every subtitle now finalized via the blank-line path already has parts extracted):

```rust
  if let Some(mut sub) = current_subtitle {
    let (plain, parts) = extract_text_parts(&sub.text);
    sub.text = plain;
    sub.text_parts = parts;
    subtitles.push(sub);
  }

  Ok(subtitles)
```

- [ ] **Step 3: Verify SRT parser tests pass**

Run: `cargo test --lib srt:: 2>&1 | tail -10`
Expected: all SRT tests PASS — including `test_parse_bold_tag`, `test_parse_italic_tag`, `test_parse_underline_tag`, `test_parse_font_color_tag` (these assert `text_parts` are populated).

- [ ] **Step 4: Commit**

```bash
git add src/srt.rs
git commit -m "perf: extract SRT text parts once per subtitle at finalize"
```

---

## Task 6: Remove redundant double-scan in VTT parser (Performance §1.2)

Same pattern as Task 5, for `vtt::parse`. VTT already has the same trailing + post-loop structure.

**Files:**
- Modify: `src/vtt.rs` (`parse` at ~line 98)

- [ ] **Step 1: Refactor the VTT finalize path**

In `src/vtt.rs`, find the blank-line handler (currently lines 123-136):

```rust
    if trimmed.is_empty() {
      if let Some(sub) = current_subtitle.take() {
        subtitles.push(sub);
      }
      if phase == Phase::Header && !header_lines.is_empty() {
        header = Some(header_lines.join("\n"));
        header_lines.clear();
      }
      phase = match phase {
        Phase::VttComment => Phase::VttComment,
        _ => Phase::Cue,
      };
      continue;
    }
```

Replace with:

```rust
    if trimmed.is_empty() {
      if let Some(mut sub) = current_subtitle.take() {
        let (plain, parts) = extract_text_parts(&sub.text);
        sub.text = plain;
        sub.text_parts = parts;
        subtitles.push(sub);
      }
      if phase == Phase::Header && !header_lines.is_empty() {
        header = Some(header_lines.join("\n"));
        header_lines.clear();
      }
      phase = match phase {
        Phase::VttComment => Phase::VttComment,
        _ => Phase::Cue,
      };
      continue;
    }
```

- [ ] **Step 2: Remove the redundant post-loop**

Find the trailing + post-loop block (currently lines 207-225):

```rust
  if let Some(mut sub) = current_subtitle {
    let (plain, parts) = extract_text_parts(&sub.text);
    sub.text = plain;
    sub.text_parts = parts;
    subtitles.push(sub);
  }

  // Finalize header if still in header phase and has content
  if header.is_none() && !header_lines.is_empty() {
    header = Some(header_lines.join("\n"));
  }

  for sub in &mut subtitles {
    let (plain, parts) = extract_text_parts(&sub.text);
    sub.text = plain;
    sub.text_parts = parts;
  }

  Ok((header, subtitles))
```

Replace with:

```rust
  if let Some(mut sub) = current_subtitle {
    let (plain, parts) = extract_text_parts(&sub.text);
    sub.text = plain;
    sub.text_parts = parts;
    subtitles.push(sub);
  }

  // Finalize header if still in header phase and has content
  if header.is_none() && !header_lines.is_empty() {
    header = Some(header_lines.join("\n"));
  }

  Ok((header, subtitles))
```

- [ ] **Step 3: Verify VTT parser tests pass**

Run: `cargo test --lib vtt:: 2>&1 | tail -10`
Expected: all VTT tests PASS, including `test_parse_bold_tag` and `test_parse_voice_tag`.

- [ ] **Step 4: Commit**

```bash
git add src/vtt.rs
git commit -m "perf: extract VTT text parts once per subtitle at finalize"
```

---

## Task 7: Lift VTT `extract_text_parts` regex to LazyLock (Performance §1.1)

`vtt::extract_text_parts` builds its regex string and compiles on every call.

**Files:**
- Modify: `src/vtt.rs` (top-of-file static + `extract_text_parts` at ~line 21)

- [ ] **Step 1: Add the LazyLock static**

In `src/vtt.rs`, after the existing `use` block (line 10) and before `#[derive(PartialEq)] enum Phase`, add:

```rust
use std::sync::LazyLock;

static RE_VTT_TAG: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(concat!(
    r"<v(?:\s+\w+)?>|</v>|",
    r"</?(?:b|i|u|c)(?:\.[^>]*)?>"
  ))
  .unwrap()
});
```

Add `use std::sync::LazyLock;` only if not already present (it is not — SRT has it, VTT does not). The `Regex` import is already present (line 5).

- [ ] **Step 2: Replace the per-call regex build in `extract_text_parts`**

Find the regex construction in `extract_text_parts` (currently lines 30-34):

```rust
  let combined = format!(
    "{}{}",
    r"<v(?:\s+\w+)?>|</v>|", r"</?(?:b|i|u|c)(?:\.[^>]*)?>"
  );
  let re = Regex::new(&combined).unwrap();
```

Replace with:

```rust
  let re = &RE_VTT_TAG;
```

- [ ] **Step 3: Verify build + VTT tests**

Run: `cargo test --lib vtt:: 2>&1 | tail -10`
Expected: all VTT tests PASS.

- [ ] **Step 4: Commit**

```bash
git add src/vtt.rs
git commit -m "perf: lift VTT extract_text_parts regex to LazyLock"
```

---

## Task 8: Lift ASS regexes to LazyLock (Performance §1.1)

`ass::parse_ass_tags` and `ass::ass_to_plaintext` each compile a regex per call.

**Files:**
- Modify: `src/ass.rs` (statics near top + `parse_ass_tags` at ~line 249 + `ass_to_plaintext` at ~line 330)

- [ ] **Step 1: Add the LazyLock static**

In `src/ass.rs`, after the existing `static RE_INFO: ...` block (line 18) add:

```rust
static RE_ASS_TAG_INLINE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"\{([^}]*)\}").unwrap());
```

(`LazyLock` and `Regex` are already imported at the top of `ass.rs`.)

- [ ] **Step 2: Replace the regex in `parse_ass_tags`**

Find in `parse_ass_tags` (currently line 258):

```rust
  let re = Regex::new(r"\{([^}]*)\}").unwrap();
```

Replace with:

```rust
  let re = &RE_ASS_TAG_INLINE;
```

Then the existing `for caps in re.captures_iter(text)` keeps working (a `&Regex` derefs).

- [ ] **Step 3: Replace the regex in `ass_to_plaintext`**

Find in `ass_to_plaintext` (currently lines 330-338):

```rust
pub fn ass_to_plaintext(text: &str) -> String {
  let re = Regex::new(r"\{[^}]*\}").unwrap();
  let stripped = re.replace_all(text, "");
  stripped
    .replace("\\N", "\n")
    .replace("\\n", "\n")
    .replace("\\h", " ")
    .to_string()
}
```

Replace with:

```rust
pub fn ass_to_plaintext(text: &str) -> String {
  let stripped = RE_ASS_TAG_INLINE.replace_all(text, "");
  stripped
    .replace("\\N", "\n")
    .replace("\\n", "\n")
    .replace("\\h", " ")
    .to_string()
}
```

(Note: `RE_ASS_TAG_INLINE` is `\{([^}]*)\}` which has a capture group; `replace_all` with a non-template replacement string ignores captures and strips the whole match — semantically identical to the old `\{[^}]*\}`.)

- [ ] **Step 4: Verify ASS tests pass**

Run: `cargo test --lib ass:: 2>&1 | tail -10`
Expected: all ASS tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/ass.rs
git commit -m "perf: lift ASS parse_ass_tags/ass_to_plaintext regexes to LazyLock"
```

---

## Task 9: Lift `fix_ocr_errors` regexes to LazyLock (Performance §1.1)

`normalize::fix_ocr_errors` compiles 5 regexes in a loop on every call.

**Files:**
- Modify: `src/normalize.rs` (static + `fix_ocr_errors` at ~line 66)

- [ ] **Step 1: Add a LazyLock array of (regex, replacement) pairs**

At the top of `src/normalize.rs`, after the existing `RE_MUSIC_NOTE` static (line 33), add:

```rust
static RE_OCR_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
  vec![
    (Regex::new(r"\brn\b").unwrap(), "m"),
    (Regex::new(r"(\d)rn(\w)").unwrap(), "${1}m${2}"),
    (Regex::new(r"(\d)O(\d)").unwrap(), "${1}0${2}"),
    (Regex::new(r"(\d)l(\d)").unwrap(), "${1}1${2}"),
    (Regex::new(r"([a-z])0([a-z])").unwrap(), "${1}o${2}"),
  ]
});
```

- [ ] **Step 2: Rewrite `fix_ocr_errors` to use the cached array**

Find `fix_ocr_errors` (currently lines 66-81):

```rust
pub fn fix_ocr_errors(text: &str) -> String {
  let mut result = text.to_string();
  let patterns: &[(&str, &str)] = &[
    (r"\brn\b", "m"),
    (r"(\d)rn(\w)", "${1}m${2}"),
    (r"(\d)O(\d)", "${1}0${2}"),
    (r"(\d)l(\d)", "${1}1${2}"),
    (r"([a-z])0([a-z])", "${1}o${2}"),
  ];
  for (pat, rep) in patterns {
    if let Ok(re) = Regex::new(pat) {
      result = re.replace_all(&result, *rep).to_string();
    }
  }
  result
}
```

Replace with:

```rust
pub fn fix_ocr_errors(text: &str) -> String {
  let mut result = text.to_string();
  for (re, rep) in RE_OCR_PATTERNS.iter() {
    result = re.replace_all(&result, *rep).to_string();
  }
  result
}
```

- [ ] **Step 3: Verify normalize tests pass**

Run: `cargo test --lib normalize:: 2>&1 | tail -10`
Expected: `test_fix_ocr_errors` PASS (asserts `12O456`→`120456`, `1l0`→`110`, `w0rd`→`word`).

- [ ] **Step 4: Commit**

```bash
git add src/normalize.rs
git commit -m "perf: cache fix_ocr_errors regexes in LazyLock array"
```

---

## Task 10: Lift `srt::detect_format` regex to LazyLock (Performance §1.1)

**Files:**
- Modify: `src/srt.rs` (`detect_format` at ~line 237)

- [ ] **Step 1: Add the LazyLock static**

In `src/srt.rs`, after the existing `RE_SRT_TAG` static (line 14), add:

```rust
static RE_SRT_DETECT: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"^\d+\s*\n\d{2}:\d{2}:\d{2}[,.]\d{3}\s*-->").unwrap()
});
```

(`LazyLock` is already imported at line 9.)

- [ ] **Step 2: Replace the per-call regex**

Find in `detect_format` (currently lines 244-248):

```rust
      if Regex::new(r"^\d+\s*\n\d{2}:\d{2}:\d{2}[,.]\d{3}\s*-->")
        .unwrap()
        .is_match(trimmed)
      {
        return Some(crate::model::SubtitleFormat::Srt);
      }
```

Replace with:

```rust
      if RE_SRT_DETECT.is_match(trimmed) {
        return Some(crate::model::SubtitleFormat::Srt);
      }
```

- [ ] **Step 3: Verify detect tests pass**

Run: `cargo test --lib srt::tests::test_detect_format 2>&1 | tail -5`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/srt.rs
git commit -m "perf: lift srt::detect_format regex to LazyLock"
```

---

## Task 11: Add typed errors module (`error.rs`) (Architecture §3.1)

**Files:**
- Create: `src/error.rs`
- Modify: `src/lib.rs` (add module declaration)

- [ ] **Step 1: Create `src/error.rs`**

```rust
//! Typed errors for subtitle parsing.
//!
//! Existing public functions still return `AnyResult<T>` (alias for
//! `Result<T, anyhow::Error>`). This module provides structured error variants
//! for new code and gradual migration. `SubtitleError` converts into
//! `anyhow::Error` automatically via `thiserror`, so a function returning
//! `Result<_, SubtitleError>` interops with `AnyResult` callers.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SubtitleError {
  #[error("invalid timestamp format: {0}")]
  InvalidTimestamp(String),

  #[error("expected {expected} at row {row}, got: {got}")]
  UnexpectedLine {
    row: usize,
    expected: &'static str,
    got: String,
  },

  #[error("invalid UTF-8: {0}")]
  InvalidUtf8(#[from] std::string::FromUtf8Error),

  #[error("I/O error: {0}")]
  Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn invalid_timestamp_display() {
    let e = SubtitleError::InvalidTimestamp("nope".into());
    assert_eq!(e.to_string(), "invalid timestamp format: nope");
  }

  #[test]
  fn unexpected_line_display() {
    let e = SubtitleError::UnexpectedLine {
      row: 7,
      expected: "timestamp",
      got: "hello".into(),
    };
    assert_eq!(
      e.to_string(),
      "expected timestamp at row 7, got: hello"
    );
  }

  #[test]
  fn from_utf8_error_wraps() {
    let bad = vec![0xFF, 0xFE, 0xFD];
    let utf8_err = String::from_utf8(bad).unwrap_err();
    let e = SubtitleError::from(utf8_err);
    assert!(matches!(e, SubtitleError::InvalidUtf8(_)));
    assert!(e.to_string().starts_with("invalid UTF-8:"));
  }
}
```

- [ ] **Step 2: Wire the module into `lib.rs`**

In `src/lib.rs`, find line 1:

```rust
pub mod ass;
```

Add immediately after (keep alphabetical order):

```rust
pub mod error;
```

- [ ] **Step 3: Verify the new module compiles and tests pass**

Run: `cargo test --lib error:: 2>&1 | tail -10`
Expected: three tests PASS.

- [ ] **Step 4: Verify full suite still passes**

Run: `cargo test 2>&1 | tail -5`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add src/error.rs src/lib.rs
git commit -m "feat: add typed SubtitleError module (opt-in)"
```

---

## Task 12: Add ASS symmetric entry points (Architecture §3.3)

Add `parse_bytes`, async `parse_file`, and (http-gated) `parse_url` to `ass.rs` so ASS matches SRT/VTT's API surface.

**Files:**
- Modify: `src/ass.rs` (add functions after `parse_content` at ~line 160)

- [ ] **Step 1: Add `parse_bytes`**

In `src/ass.rs`, immediately after the closing brace of `parse_content` (line 160), add:

```rust
pub fn parse_bytes(data: &[u8]) -> AnyResult<SubtitleFile> {
  let text = String::from_utf8(data.to_vec())
    .map_err(|e| anyhow!("Invalid UTF-8: {}", e))?;
  parse_content(&text)
}

pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<SubtitleFile> {
  let text = tokio::fs::read_to_string(path).await?;
  parse_content(&text)
}

#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<SubtitleFile> {
  let response = reqwest::get(url).await?;
  let content = response.text().await?;
  parse_content(&content)
}
```

- [ ] **Step 2: Write tests for the new entry points**

At the end of the `#[cfg(test)] mod tests` block in `src/ass.rs`, before the closing `}`, add:

```rust
  #[test]
  fn test_parse_bytes() {
    let data = b"[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: ...\nStyle: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,Hello\n";
    let result = parse_bytes(data.as_ref()).unwrap();
    assert_eq!(result.subtitles().len(), 1);
    assert_eq!(result.subtitles()[0].text, "Hello");
  }

  #[tokio::test]
  async fn test_parse_file() {
    let content = "[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: ...\nStyle: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,FromFile\n";
    let path = "test_ass_parse_file.ass";
    std::fs::write(path, content).unwrap();
    let result = parse_file(path).await.unwrap();
    let _ = std::fs::remove_file(path);
    assert_eq!(result.subtitles()[0].text, "FromFile");
  }
```

- [ ] **Step 3: Verify the new tests pass**

Run: `cargo test --lib ass:: 2>&1 | tail -10`
Expected: `test_parse_bytes` and `test_parse_file` PASS along with existing ASS tests.

- [ ] **Step 4: Verify `--no-default-features` still compiles (http gating)**

Run: `cargo build --no-default-features 2>&1 | tail -5`
Expected: builds cleanly (the `parse_url` is `#[cfg(feature = "http")]`-gated).

- [ ] **Step 5: Commit**

```bash
git add src/ass.rs
git commit -m "feat: add ASS parse_bytes/parse_file/parse_url for parity with SRT/VTT"
```

---

## Task 13: Trim tokio features (Architecture §3.2)

**Files:**
- Modify: `Cargo.toml:38`

- [ ] **Step 1: Replace the tokio dependency line**

In `Cargo.toml`, find line 38:

```toml
tokio = {version = "^1.52.3", features = ["full"]}
```

Replace with:

```toml
tokio = {version = "^1.52.3", features = ["fs", "io-util", "rt-multi-thread", "macros"]}
```

- [ ] **Step 2: Verify default build + tests**

Run: `cargo test 2>&1 | tail -5`
Expected: all tests PASS. (`#[tokio::test]` needs `macros` + `rt`; `#[tokio::main]` multi-thread needs `rt-multi-thread`; `tokio::fs` needs `fs`; `AsyncBufReadExt`/`AsyncWriteExt`/`BufReader` need `io-util`.)

- [ ] **Step 3: Verify `--no-default-features` build**

Run: `cargo build --no-default-features 2>&1 | tail -5`
Expected: builds cleanly.

- [ ] **Step 4: If the build fails with a missing feature**, add it back.

For example, if the error mentions `tokio::sync`, add `"sync"`. If it mentions `tokio::time`, add `"time"`. Re-run Step 2. The four-feature set above was verified against actual usage (`fs`, `io-util`, `rt-multi-thread`, `macros`); add only what's actually missing.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml
git commit -m "perf: trim tokio features from full to {fs, io-util, rt-multi-thread, macros}"
```

---

## Task 14: Add a regex-hotspot benchmark group (Performance verification §4.2)

The existing benchmark file uses `criterion::{Criterion, black_box, criterion_group, criterion_main}` and registers benches via a `criterion_group!(benches, ...)` macro at line 559 followed by `criterion_main!(benches);` at line 604. The new group matches that style.

**Files:**
- Modify: `benches/subtitler_benchmark.rs`

- [ ] **Step 1: Add the hotspot benchmark function**

Insert this new function immediately before the existing `criterion_group!` block (before line 559):

```rust
fn bench_regex_hotspots(c: &mut Criterion) {
  let mut group = c.benchmark_group("regex_hotspots");

  let tagged = "<b>Bold</b> <i>italic</i> <u>under</u> <font color=\"#ff0000\">red</font> plain tail";
  let sub = subtitler::model::Subtitle::new(0, 1000, tagged);

  group.bench_function("plaintext", |b| {
    b.iter(|| {
      black_box(sub.plaintext());
    });
  });

  group.bench_function("strip_tags", |b| {
    b.iter(|| {
      let mut s = sub.clone();
      s.strip_tags();
      black_box(s.text);
    });
  });

  let noisy = "12O456 and 1l0 with w0rd plus somern";
  group.bench_function("fix_ocr_errors", |b| {
    b.iter(|| {
      black_box(subtitler::normalize::fix_ocr_errors(noisy));
    });
  });

  group.finish();
}
```

- [ ] **Step 2: Register the new function in the `criterion_group!` macro**

In the `criterion_group!( ... )` block (starting line 559), add `bench_regex_hotspots` to the argument list. Add it after the `bench_srt_to_ass_convert,` line (line 602), so the tail of the macro becomes:

```rust
  // conversion
  bench_srt_to_vtt_convert,
  bench_srt_to_ass_convert,
  // regex hotspots (perf regression tracking)
  bench_regex_hotspots,
);
```

- [ ] **Step 3: Verify the benchmark compiles**

Run: `cargo build --benches 2>&1 | tail -5`
Expected: compiles with no errors.

- [ ] **Step 4: Run the new benchmark group once to confirm it works**

Run: `cargo bench --bench subtitler_benchmark -- regex_hotspots 2>&1 | tail -15`
Expected: three benchmark results printed (`regex_hotspots/plaintext`, `regex_hotspots/strip_tags`, `regex_hotspots/fix_ocr_errors`), no errors.

- [ ] **Step 5: Commit**

```bash
git add benches/subtitler_benchmark.rs
git commit -m "bench: add regex_hotspots group for perf regression tracking"
```

---

## Task 15: Bump version + create CHANGELOG (§4.3)

**Files:**
- Modify: `Cargo.toml` (version line 19)
- Create: `CHANGELOG.md`

- [ ] **Step 1: Bump the version**

In `Cargo.toml`, find line 19:

```toml
version = "0.0.5"
```

Replace with:

```toml
version = "0.1.0"
```

- [ ] **Step 2: Create `CHANGELOG.md`**

Create `CHANGELOG.md` at the crate root:

```markdown
# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/).

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
  produced false negatives when subtitles were out of order.
- `Subtitle::chars_per_second()` now counts `plaintext()` characters
  (excluding markup) instead of raw `text`. Fixes over-counting for tagged
  subtitles; affects `validate_extended`, `auto_extend_for_cps`, and CLI
  `info` output.
- SRT and VTT `to_string` now emit 1-based positional cue indices instead of
  echoing stored (potentially stale) indices. Fixes non-sequential cue
  numbers after `merge_adjacent`, `split_long`, or `filter`.
- Updated `chardetng` calls to the 1.0 API (`EncodingDetector::new` /
  `guess`) so the crate compiles against the locked dependency.

### Added

- `error` module with a typed `SubtitleError` enum (opt-in; existing
  `AnyResult` signatures unchanged).
- `ass::parse_bytes`, `ass::parse_file` (async), and `ass::parse_url`
  (http-gated) entry points, bringing ASS to parity with SRT/VTT.
- `regex_hotspots` criterion benchmark group for regression tracking.

### Changed

- **Breaking (within 0.x):** `validate()` overlap detection, `chars_per_second`
  semantics, and SRT/VTT output indices are corrected as described under
  Fixed. Consumers relying on the prior (buggy) behavior should review.
```

- [ ] **Step 3: Verify the version bump is consistent**

Run: `cargo build 2>&1 | tail -3`
Expected: `Compiling subtitler v0.1.0` and `Finished`.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml CHANGELOG.md
git commit -m "chore: bump to 0.1.0, add CHANGELOG"
```

---

## Task 16: Update README (§4.4)

The README's API section has per-module tables. The SRT table (around line 175) and VTT table (around line 187) list `parse_file` / `parse_bytes` / `parse_url`, but the ASS table (around line 199) only lists `parse_content`. Bring ASS to parity. (Verified: the README contains no `0.0.x` version string, so no version-sweep is needed.)

**Files:**
- Modify: `README.md` (ASS Module table around line 199)

- [ ] **Step 1: Expand the ASS Module API table**

Find the ASS Module table (around lines 197-203), which currently reads:

```markdown
### ASS Module (`subtitler::ass`)

| Function | Description |
|----------|-------------|
| `parse_content(content)` | Parse ASS/SSA from string, returns `SubtitleFile` |
| `to_string(info, styles, subtitles)` | Format as ASS string |
| `detect_format(data)` | Detect if data is ASS/SSA |
```

Replace with:

```markdown
### ASS Module (`subtitler::ass`)

| Function | Description |
|----------|-------------|
| `parse_content(content)` | Parse ASS/SSA from string, returns `SubtitleFile` |
| `parse_file(path)` | Parse ASS/SSA from file (async) |
| `parse_bytes(data)` | Parse ASS/SSA from byte slice |
| `parse_url(url)` | Parse ASS/SSA from HTTP URL (requires `http` feature) |
| `to_string(info, styles, subtitles)` | Format as ASS string |
| `detect_format(data)` | Detect if data is ASS/SSA |
```

- [ ] **Step 2: Verify the edit applied cleanly**

Run: `grep -n "parse_file(path)" README.md`
Expected: at least three lines — SRT, VTT, and ASS sections each mention `parse_file(path)`.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: document ASS parse_file/parse_bytes/parse_url in README"
```

---

## Task 17: Final verification gate (§4.5)

Run the full verification suite per `AGENTS.md`. Nothing should fail.

- [ ] **Step 1: Format check (2-space indent per `rustfmt.toml`)**

Run: `cargo fmt -- --check 2>&1 | tail -10`
Expected: no output (clean). If output appears, run `cargo fmt` and re-check, then `git add -u && git commit --amend --no-edit` the formatting onto the relevant commit (or a separate `style:` commit).

- [ ] **Step 2: Clippy with `-D warnings`**

Run: `cargo clippy --all-targets -- -D warnings 2>&1 | tail -15`
Expected: no warnings. If warnings appear, fix them and re-run.

- [ ] **Step 3: Full test suite (all targets)**

Run: `cargo test --all-targets 2>&1 | tail -10`
Expected: every `test result:` line shows `ok`.

- [ ] **Step 4: No-default-features build (HTTP stripped)**

Run: `cargo build --no-default-features 2>&1 | tail -5`
Expected: `Finished`.

- [ ] **Step 5: Benchmarks still compile/run**

Run: `cargo bench --bench subtitler_benchmark --no-run 2>&1 | tail -5`
Expected: compiles cleanly.

- [ ] **Step 6: If anything in Steps 1-5 fails**, fix it, commit the fix, and re-run the failing step plus the steps after it. Do not advance past a failed gate.

- [ ] **Step 7: Final smoke test of the CLI**

Run: `cargo run -- parse examples/example.srt 2>&1 | tail -10`
Expected: subtitles printed, no panic.

Run: `cargo run -- detect examples/example.vtt 2>&1 | tail -3`
Expected: prints `vtt`.

- [ ] **Step 8: Confirm the spec's behavior changes are all in place**

Run: `cargo test --test cleanup_batch 2>&1 | tail -10`
Expected: all integration tests PASS (overlap-on-unsorted, positional indices, chars_per_second plaintext).

---

## Notes for the implementer

- **Task ordering matters.** Tasks 0-2 unblock compilation and the model-level fixes that later tests depend on. Tasks 3-4 add tests to `tests/cleanup_batch.rs` — append, don't overwrite. Tasks 5-10 are independent perf lifts but must each keep their module's tests green.
- **`rustfmt.toml` uses 2-space indent** (not Rust's default 4). Match this in all hand-written code.
- **Never `cargo add` new dependencies.** `thiserror`, `regex`, `tokio` are already declared; you're only changing features and adding modules.
- **The `#[tokio::test]` macro** requires `macros` + `rt` features — do not remove them from the tokio feature list (Task 13).
- **ASS `parse_content` stays synchronous.** Do not rewrite its internals to async (that's Approach C, explicitly rejected).
- **`SubtitleFile` enum stays `Srt | Vtt | Ass`.** Do not add MicroDvd/SubViewer variants (Approach B, deferred).
