use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Subtitle {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub index: Option<usize>,
  pub start: u64,
  pub end: u64,
  pub text: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub settings: Option<String>,
  #[serde(skip_serializing_if = "Vec::is_empty", default)]
  pub text_parts: Vec<TextPart>,
}

impl Subtitle {
  pub fn new(start: u64, end: u64, text: &str) -> Self {
    Subtitle {
      index: None,
      start,
      end,
      settings: None,
      text: text.to_string(),
      text_parts: Vec::new(),
    }
  }

  pub fn shift(&mut self, offset_ms: i64) {
    let start = self.start as i64 + offset_ms;
    let end = self.end as i64 + offset_ms;
    self.start = start.max(0) as u64;
    self.end = end.max(0) as u64;
  }

  pub fn duration_ms(&self) -> u64 {
    self.end.saturating_sub(self.start)
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
pub enum SubtitleFormat {
  Srt,
  Vtt,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum SubtitleFile {
  Srt(Vec<Subtitle>),
  Vtt(Vec<Subtitle>),
}

impl SubtitleFile {
  pub fn subtitles(&self) -> &[Subtitle] {
    match self {
      SubtitleFile::Srt(subs) => subs,
      SubtitleFile::Vtt(subs) => subs,
    }
  }

  pub fn subtitles_mut(&mut self) -> &mut Vec<Subtitle> {
    match self {
      SubtitleFile::Srt(subs) => subs,
      SubtitleFile::Vtt(subs) => subs,
    }
  }

  pub fn format(&self) -> SubtitleFormat {
    match self {
      SubtitleFile::Srt(_) => SubtitleFormat::Srt,
      SubtitleFile::Vtt(_) => SubtitleFormat::Vtt,
    }
  }

  pub fn shift_all(&mut self, offset_ms: i64) {
    for sub in self.subtitles_mut().iter_mut() {
      sub.shift(offset_ms);
    }
  }
}
