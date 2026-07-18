use crate::model::{SubtitleFile, SubtitleFormat};
use serde::{Deserialize, Serialize};

/// Chainable builder for subtitle file transformations.
///
/// ```ignore
/// use subtitler::pipeline::SubtitleBuilder;
///
/// let file = SubtitleBuilder::from(file)
///   .sort()
///   .shift(500)
///   .split_long(42)
///   .merge_adjacent(200)
///   .build();
/// ```
#[derive(Debug, Clone)]
pub struct SubtitleBuilder {
  file: SubtitleFile,
}

impl SubtitleBuilder {
  pub fn from(file: SubtitleFile) -> Self {
    SubtitleBuilder { file }
  }

  pub fn build(self) -> SubtitleFile {
    self.file
  }

  pub fn sort(mut self) -> Self {
    self.file.sort();
    self
  }

  pub fn shift(mut self, offset_ms: i64) -> Self {
    self.file.shift_all(offset_ms);
    self
  }

  pub fn merge_adjacent(mut self, max_gap_ms: u64) -> Self {
    self.file.merge_adjacent(max_gap_ms);
    self
  }

  pub fn split_long(mut self, max_chars: usize) -> Self {
    self.file.split_long(max_chars);
    self
  }

  pub fn transform_fps(mut self, in_fps: f64, out_fps: f64) -> Self {
    self.file.transform_framerate(in_fps, out_fps);
    self
  }

  pub fn remove_overlaps(mut self) -> Self {
    self.file.remove_overlaps();
    self
  }

  pub fn enforce_min_duration(mut self, min_ms: u64) -> Self {
    self.file.enforce_min_duration(min_ms);
    self
  }

  pub fn enforce_max_duration(mut self, max_ms: u64) -> Self {
    self.file.enforce_max_duration(max_ms);
    self
  }

  pub fn auto_extend_cps(mut self, max_cps: f64) -> Self {
    self.file.auto_extend_for_cps(max_cps);
    self
  }

  pub fn map<F: FnMut(&mut crate::model::Subtitle)>(mut self, f: F) -> Self {
    self.file = self.file.map(f);
    self
  }

  pub fn filter<F: FnMut(&crate::model::Subtitle) -> bool>(mut self, f: F) -> Self {
    self.file = self.file.filter(f);
    self
  }

  /// Remove consecutive duplicate subtitles (same `text` after trim),
  /// keeping the first occurrence's timing.
  pub fn remove_duplicates(mut self) -> Self {
    let subs = self.file.subtitles_mut();
    subs.dedup_by(|a, b| a.text.trim() == b.text.trim());
    self
  }
}

/// A single pipeline operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum PipelineOp {
  Sort,
  Shift { offset_ms: i64 },
  MergeAdjacent { max_gap_ms: u64 },
  SplitLong { max_chars: usize },
  TransformFps { in_fps: f64, out_fps: f64 },
  RemoveOverlaps,
  EnforceMinDuration { min_ms: u64 },
  EnforceMaxDuration { max_ms: u64 },
  AutoExtendCps { max_cps: f64 },
  FilterEmpty,
  RemoveDuplicates,
}

/// A declarative pipeline of subtitle transformation operations.
///
/// Can be serialized to/from JSON for config-file based processing.
///
/// ```ignore
/// use subtitler::pipeline::{Pipeline, PipelineOp};
///
/// let pipeline = Pipeline::new()
///   .sort()
///   .shift(500)
///   .split_long(42);
///
/// let result = pipeline.apply(file)?;
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Pipeline {
  #[serde(default)]
  pub operations: Vec<PipelineOp>,
}

impl Pipeline {
  pub fn new() -> Self {
    Pipeline {
      operations: Vec::new(),
    }
  }

  pub fn push(mut self, op: PipelineOp) -> Self {
    self.operations.push(op);
    self
  }

  pub fn sort(mut self) -> Self {
    self.operations.push(PipelineOp::Sort);
    self
  }

