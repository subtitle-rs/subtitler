use serde::{Deserialize, Serialize};

use smallvec::SmallVec;

use std::sync::LazyLock;

use crate::types::AnyResult;

/// Policy for writing subtitle output files.
///
/// Passed to `generate()` functions in each format module.
/// `None` defaults to `Overwrite`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WritePolicy {
  /// Overwrite the destination file if it exists (current default).
  #[default]
  Overwrite,
  /// Return an error if the destination file already exists.
  RefuseIfExists,
  /// Append to the destination file; create if missing.
  Append,
}

static RE_HTML_TAG: LazyLock<regex::Regex> =
  LazyLock::new(|| regex::Regex::new(r"</?(?:b|i|u|s|font|v|c)(?:\.[^>]*)?(?:\s[^>]*)?>").unwrap());

static RE_ASS_TAG: LazyLock<regex::Regex> =
  LazyLock::new(|| regex::Regex::new(r"\{[^}]*\}").unwrap());

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Subtitle {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub index: Option<usize>,
  pub start: u64,
  pub end: u64,
  pub text: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub settings: Option<String>,
  #[serde(skip_serializing_if = "SmallVec::is_empty", default)]
  pub text_parts: SmallVec<[TextPart; 4]>,
  // ASS/SSA fields
  #[serde(skip_serializing_if = "Option::is_none")]
  pub style: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub actor: Option<String>,
  #[serde(skip_serializing_if = "is_false", default)]
  pub is_comment: bool,
}

impl Subtitle {
  pub fn new(start: u64, end: u64, text: &str) -> Self {
    Subtitle {
      index: None,
      start,
      end,
      settings: None,
      text: text.to_string(),
      text_parts: SmallVec::new(),
      style: None,
      actor: None,
      is_comment: false,
    }
  }

  /// Builder-style: set the subtitle index (cue number).
  pub fn with_index(mut self, index: usize) -> Self {
    self.index = Some(index);
    self
  }

  /// Builder-style: set the style name (ASS/SSA).
  pub fn with_style(mut self, style: impl Into<String>) -> Self {
    self.style = Some(style.into());
    self
  }

  /// Builder-style: set the settings string (VTT).
  pub fn with_settings(mut self, settings: impl Into<String>) -> Self {
    self.settings = Some(settings.into());
    self
  }

  /// Shift both start and end by `offset_ms` milliseconds.
  ///
  /// A positive offset delays the subtitle; a negative offset advances it.
  /// Values are clamped to 0 — a large negative shift can produce `end == start == 0`
  /// which downstream `validate()` will report as negative or zero duration.
  pub fn shift(&mut self, offset_ms: i64) {
    let start = self.start as i64 + offset_ms;
    let end = self.end as i64 + offset_ms;
    self.start = start.max(0) as u64;
    self.end = end.max(0) as u64;
  }

  pub fn duration_ms(&self) -> u64 {
    self.end.saturating_sub(self.start)
  }

  pub fn chars_per_second(&self) -> f64 {
    let dur = self.duration_ms() as f64 / 1000.0;
    if dur > 0.0 {
      self.plaintext().chars().count() as f64 / dur
    } else {
      f64::INFINITY
    }
  }

  pub fn reading_speed_wpm(&self) -> f64 {
    let word_count = self.text.split_whitespace().count() as f64;
    let dur_minutes = self.duration_ms() as f64 / 60000.0;
    if dur_minutes > 0.0 {
      word_count / dur_minutes
    } else {
      f64::INFINITY
    }
  }

  /// Returns true if the subtitle text is empty or contains only whitespace.
  pub fn is_empty(&self) -> bool {
    self.text.trim().is_empty()
  }

  pub fn strip_tags(&mut self) {
    self.text = RE_HTML_TAG.replace_all(&self.text, "").to_string();
    self.text = RE_ASS_TAG.replace_all(&self.text, "").to_string();
    self.text_parts.clear();
  }

