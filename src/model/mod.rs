pub mod builder;
pub mod convert;
pub mod format;
pub mod streaming;
pub mod subtitle;
pub mod types;
pub mod validation;

mod r#trait;

pub use builder::{ParseConfig, SubtitleFileBuilder};
pub use convert::{
  format_ass_color, frames_to_ms, ms_to_frames, parse_ass_color, split_text_chunks,
};
pub use format::{Format, SubtitleFile};
pub use streaming::StreamingParser;
pub use subtitle::{Subtitle, TextFormat, TextPart};
pub use r#trait::SubtitleFormat;
pub use types::{AssData, AssStyle, Timestamp, WritePolicy};
pub use validation::ValidationIssue;

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_sort() {
    let mut file = SubtitleFile::Srt(vec![
      Subtitle::new(5000, 7000, "third"),
      Subtitle::new(1000, 3000, "first"),
      Subtitle::new(3000, 5000, "second"),
    ]);
    file.sort();
    let subs = file.subtitles();
    assert_eq!(subs[0].start, 1000);
    assert_eq!(subs[1].start, 3000);
    assert_eq!(subs[2].start, 5000);
  }

  #[test]
  fn test_validate_overlap() {
    let file = SubtitleFile::Srt(vec![
      Subtitle::new(1000, 3000, "first"),
      Subtitle::new(2000, 4000, "overlaps"),
    ]);
    let issues = file.validate();
    assert_eq!(issues.len(), 1);
    assert!(matches!(issues[0], ValidationIssue::Overlap { .. }));
  }

  #[test]
  fn test_validate_negative_duration() {
    let file = SubtitleFile::Srt(vec![Subtitle::new(3000, 1000, "bad")]);
    let issues = file.validate();
    assert_eq!(issues.len(), 1);
    assert!(matches!(
      issues[0],
      ValidationIssue::NegativeDuration { .. }
    ));
  }

  #[test]
  fn test_validate_zero_duration() {
    let file = SubtitleFile::Srt(vec![Subtitle::new(1000, 1000, "instant")]);
    let issues = file.validate();
    assert_eq!(issues.len(), 1);
    assert!(matches!(issues[0], ValidationIssue::ZeroDuration { .. }));
  }

  #[test]
  fn test_validate_decreasing_start() {
    let file = SubtitleFile::Srt(vec![
      Subtitle::new(3000, 5000, "second"),
      Subtitle::new(1000, 2000, "first"),
    ]);
    let issues = file.validate();
    assert_eq!(issues.len(), 1);
    assert!(
      issues
        .iter()
        .any(|i| matches!(i, ValidationIssue::DecreasingStartTime { .. }))
    );
  }

  #[test]
  fn test_validate_clean() {
    let file = SubtitleFile::Srt(vec![
      Subtitle::new(1000, 3000, "first"),
      Subtitle::new(4000, 6000, "second"),
    ]);
    assert!(file.validate().is_empty());
  }

  #[test]
  fn test_validate_extended() {
    let file = SubtitleFile::Srt(vec![
      Subtitle::new(1000, 3000, "first"),
      Subtitle::new(10000, 12000, "second with a very large gap"),
    ]);
    let issues = file.validate_extended(50, 5000, 30.0);
    assert!(
      issues
        .iter()
        .any(|i| matches!(i, ValidationIssue::TooLongGap { .. }))
    );
  }

  #[test]
  fn test_merge_adjacent() {
    let mut file = SubtitleFile::Srt(vec![
      Subtitle::new(1000, 3000, "first"),
      Subtitle::new(3100, 5000, "second"),
      Subtitle::new(7000, 9000, "third"),
    ]);
    file.merge_adjacent(500);
    let subs = file.subtitles();
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0].text, "first\nsecond");
    assert_eq!(subs[0].end, 5000);
    assert_eq!(subs[1].text, "third");
  }

  #[test]
  fn test_merge_adjacent_noop() {
    let mut file = SubtitleFile::Srt(vec![
      Subtitle::new(1000, 3000, "first"),
      Subtitle::new(5000, 7000, "second"),
    ]);
    file.merge_adjacent(100);
    assert_eq!(file.subtitles().len(), 2);
  }

  #[test]
  fn test_split_long() {
    let mut file = SubtitleFile::Srt(vec![Subtitle::new(
      1000,
      5000,
      "this is a very long subtitle that should be split into multiple parts",
    )]);
    file.split_long(20);
    let subs = file.subtitles();
    assert!(subs.len() > 1);
    assert!(subs[0].text.len() <= 20 || subs[0].text.chars().count() <= 20);
  }

  #[test]
  fn test_split_long_short() {
    let mut file = SubtitleFile::Srt(vec![Subtitle::new(1000, 3000, "short")]);
    file.split_long(20);
    assert_eq!(file.subtitles().len(), 1);
  }

  #[test]
  fn test_split_long_no_zero_duration() {
    // 8 single-char words split at max_chars=1 → 8 chunks.
    // duration = 1003 - 1000 = 3ms; integer division 3/8 = 0 per chunk.
    // Before the fix: middle chunks got start == end (zero duration),
    // which validate() reports as ZeroDuration.
    let mut file = SubtitleFile::Srt(vec![Subtitle::new(1000, 1003, "a b c d e f g h")]);
    file.split_long(1);
    let subs = file.subtitles();
    // Sanity: the split actually produced multiple chunks
    assert!(
      subs.len() > 1,
      "split_long did not split: {} subs",
      subs.len()
    );
    // Every produced subtitle must have positive duration
    for (i, sub) in subs.iter().enumerate() {
      assert!(
        sub.duration_ms() > 0,
        "subtitle {} has zero duration (start={}, end={})",
        i,
        sub.start,
        sub.end
      );
    }
    // And validate() must not flag any ZeroDuration
    let issues = file.validate();
    assert!(
      !issues
        .iter()
        .any(|i| matches!(i, crate::model::ValidationIssue::ZeroDuration { .. })),
      "validate() flagged zero-duration after split_long: {:?}",
      issues
    );
  }

  #[test]
  fn test_transform_framerate() {
    let mut file = SubtitleFile::Srt(vec![Subtitle::new(1000, 3000, "test")]);
    file.transform_framerate(23.976, 25.0);
    let sub = &file.subtitles()[0];
    assert!(sub.start >= 1040 && sub.start <= 1045);
  }

  #[test]
  fn test_ms_to_frames() {
    assert_eq!(ms_to_frames(1000, 25.0), 25);
    assert_eq!(ms_to_frames(0, 25.0), 0);
  }

  #[test]
  fn test_frames_to_ms() {
    assert_eq!(frames_to_ms(25, 25.0), 1000);
    assert_eq!(frames_to_ms(0, 25.0), 0);
  }

  #[test]
  fn test_chars_per_second() {
    let sub = Subtitle::new(0, 2000, "Hello World");
    assert!((sub.chars_per_second() - 5.5).abs() < 0.01);
  }

  #[test]
  fn test_chars_per_second_counts_plaintext() {
    let sub = Subtitle::new(0, 1000, "<b>Hi</b>");
    assert!((sub.chars_per_second() - 2.0).abs() < 0.01);
  }
}