  pub fn shift(mut self, offset_ms: i64) -> Self {
    self.operations.push(PipelineOp::Shift { offset_ms });
    self
  }

  pub fn merge_adjacent(mut self, max_gap_ms: u64) -> Self {
    self
      .operations
      .push(PipelineOp::MergeAdjacent { max_gap_ms });
    self
  }

  pub fn split_long(mut self, max_chars: usize) -> Self {
    self.operations.push(PipelineOp::SplitLong { max_chars });
    self
  }

  pub fn transform_fps(mut self, in_fps: f64, out_fps: f64) -> Self {
    self
      .operations
      .push(PipelineOp::TransformFps { in_fps, out_fps });
    self
  }

  pub fn remove_overlaps(mut self) -> Self {
    self.operations.push(PipelineOp::RemoveOverlaps);
    self
  }

  pub fn enforce_min_duration(mut self, min_ms: u64) -> Self {
    self
      .operations
      .push(PipelineOp::EnforceMinDuration { min_ms });
    self
  }

  pub fn enforce_max_duration(mut self, max_ms: u64) -> Self {
    self
      .operations
      .push(PipelineOp::EnforceMaxDuration { max_ms });
    self
  }

  pub fn auto_extend_cps(mut self, max_cps: f64) -> Self {
    self.operations.push(PipelineOp::AutoExtendCps { max_cps });
    self
  }

  pub fn filter_empty(mut self) -> Self {
    self.operations.push(PipelineOp::FilterEmpty);
    self
  }

  pub fn remove_duplicates(mut self) -> Self {
    self.operations.push(PipelineOp::RemoveDuplicates);
    self
  }

  pub fn apply(&self, file: SubtitleFile) -> SubtitleFile {
    let mut builder = SubtitleBuilder::from(file);
    for op in &self.operations {
      builder = match op {
        PipelineOp::Sort => builder.sort(),
        PipelineOp::Shift { offset_ms } => builder.shift(*offset_ms),
        PipelineOp::MergeAdjacent { max_gap_ms } => builder.merge_adjacent(*max_gap_ms),
        PipelineOp::SplitLong { max_chars } => builder.split_long(*max_chars),
        PipelineOp::TransformFps { in_fps, out_fps } => builder.transform_fps(*in_fps, *out_fps),
        PipelineOp::RemoveOverlaps => builder.remove_overlaps(),
        PipelineOp::EnforceMinDuration { min_ms } => builder.enforce_min_duration(*min_ms),
        PipelineOp::EnforceMaxDuration { max_ms } => builder.enforce_max_duration(*max_ms),
        PipelineOp::AutoExtendCps { max_cps } => builder.auto_extend_cps(*max_cps),
        PipelineOp::FilterEmpty => {
          builder = builder.filter(|sub| !sub.text.trim().is_empty());
          builder
        }
        PipelineOp::RemoveDuplicates => builder.remove_duplicates(),
      };
    }
    builder.build()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::{Subtitle, SubtitleFile};

  fn make_sub(start: u64, end: u64, text: &str) -> Subtitle {
    Subtitle::new(start, end, text)
  }

  fn test_file() -> SubtitleFile {
    SubtitleFile::Srt(vec![
      make_sub(5000, 7000, "third"),
      make_sub(1000, 3000, "first"),
      make_sub(3000, 5000, "second"),
    ])
  }

  #[test]
  fn test_builder_sort() {
    let file = SubtitleBuilder::from(test_file()).sort().build();
    let subs = file.subtitles();
    assert_eq!(subs[0].start, 1000);
    assert_eq!(subs[1].start, 3000);
    assert_eq!(subs[2].start, 5000);
  }

  #[test]
  fn test_builder_shift() {
    let file = SubtitleBuilder::from(test_file()).sort().shift(500).build();
    let subs = file.subtitles();
    assert_eq!(subs[0].start, 1500);
    assert_eq!(subs[1].start, 3500);
    assert_eq!(subs[2].start, 5500);
  }

  #[test]
  fn test_builder_chain() {
    let file = SubtitleBuilder::from(test_file())
      .sort()
      .shift(100)
      .merge_adjacent(5000)
      .build();
    let subs = file.subtitles();
    assert_eq!(subs.len(), 1);
    assert!(subs[0].text.contains("first"));
    assert!(subs[0].text.contains("second"));
    assert!(subs[0].text.contains("third"));
  }

  #[test]
  fn test_builder_filter() {
    let file = SubtitleFile::Srt(vec![
      make_sub(1000, 2000, "keep"),
      make_sub(3000, 4000, ""),
      make_sub(5000, 6000, "also keep"),
    ]);
    let file = SubtitleBuilder::from(file)
      .filter(|s| !s.text.is_empty())
      .build();
    assert_eq!(file.subtitles().len(), 2);
  }

  #[test]
  fn test_pipeline_sort_shift() {
    let pipeline = Pipeline::new().sort().shift(500);
    let result = pipeline.apply(test_file());
    let subs = result.subtitles();
    assert_eq!(subs[0].start, 1500);
    assert_eq!(subs[0].text, "first");
    assert_eq!(subs[1].start, 3500);
    assert_eq!(subs[1].text, "second");
  }

  #[test]
  fn test_pipeline_serialize() {
    let pipeline = Pipeline::new()
      .sort()
      .shift(500)
      .split_long(42)
      .merge_adjacent(200);

    let json = serde_json::to_string_pretty(&pipeline).unwrap();
    assert!(json.contains("Sort"));
    assert!(json.contains("500"));
    assert!(json.contains("42"));

    let parsed: Pipeline = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.operations.len(), 4);

    let result = parsed.apply(test_file());
    let subs = result.subtitles();
    assert_eq!(subs[0].start, 1500);
  }

