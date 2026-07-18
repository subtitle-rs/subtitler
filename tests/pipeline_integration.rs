//! Pipeline and Builder integration tests — verify the chaining API works
//! correctly across multiple operations and edge cases.

use subtitler::model::{Subtitle, SubtitleFile, SubtitleFormat};
use subtitler::pipeline::{Pipeline, SubtitleBuilder};

fn make_sub(start: u64, end: u64, text: &str) -> Subtitle {
  Subtitle::new(start, end, text)
}

fn test_file_sorted() -> SubtitleFile {
  SubtitleFile::Srt(vec![
    make_sub(1000, 3000, "first"),
    make_sub(4000, 6000, "second"),
    make_sub(7000, 9000, "third"),
  ])
}

fn test_file_unsorted() -> SubtitleFile {
  SubtitleFile::Srt(vec![
    make_sub(7000, 9000, "third"),
    make_sub(1000, 3000, "first"),
    make_sub(4000, 6000, "second"),
  ])
}

// ── Builder tests ──

#[test]
fn builder_sort_unsorted_file() {
  let file = SubtitleBuilder::from(test_file_unsorted()).sort().build();
  let subs = file.subtitles();
  assert_eq!(subs[0].start, 1000);
  assert_eq!(subs[1].start, 4000);
  assert_eq!(subs[2].start, 7000);
}

#[test]
fn builder_chain_multiple_operations() {
  let file = SubtitleBuilder::from(test_file_unsorted())
    .sort()
    .shift(1000)
    .enforce_min_duration(2000)
    .build();

  let subs = file.subtitles();
  assert_eq!(subs[0].start, 2000); // 1000 + 1000 shift
  assert_eq!(subs[1].start, 5000); // 4000 + 1000 shift
  assert_eq!(subs[2].start, 8000); // 7000 + 1000 shift

  // each subtitle should have at least 2000ms duration
  for sub in subs {
    assert!(sub.duration_ms() >= 2000);
  }
}

#[test]
fn builder_shift_negative_clamps_to_zero() {
  let file = SubtitleBuilder::from(test_file_sorted())
    .shift(-1500)
    .build();
  let subs = file.subtitles();
  assert_eq!(subs[0].start, 0);
  assert_eq!(subs[0].end, 1500); // 3000 - 1500
}

#[test]
fn builder_merge_long_gap_noop() {
  let file = SubtitleFile::Srt(vec![
    make_sub(1000, 2000, "a"),
    make_sub(5000, 6000, "b"), // 3000ms gap
  ]);
  let file = SubtitleBuilder::from(file).merge_adjacent(500).build();
  assert_eq!(file.subtitles().len(), 2);
}

#[test]
fn builder_merge_small_gap_joins() {
  let file = SubtitleFile::Srt(vec![
    make_sub(1000, 2000, "hello"),
    make_sub(2100, 4000, "world"), // 100ms gap
  ]);
  let file = SubtitleBuilder::from(file).merge_adjacent(500).build();
  assert_eq!(file.subtitles().len(), 1);
  assert_eq!(file.subtitles()[0].text, "hello\nworld");
  assert_eq!(file.subtitles()[0].start, 1000);
  assert_eq!(file.subtitles()[0].end, 4000);
}

#[test]
fn builder_split_long_preserves_order() {
  let mut file = SubtitleFile::Srt(vec![make_sub(
    1000,
    5000,
    "one two three four five six seven eight nine ten",
  )]);
  file = SubtitleBuilder::from(file).split_long(20).sort().build();
  let subs = file.subtitles();
  assert!(subs.len() >= 2);
  // verify ascending time order
  for w in subs.windows(2) {
    assert!(w[0].start <= w[1].start);
  }
}

#[test]
fn builder_filter_removes_empty() {
  let file = SubtitleFile::Srt(vec![
    make_sub(1000, 2000, "keep"),
    make_sub(3000, 4000, ""),
    make_sub(5000, 6000, "also keep"),
  ]);
  let file = SubtitleBuilder::from(file)
    .filter(|s| !s.text.trim().is_empty())
    .build();
  assert_eq!(file.subtitles().len(), 2);
  assert_eq!(file.subtitles()[0].text, "keep");
  assert_eq!(file.subtitles()[1].text, "also keep");
}

