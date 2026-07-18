#![cfg(not(target_arch = "wasm32"))]
use subtitler::model::{Subtitle, SubtitleFile, SubtitleFormat, ValidationIssue};

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
    issues
      .iter()
      .any(|i| matches!(i, ValidationIssue::Overlap { .. })),
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
  let overlaps: Vec<_> = overlap_issues(&file);
  assert!(overlaps.is_empty());
}

#[test]
fn validate_sorted_overlapping_input_detected() {
  let file = SubtitleFile::Srt(vec![
    Subtitle::new(1000, 3000, "first"),
    Subtitle::new(2000, 4000, "overlaps"),
  ]);
  assert!(!overlap_issues(&file).is_empty());
}

#[test]
fn validate_detects_overlap_skipped_after_break() {
  // Old code breaks at i=0,j=1 (subs[1].start=5000 >= subs[0].end=2000) and
  // skips the true (0,2) overlap. Its outer loop then reaches i=1,j=2 and
  // emits a *false-positive* (1,2) overlap — subs[1]=5000-6000 does not actually
  // overlap subs[2]=1000-1500. So the old code reported the wrong pair; the
  // sorted scan must report the true pair (0,2).
  let file = SubtitleFile::Srt(vec![
    Subtitle::new(0, 2000, "A"),    // index 0
    Subtitle::new(5000, 6000, "B"), // index 1
    Subtitle::new(1000, 1500, "C"), // index 2, overlaps A but not B
  ]);
  let overlaps = overlap_issues(&file);
  assert!(
    overlaps.iter().any(|i| matches!(
      i,
      ValidationIssue::Overlap {
        index_a: 0,
        index_b: 2,
        ..
      }
    )),
    "expected Overlap between original indices 0 and 2, got: {:?}",
    overlaps
  );
}

fn overlap_issues(file: &SubtitleFile) -> Vec<ValidationIssue> {
  file
    .validate()
    .iter()
    .filter(|i| matches!(i, ValidationIssue::Overlap { .. }))
    .cloned()
    .collect()
}

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