  pub fn plaintext(&self) -> String {
    let mut text = self.text.clone();
    text = RE_HTML_TAG.replace_all(&text, "").to_string();
    text = RE_ASS_TAG.replace_all(&text, "").to_string();
    text
      .replace("\\N", "\n")
      .replace("\\n", "\n")
      .replace("\\h", " ")
  }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct TextPart {
  pub text: String,
  #[serde(skip_serializing_if = "is_false", default)]
  pub bold: bool,
  #[serde(skip_serializing_if = "is_false", default)]
  pub italic: bool,
  #[serde(skip_serializing_if = "is_false", default)]
  pub underline: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub color: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub voice: Option<String>,
}

fn is_false(v: &bool) -> bool {
  !v
}

impl TextPart {
  pub fn plain(text: impl Into<String>) -> Self {
    TextPart {
      text: text.into(),
      bold: false,
      italic: false,
      underline: false,
      color: None,
      voice: None,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Timestamp {
  pub start: u64,
  pub end: u64,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub settings: Option<String>,
}

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
  #[cfg(feature = "ttml")]
  Ttml,
  #[cfg(feature = "sbv")]
  Sbv,
  #[cfg(feature = "lrc")]
  Lrc,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AssStyle {
  pub name: String,
  pub fontname: String,
  pub fontsize: u32,
  pub primary_color: String,
  pub secondary_color: String,
  pub outline_color: String,
  pub back_color: String,
  pub bold: bool,
  pub italic: bool,
  pub underline: bool,
  pub strikeout: bool,
  #[serde(default)]
  pub scale_x: f64,
  #[serde(default)]
  pub scale_y: f64,
  #[serde(default)]
  pub spacing: f64,
  #[serde(default)]
  pub angle: f64,
  #[serde(default = "default_border_style")]
  pub border_style: u32,
  #[serde(default)]
  pub outline: f64,
  #[serde(default)]
  pub shadow: f64,
  #[serde(default = "default_alignment")]
  pub alignment: u32,
  #[serde(default)]
  pub margin_l: i32,
  #[serde(default)]
  pub margin_r: i32,
  #[serde(default)]
  pub margin_v: i32,
  #[serde(default = "default_encoding")]
  pub encoding: i32,
}

fn default_border_style() -> u32 {
  1
}
fn default_alignment() -> u32 {
  2
}
fn default_encoding() -> i32 {
  1
}

impl AssStyle {
  pub fn default_style() -> Self {
    AssStyle {
      name: "Default".into(),
      fontname: "Arial".into(),
      fontsize: 48,
      primary_color: "&H00FFFFFF".into(),
      secondary_color: "&H000000FF".into(),
      outline_color: "&H00000000".into(),
      back_color: "&H00000000".into(),
      bold: false,
      italic: false,
      underline: false,
      strikeout: false,
      scale_x: 100.0,
      scale_y: 100.0,
      spacing: 0.0,
      angle: 0.0,
      border_style: 1,
      outline: 2.0,
      shadow: 2.0,
      alignment: 2,
      margin_l: 10,
      margin_r: 10,
      margin_v: 10,
      encoding: 1,
    }
  }
}

pub fn parse_ass_color(color: &str) -> (u8, u8, u8, u8) {
  let hex = color.trim_start_matches("&H").trim_start_matches("&h");
  let parsed = u32::from_str_radix(hex, 16).unwrap_or(0x00FFFFFF);
  let b = (parsed >> 16 & 0xFF) as u8;
  let g = (parsed >> 8 & 0xFF) as u8;
  let r = (parsed & 0xFF) as u8;
  let a = (parsed >> 24 & 0xFF) as u8;
  (r, g, b, a)
}

pub fn format_ass_color(r: u8, g: u8, b: u8, a: u8) -> String {
  let value = ((a as u32) << 24) | ((b as u32) << 16) | ((g as u32) << 8) | (r as u32);
  format!("&H{:08X}", value)
}

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

/// Parsed subtitle file. Each variant holds format-specific data plus a
/// common `Vec<Subtitle>`. Use `subtitles()` / `subtitles_mut()` to access
/// the shared subtitle list regardless of the source format.
///
/// Methods like `validate()`, `shift_all()`, `merge_adjacent()` etc. are
/// available through the `SubtitleFormat` trait (auto-derived for all
/// variants via default implementations).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum SubtitleFile {
  /// SubRip Text format (`.srt`). The most widely-supported subtitle format.
  #[cfg(feature = "srt")]
  Srt(Vec<Subtitle>),

  /// WebVTT format (`.vtt`). Used by HTML5 video players.
  /// `header` stores optional metadata (`WEBVTT` header + Kind/Language lines).
  #[cfg(feature = "vtt")]
  Vtt {
    #[serde(skip_serializing_if = "Option::is_none")]
    header: Option<String>,
    subtitles: Vec<Subtitle>,
  },

  /// Advanced SubStation Alpha v4+ format (`.ass`). Supports rich styling,
  /// positioning, and karaoke effects.
  #[cfg(feature = "ass")]
  Ass(AssData),

  /// SubStation Alpha v4 format (`.ssa`). Older ASS variant; shares the same
  /// data structure (`AssData`) as `Ass`, differing only in the `format()` tag.
  #[cfg(feature = "ssa")]
  Ssa(AssData),

  /// MicroDVD format (`.sub`). Frame-based timestamps. `fps` records the
  /// frame rate used for frame↔ms conversion.
  #[cfg(feature = "microdvd")]
  MicroDvd { fps: f64, subtitles: Vec<Subtitle> },

  /// SubViewer format. `header` stores the original `[INFORMATION]` block.
  #[cfg(feature = "subviewer")]
  SubViewer {
    #[serde(skip_serializing_if = "Option::is_none")]
    header: Option<String>,
    subtitles: Vec<Subtitle>,
  },

  /// TTML / IMSC 1.0/1.1 format (`.ttml`). XML-based, used in streaming
  /// (Netflix, Amazon, Hulu). `header` is reserved for future metadata support.
  #[cfg(feature = "ttml")]
  Ttml {
    #[serde(skip_serializing_if = "Option::is_none")]
    header: Option<String>,
    subtitles: Vec<Subtitle>,
  },

  /// YouTube SBV format (`.sbv`). Simple comma-separated timestamps.
  #[cfg(feature = "sbv")]
  Sbv(Vec<Subtitle>),

  /// LRC lyrics format (`.lrc`). Used for song lyric synchronization.
  #[cfg(feature = "lrc")]
  Lrc(Vec<Subtitle>),
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum ValidationIssue {
  Overlap {
    index_a: usize,
    index_b: usize,
    end_a: u64,
    start_b: u64,
  },
  NegativeDuration {
    index: usize,
    start: u64,
    end: u64,
  },
  ZeroDuration {
    index: usize,
    time: u64,
  },
  DecreasingStartTime {
    index: usize,
    prev_start: u64,
    curr_start: u64,
  },
  TooLongGap {
    index: usize,
    prev_end: u64,
    curr_start: u64,
    gap_ms: u64,
  },
  TextTooLong {
    index: usize,
    chars: usize,
    max_chars: usize,
  },
  CpsTooHigh {
    index: usize,
    cps: f64,
    max_cps: f64,
  },
}

impl ValidationIssue {
  pub fn description(&self) -> String {
    match self {
      ValidationIssue::Overlap {
        index_a,
        index_b,
        end_a,
        start_b,
      } => {
        format!(
          "subtitle {index_a} (ends at {end_a}ms) overlaps with subtitle {index_b} (starts at {start_b}ms)"
        )
      }
      ValidationIssue::NegativeDuration { index, start, end } => {
        format!("subtitle {index} has negative duration: {start}ms -> {end}ms")
      }
      ValidationIssue::ZeroDuration { index, time } => {
        format!("subtitle {index} has zero duration at {time}ms")
      }
      ValidationIssue::DecreasingStartTime {
        index,
        prev_start,
        curr_start,
      } => {
        format!(
          "subtitle {index} starts at {curr_start}ms before previous subtitle's start time {prev_start}ms"
        )
      }
      ValidationIssue::TooLongGap {
        index,
        prev_end,
        curr_start,
        gap_ms,
      } => {
        format!("subtitle {index}: {gap_ms}ms gap between {prev_end}ms and {curr_start}ms")
      }
      ValidationIssue::TextTooLong {
        index,
        chars,
        max_chars,
      } => {
        format!("subtitle {index} has {chars} characters (max recommended: {max_chars})")
      }
      ValidationIssue::CpsTooHigh {
        index,
        cps,
        max_cps,
      } => {
        format!("subtitle {index} has {cps:.1} chars/second (max recommended: {max_cps:.1})")
      }
    }
  }
}

/// Trait unifying all subtitle format operations. The four required methods
/// (`subtitles`, `subtitles_mut`, `format`, `to_string_with_format`) are
/// per-variant; the editing methods below have default implementations that
/// work through `subtitles()`/`subtitles_mut()`, so every format gets them for
/// free.
pub trait SubtitleFormat: std::fmt::Debug + Clone + Send + Sync {
  fn subtitles(&self) -> &[Subtitle];
  fn subtitles_mut(&mut self) -> &mut Vec<Subtitle>;
  fn format(&self) -> Format;
  fn to_string_with_format(&self, format: &Format) -> String;

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
        // Use move semantics instead of clone to avoid allocation
        let next_text = std::mem::take(&mut subs[i + 1].text);
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

impl SubtitleFormat for SubtitleFile {
  fn subtitles(&self) -> &[Subtitle] {
    match self {
      #[cfg(feature = "srt")]
      SubtitleFile::Srt(subs) => subs,
      #[cfg(feature = "vtt")]
      SubtitleFile::Vtt {
        subtitles: subs, ..
      } => subs,
      #[cfg(feature = "ass")]
      SubtitleFile::Ass(data) => &data.subtitles,
      #[cfg(feature = "ssa")]
      SubtitleFile::Ssa(data) => &data.subtitles,
      #[cfg(feature = "microdvd")]
      SubtitleFile::MicroDvd {
        subtitles: subs, ..
      } => subs,
      #[cfg(feature = "subviewer")]
      SubtitleFile::SubViewer {
        subtitles: subs, ..
      } => subs,
      #[cfg(feature = "ttml")]
      SubtitleFile::Ttml {
        subtitles: subs, ..
      } => subs,
      #[cfg(feature = "sbv")]
      SubtitleFile::Sbv(subs) => subs,
      #[cfg(feature = "lrc")]
      SubtitleFile::Lrc(subs) => subs,
    }
  }

  fn subtitles_mut(&mut self) -> &mut Vec<Subtitle> {
    match self {
      #[cfg(feature = "srt")]
      SubtitleFile::Srt(subs) => subs,
      #[cfg(feature = "vtt")]
      SubtitleFile::Vtt {
        subtitles: subs, ..
      } => subs,
      #[cfg(feature = "ass")]
      SubtitleFile::Ass(data) => &mut data.subtitles,
      #[cfg(feature = "ssa")]
      SubtitleFile::Ssa(data) => &mut data.subtitles,
      #[cfg(feature = "microdvd")]
      SubtitleFile::MicroDvd {
        subtitles: subs, ..
      } => subs,
      #[cfg(feature = "subviewer")]
      SubtitleFile::SubViewer {
        subtitles: subs, ..
      } => subs,
      #[cfg(feature = "ttml")]
      SubtitleFile::Ttml {
        subtitles: subs, ..
      } => subs,
      #[cfg(feature = "sbv")]
      SubtitleFile::Sbv(subs) => subs,
      #[cfg(feature = "lrc")]
      SubtitleFile::Lrc(subs) => subs,
    }
  }

  fn format(&self) -> Format {
    match self {
      #[cfg(feature = "srt")]
      SubtitleFile::Srt(_) => Format::Srt,
      #[cfg(feature = "vtt")]
      SubtitleFile::Vtt { .. } => Format::Vtt,
      #[cfg(feature = "ass")]
      SubtitleFile::Ass(_) => Format::Ass,
      #[cfg(feature = "ssa")]
      SubtitleFile::Ssa(_) => Format::Ssa,
      #[cfg(feature = "microdvd")]
      SubtitleFile::MicroDvd { .. } => Format::MicroDvd,
      #[cfg(feature = "subviewer")]
      SubtitleFile::SubViewer { .. } => Format::SubViewer,
      #[cfg(feature = "ttml")]
      SubtitleFile::Ttml { .. } => Format::Ttml,
      #[cfg(feature = "sbv")]
      SubtitleFile::Sbv(_) => Format::Sbv,
      #[cfg(feature = "lrc")]
      SubtitleFile::Lrc(_) => Format::Lrc,
    }
  }

  fn to_string_with_format(&self, format: &Format) -> String {
    let subs = self.subtitles();
    match format {
      #[cfg(feature = "srt")]
      Format::Srt => crate::srt::to_string(subs),
      #[cfg(feature = "vtt")]
      Format::Vtt => crate::vtt::to_string(subs, None),
      #[cfg(feature = "ass")]
      Format::Ass => ass_to_string_impl(self, subs),
      #[cfg(feature = "ssa")]
      Format::Ssa => ass_to_string_impl(self, subs),
      #[cfg(feature = "microdvd")]
      Format::MicroDvd => {
        let fps = match self {
          SubtitleFile::MicroDvd { fps, .. } => Some(*fps),
          _ => None,
        };
        // Emit the fps header line when the stored fps differs from the
        // default, so round-trips preserve fps instead of silently falling
        // back to 23.976 on re-parse.
        match fps {
          Some(f) if (f - crate::microdvd::DEFAULT_FPS).abs() > f64::EPSILON => {
            crate::microdvd::to_string_with_fps_header(subs, f)
          }
          _ => crate::microdvd::to_string(subs, fps),
        }
      }
      #[cfg(feature = "subviewer")]
      Format::SubViewer => {
        let header = match self {
          SubtitleFile::SubViewer { header, .. } => header.as_deref(),
          _ => None,
        };
        crate::subviewer::to_string(subs, header)
      }
      #[cfg(feature = "ttml")]
      Format::Ttml => {
        let header = match self {
          SubtitleFile::Ttml { header, .. } => header.as_deref(),
          _ => None,
        };
        crate::ttml::to_string(subs, header)
      }
      #[cfg(feature = "sbv")]
      Format::Sbv => crate::sbv::to_string(subs),
      #[cfg(feature = "lrc")]
      Format::Lrc => crate::lrc::to_string(subs),
    }
  }
}

/// Shared ASS/SSA serialization (both formats produce the same body; they
/// differ only in identity). Split out so the trait's `match format` can give
/// `Ass` and `Ssa` their own `#[cfg]`-gated arms.
#[cfg(any(feature = "ass", feature = "ssa"))]
fn ass_to_string_impl(file: &SubtitleFile, subs: &[Subtitle]) -> String {
  let (info, styles) = match file {
    #[cfg(feature = "ass")]
    SubtitleFile::Ass(data) => (data.info.clone(), data.styles.clone()),
    #[cfg(feature = "ssa")]
    SubtitleFile::Ssa(data) => (data.info.clone(), data.styles.clone()),
    #[allow(unreachable_patterns)]
    _ => (
      std::collections::HashMap::new(),
      vec![crate::model::AssStyle::default_style()],
    ),
  };
  crate::ass::to_string(&info, &styles, subs)
}

impl SubtitleFile {
  /// Append another file's subtitles after a gap. Kept as an inherent method
  /// (takes a concrete `&SubtitleFile`, not the trait) so it stays callable
  /// without trait gymnastics.
  pub fn concatenate(&mut self, other: &SubtitleFile, gap_ms: u64) {
    let own_end = self.subtitles().iter().map(|s| s.end).max().unwrap_or(0);
    let offset = own_end + gap_ms;
    let subs = self.subtitles_mut();
    for sub in other.subtitles() {
      let mut clone = sub.clone();
      clone.shift(offset as i64);
      subs.push(clone);
    }
    self.sort();
  }
}

fn split_text_chunks(text: &str, max_chars: usize) -> Vec<String> {
  let mut chunks = Vec::new();
  let words: Vec<&str> = text.split_whitespace().collect();
  let mut current = String::new();

  for word in words {
    let test = if current.is_empty() {
      word.to_string()
    } else {
      format!("{} {}", current, word)
    };

    if test.chars().count() > max_chars && !current.is_empty() {
      chunks.push(std::mem::take(&mut current));
      current.push_str(word);
    } else {
      current = test;
    }
  }

  if !current.is_empty() {
    chunks.push(current);
  }

  chunks
}

pub fn ms_to_frames(ms: u64, fps: f64) -> u64 {
  ((ms as f64) * fps / 1000.0).round() as u64
}

pub fn frames_to_ms(frames: u64, fps: f64) -> u64 {
  ((frames as f64) * 1000.0 / fps).round() as u64
}

/// Builder for constructing `SubtitleFile` with a fluent API.
///
/// # Example
///
/// ```no_run
/// use subtitler::model::{SubtitleFileBuilder, Subtitle, Format};
///
/// let file = SubtitleFileBuilder::new(Format::Srt)
///   .add_subtitle(Subtitle::new(0, 5000, "Hello"))
///   .add_subtitle(Subtitle::new(6000, 10000, "World"))
///   .build()
///   .unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct SubtitleFileBuilder {
  format: Format,
  subtitles: Vec<Subtitle>,
  fps: Option<f64>,
  header: Option<String>,
  styles: Vec<AssStyle>,
}

impl SubtitleFileBuilder {
  /// Create a new builder for the specified format.
  pub fn new(format: Format) -> Self {
    Self {
      format,
      subtitles: Vec::new(),
      fps: None,
      header: None,
      styles: Vec::new(),
    }
  }

  /// Add a subtitle to the file.
  pub fn add_subtitle(mut self, subtitle: Subtitle) -> Self {
    self.subtitles.push(subtitle);
    self
  }

  /// Add multiple subtitles to the file.
  pub fn add_subtitles(mut self, subtitles: impl IntoIterator<Item = Subtitle>) -> Self {
    self.subtitles.extend(subtitles);
    self
  }

  /// Set the frame rate (required for MicroDVD format).
  pub fn with_fps(mut self, fps: f64) -> Self {
    self.fps = Some(fps);
    self
  }

  /// Set the header (optional for VTT, SubViewer, TTML formats).
  pub fn with_header(mut self, header: impl Into<String>) -> Self {
    self.header = Some(header.into());
    self
  }

  /// Add an ASS style (for ASS/SSA formats).
  pub fn add_style(mut self, style: AssStyle) -> Self {
    self.styles.push(style);
    self
  }

  /// Add multiple ASS styles.
  pub fn add_styles(mut self, styles: impl IntoIterator<Item = AssStyle>) -> Self {
    self.styles.extend(styles);
    self
  }

  /// Build the `SubtitleFile`.
  ///
  /// Returns `None` if required fields are missing:
  /// - MicroDVD requires `fps`
  pub fn build(self) -> Option<SubtitleFile> {
    match self.format {
      #[cfg(feature = "srt")]
      Format::Srt => Some(SubtitleFile::Srt(self.subtitles)),

      #[cfg(feature = "vtt")]
      Format::Vtt => Some(SubtitleFile::Vtt {
        header: self.header,
        subtitles: self.subtitles,
      }),

      #[cfg(feature = "ass")]
      Format::Ass => Some(SubtitleFile::Ass(AssData {
        info: std::collections::HashMap::new(),
        styles: if self.styles.is_empty() {
          vec![AssStyle::default_style()]
        } else {
          self.styles
        },
        subtitles: self.subtitles,
      })),

      #[cfg(feature = "ssa")]
      Format::Ssa => Some(SubtitleFile::Ssa(AssData {
        info: std::collections::HashMap::new(),
        styles: if self.styles.is_empty() {
          vec![AssStyle::default_style()]
        } else {
          self.styles
        },
        subtitles: self.subtitles,
      })),

      #[cfg(feature = "microdvd")]
      Format::MicroDvd => {
        let fps = self.fps?;
        Some(SubtitleFile::MicroDvd {
          fps,
          subtitles: self.subtitles,
        })
      }

      #[cfg(feature = "subviewer")]
      Format::SubViewer => Some(SubtitleFile::SubViewer {
        header: self.header,
        subtitles: self.subtitles,
      }),

      #[cfg(feature = "ttml")]
      Format::Ttml => Some(SubtitleFile::Ttml {
        header: self.header,
        subtitles: self.subtitles,
      }),

      #[cfg(feature = "sbv")]
      Format::Sbv => Some(SubtitleFile::Sbv(self.subtitles)),

      #[cfg(feature = "lrc")]
      Format::Lrc => Some(SubtitleFile::Lrc(self.subtitles)),
    }
  }
}

/// Configuration options for subtitle parsing behavior.
///
/// # Example
///
/// ```no_run
/// use subtitler::model::ParseConfig;
///
/// let config = ParseConfig::new()
///   .preserve_indices(true)       // Keep original indices
///   .lenient_mode(true)           // Tolerate format errors
///   .auto_detect_encoding(true);  // Auto-detect encoding
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ParseConfig {
  /// Preserve original subtitle indices (cue numbers).
  /// Default: false (re-index from 1)
  pub preserve_indices: bool,

  /// Lenient parsing mode: tolerate certain format errors.
  /// Default: false (strict parsing)
  pub lenient_mode: bool,

  /// Auto-detect text encoding (e.g., UTF-8, Latin-1).
  /// Default: true
  pub auto_detect_encoding: bool,

  /// Maximum allowed subtitle duration in ms (0 = no limit).
  /// Default: 0
  pub max_duration_ms: u64,

  /// Minimum allowed subtitle duration in ms.
  /// Default: 0
  pub min_duration_ms: u64,
}

impl Default for ParseConfig {
  fn default() -> Self {
    Self {
      preserve_indices: false,
      lenient_mode: false,
      auto_detect_encoding: true,
      max_duration_ms: 0,
      min_duration_ms: 0,
    }
  }
}

impl ParseConfig {
  /// Create a new ParseConfig with default values.
  pub fn new() -> Self {
    Self::default()
  }

  /// Preserve original subtitle indices (cue numbers).
  pub fn preserve_indices(mut self, preserve: bool) -> Self {
    self.preserve_indices = preserve;
    self
  }

  /// Enable lenient parsing mode (tolerate format errors).
  pub fn lenient_mode(mut self, lenient: bool) -> Self {
    self.lenient_mode = lenient;
    self
  }

  /// Auto-detect text encoding.
  pub fn auto_detect_encoding(mut self, detect: bool) -> Self {
    self.auto_detect_encoding = detect;
    self
  }

  /// Set maximum allowed subtitle duration (0 = no limit).
  pub fn max_duration_ms(mut self, ms: u64) -> Self {
    self.max_duration_ms = ms;
    self
  }

  /// Set minimum allowed subtitle duration.
  pub fn min_duration_ms(mut self, ms: u64) -> Self {
    self.min_duration_ms = ms;
    self
  }
}

/// Trait for streaming subtitle parsers.
///
/// Provides a unified interface for incremental parsing of subtitle files,
/// useful for large files or memory-constrained environments.
///
/// # Example
///
/// ```no_run
/// use subtitler::model::StreamingParser;
///
/// let content = "1\n00:00:01,000 --> 00:00:03,500\nHello\n\n";
/// let mut parser = subtitler::srt::parse_stream(content);
///
/// while let Some(result) = parser.next() {
///   let subtitle = result?;
///   println!("{:?}", subtitle);
/// }
/// ```
pub trait StreamingParser: Iterator<Item = AnyResult<Subtitle>> {
  /// Parse all remaining subtitles and return as a vector.
  ///
  /// Returns an error if any subtitle fails to parse.
  fn collect_all(&mut self) -> AnyResult<Vec<Subtitle>> {
    let mut subtitles = Vec::new();
    for result in self {
      subtitles.push(result?);
    }
    Ok(subtitles)
  }

  /// Count remaining subtitles without collecting them.
  ///
  /// This consumes the iterator.
  fn count_remaining(&mut self) -> usize {
    self.filter(|r| r.is_ok()).count()
  }
}

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
    // These two subtitles do not overlap in time (1000-2000 vs 3000-5000),
    // so the only issue is the decreasing start time.
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
  fn test_transform_framerate() {
    let mut file = SubtitleFile::Srt(vec![Subtitle::new(1000, 3000, "test")]);
    file.transform_framerate(23.976, 25.0);
    let sub = &file.subtitles()[0];
    // 1000 * 25/23.976 ≈ 1043
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
    let sub = Subtitle::new(0, 2000, "Hello World"); // 11 chars / 2s = 5.5
    assert!((sub.chars_per_second() - 5.5).abs() < 0.01);
  }

  #[test]
  fn test_chars_per_second_counts_plaintext() {
    // Tags must NOT count toward cps. "<b>Hi</b>" = 2 visible chars.
    let sub = Subtitle::new(0, 1000, "<b>Hi</b>");
    assert!((sub.chars_per_second() - 2.0).abs() < 0.01);
  }
}