#[test]
fn builder_map_uppercase() {
  let file = SubtitleBuilder::from(test_file_sorted())
    .map(|s| {
      s.text = s.text.to_uppercase();
    })
    .build();
  assert_eq!(file.subtitles()[0].text, "FIRST");
  assert_eq!(file.subtitles()[1].text, "SECOND");
}

#[test]
fn builder_overlaps_fix() {
  let file = SubtitleFile::Srt(vec![
    make_sub(1000, 4000, "overlapper"),
    make_sub(3000, 5000, "overlappee"),
  ]);
  let file = SubtitleBuilder::from(file).sort().remove_overlaps().build();
  let subs = file.subtitles();
  assert_eq!(subs[0].end, subs[1].start);
}

#[test]
fn builder_extend_for_cps_short_duration() {
  let file = SubtitleFile::Srt(vec![make_sub(
    0,
    500,
    "This subtitle has way too many characters for just 500ms",
  )]);
  let file = SubtitleBuilder::from(file).auto_extend_cps(20.0).build();
  assert!(
    file.subtitles()[0].end > 2000,
    "should extend duration for readability"
  );
}

// ── Pipeline tests ──

#[test]
fn pipeline_applies_all_ops() {
  let pipeline = Pipeline::new()
    .sort()
    .shift(500)
    .merge_adjacent(10000)
    .remove_overlaps();

  let result = pipeline.apply(test_file_unsorted());
  let subs = result.subtitles();
  assert!(!subs.is_empty());
  assert_eq!(subs[0].start, 1500); // 1000 + 500
}

#[test]
fn pipeline_idempotent_with_no_ops() {
  let pipeline = Pipeline::new();
  let result = pipeline.apply(test_file_sorted());
  assert_eq!(result.subtitles().len(), 3);
}

#[test]
fn pipeline_filter_empty_removes_blanks() {
  let file = SubtitleFile::Srt(vec![
    make_sub(1000, 2000, "keep"),
    make_sub(3000, 4000, "  "),
  ]);
  let pipeline = Pipeline::new().filter_empty();
  let result = pipeline.apply(file);
  assert_eq!(result.subtitles().len(), 1);
}

#[test]
fn pipeline_serialize_deserialize_round_trip() {
  let pipeline = Pipeline::new().sort().shift(500).split_long(42);

  let json = serde_json::to_string_pretty(&pipeline).unwrap();
  let parsed: Pipeline = serde_json::from_str(&json).unwrap();
  assert_eq!(parsed.operations.len(), 3);

  // Verify it still works
  let result = parsed.apply(test_file_unsorted());
  let subs = result.subtitles();
  assert_eq!(subs[0].start, 1500); // 1000 + 500
}

#[test]
fn pipeline_operations_are_ordered() {
  let pipeline = Pipeline::new().shift(1000).sort().shift(500);

  let file = SubtitleFile::Srt(vec![make_sub(5000, 7000, "b"), make_sub(1000, 3000, "a")]);

  // ops order: shift(+1000) → sort → shift(+500)
  // after first shift: a=2000, b=6000
  // after sort: already sorted
  // after second shift: a=2500, b=6500
  let result = pipeline.apply(file);
  let subs = result.subtitles();
  assert_eq!(subs[0].start, 2500);
  assert_eq!(subs[1].start, 6500);
}

#[test]
fn pipeline_enforce_duration_chain() {
  let file = SubtitleFile::Srt(vec![
    make_sub(1000, 1200, "too short"), // 200ms
  ]);
  let pipeline = Pipeline::new()
    .enforce_min_duration(1000)
    .enforce_max_duration(5000);
  let result = pipeline.apply(file);
  let dur = result.subtitles()[0].duration_ms();
  assert!((1000..=5000).contains(&dur), "duration should be clamped");
}