  #[test]
  fn test_pipeline_filter_empty() {
    let file = SubtitleFile::Srt(vec![
      make_sub(1000, 2000, "keep"),
      make_sub(3000, 4000, "  "),
      make_sub(5000, 6000, "also"),
    ]);
    let pipeline = Pipeline::new().filter_empty().sort();
    let result = pipeline.apply(file);
    assert_eq!(result.subtitles().len(), 2);
  }

  #[test]
  fn test_pipeline_apply_idempotent() {
    let pipeline = Pipeline::new();
    let result = pipeline.apply(test_file());
    assert_eq!(result.subtitles().len(), 3);
  }

  #[test]
  fn test_remove_duplicates_consecutive() {
    let file = SubtitleFile::Srt(vec![
      make_sub(1000, 2000, "hello"),
      make_sub(3000, 4000, "hello"), // dup text, consecutive → removed
      make_sub(5000, 6000, "world"),
      make_sub(7000, 8000, "hello"), // same text but NOT consecutive → kept
    ]);
    let result = SubtitleBuilder::from(file).remove_duplicates().build();
    let subs = result.subtitles();
    assert_eq!(subs.len(), 3);
    assert_eq!(subs[0].text, "hello");
    assert_eq!(subs[1].text, "world");
    assert_eq!(subs[2].text, "hello"); // non-consecutive dup preserved
  }

  #[test]
  fn test_remove_duplicates_via_pipeline() {
    let pipeline = Pipeline::new().remove_duplicates();
    let file = SubtitleFile::Srt(vec![
      make_sub(1000, 2000, "dup"),
      make_sub(3000, 4000, "dup"),
      make_sub(5000, 6000, "unique"),
    ]);
    let result = pipeline.apply(file);
    assert_eq!(result.subtitles().len(), 2);
  }

  #[test]
  fn test_remove_duplicates_serialize_round_trip() {
    let pipeline = Pipeline::new().remove_duplicates();
    let json = serde_json::to_string(&pipeline).unwrap();
    assert!(json.contains("RemoveDuplicates"));
    let parsed: Pipeline = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.operations.len(), 1);
  }
}
