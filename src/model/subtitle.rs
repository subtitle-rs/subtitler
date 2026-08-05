use serde::{Deserialize, Serialize};

use smallvec::SmallVec;

use std::sync::LazyLock;

bitflags::bitflags! {
  /// Text formatting flags for TextPart.
  ///
  /// Uses a single byte to represent bold/italic/underline, saving 2-7 bytes
  /// per TextPart compared to three separate bool fields.
  #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
  #[serde(transparent)]
  pub struct TextFormat: u8 {
    const BOLD = 0b00000001;
    const ITALIC = 0b00000010;
    const UNDERLINE = 0b00000100;
  }
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
  /// Resolved cue-level style properties (font family, size, color, …)
  #[serde(skip_serializing_if = "Option::is_none", default)]
  pub style_props: Option<StyleProps>,
  /// Cue placement (alignment band or explicit position).
  #[serde(skip_serializing_if = "Option::is_none", default)]
  pub position: Option<CuePosition>,
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
      style_props: None,
      position: None,
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

  /// Builder-style: set the resolved style properties.
  pub fn with_style_props(mut self, props: StyleProps) -> Self {
    self.style_props = Some(props);
    self
  }

  /// Builder-style: set the cue position.
  pub fn with_position(mut self, position: CuePosition) -> Self {
    self.position = Some(position);
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

  /// Strip all HTML and ASS tags from the subtitle text (in-place).
  pub fn strip_tags(&mut self) {
    self.text = RE_HTML_TAG.replace_all(&self.text, "").to_string();
    self.text = RE_ASS_TAG.replace_all(&self.text, "").to_string();
    self.text_parts.clear();
  }

  /// Get plain text without any tags or formatting.
  ///
  /// This method removes all HTML/ASS tags and converts ASS escape sequences.
  /// It's optimized to avoid unnecessary allocations when the text is already plain.
  pub fn plaintext(&self) -> String {
    // Fast path: if text is already plain (no special characters), just clone
    if !self.text.contains('<') && !self.text.contains('{') && !self.text.contains('\\') {
      return self.text.clone();
    }

    // Slow path: need to process tags and escape sequences
    let mut text = self.text.clone();
    text = RE_HTML_TAG.replace_all(&text, "").to_string();
    text = RE_ASS_TAG.replace_all(&text, "").to_string();

    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
      if c == '\\' {
        match chars.peek() {
          Some('N') | Some('n') => {
            result.push('\n');
            chars.next();
          }
          Some('h') => {
            result.push(' ');
            chars.next();
          }
          _ => result.push(c),
        }
      } else {
        result.push(c);
      }
    }
    result
  }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct TextPart {
  pub text: String,
  #[serde(default)]
  format: TextFormat,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub color: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub voice: Option<String>,
}

fn is_false(v: &bool) -> bool {
  !v
}

/// Resolved cue-level style properties, format-neutral.
///
/// Parsers carrying named styles (TTML `<style>` elements, ASS styles)
/// resolve them onto `Subtitle::style_props` so any writer can emit
/// cue-level styling without knowing the source format.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct StyleProps {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub font_family: Option<String>,
  /// Raw TTML value ("48px", "100%"); ASS fills "{fontsize}px".
  #[serde(skip_serializing_if = "Option::is_none")]
  pub font_size: Option<String>,
  /// Normalized "#RRGGBB".
  #[serde(skip_serializing_if = "Option::is_none")]
  pub color: Option<String>,
  #[serde(skip_serializing_if = "is_false", default)]
  pub bold: bool,
  #[serde(skip_serializing_if = "is_false", default)]
  pub italic: bool,
  #[serde(skip_serializing_if = "is_false", default)]
  pub underline: bool,
}

impl StyleProps {
  /// `other` wins: `Some` fields overwrite, `true` flags stick.
  pub fn merge_from(&mut self, other: &StyleProps) {
    if other.font_family.is_some() {
      self.font_family = other.font_family.clone();
    }
    if other.font_size.is_some() {
      self.font_size = other.font_size.clone();
    }
    if other.color.is_some() {
      self.color = other.color.clone();
    }
    self.bold |= other.bold;
    self.italic |= other.italic;
    self.underline |= other.underline;
  }

  pub fn is_default(&self) -> bool {
    self == &StyleProps::default()
  }
}

/// Horizontal alignment of cue text within its region, format-neutral.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HorizontalAlign {
  Left,
  #[default]
  Center,
  Right,
}

/// Vertical placement of the cue within the video frame, format-neutral.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VerticalAlign {
  Top,
  Center,
  #[default]
  Bottom,
}

/// Cue placement as a percentage of the video frame (0-100).
///
/// `x`/`y` locate the cue's anchor point (which point of the cue box is
/// anchored is given by `h_align`/`v_align`). `None` means "use the
/// alignment band" — the writer picks a region for the edge indicated by
/// `v_align` instead of explicit coordinates (ASS style alignment without
/// a `\pos` override lands here).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct CuePosition {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub x: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub y: Option<f64>,
  #[serde(default)]
  pub h_align: HorizontalAlign,
  #[serde(default)]
  pub v_align: VerticalAlign,
}

impl CuePosition {
  /// A position carrying only the default alignment (bottom-center, no
  /// coordinates) — writers may skip emitting layout for it.
  pub fn is_default(&self) -> bool {
    self == &CuePosition::default()
  }
}

impl TextPart {
  pub fn plain(text: impl Into<String>) -> Self {
    TextPart {
      text: text.into(),
      format: TextFormat::empty(),
      color: None,
      voice: None,
    }
  }

  /// Create a new TextPart with formatting flags.
  pub fn new(text: impl Into<String>, bold: bool, italic: bool, underline: bool) -> Self {
    let mut format = TextFormat::empty();
    format.set(TextFormat::BOLD, bold);
    format.set(TextFormat::ITALIC, italic);
    format.set(TextFormat::UNDERLINE, underline);
    TextPart {
      text: text.into(),
      format,
      color: None,
      voice: None,
    }
  }

  /// Returns true if bold formatting is set.
  pub fn bold(&self) -> bool {
    self.format.contains(TextFormat::BOLD)
  }

  /// Returns true if italic formatting is set.
  pub fn italic(&self) -> bool {
    self.format.contains(TextFormat::ITALIC)
  }

  /// Returns true if underline formatting is set.
  pub fn underline(&self) -> bool {
    self.format.contains(TextFormat::UNDERLINE)
  }

  /// Set bold formatting.
  pub fn set_bold(&mut self, value: bool) {
    self.format.set(TextFormat::BOLD, value);
  }

  /// Set italic formatting.
  pub fn set_italic(&mut self, value: bool) {
    self.format.set(TextFormat::ITALIC, value);
  }

  /// Set underline formatting.
  pub fn set_underline(&mut self, value: bool) {
    self.format.set(TextFormat::UNDERLINE, value);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_style_props_merge_child_wins() {
    let mut base = StyleProps {
      font_family: Some("Arial".into()),
      font_size: Some("48px".into()),
      color: Some("#FFFFFF".into()),
      bold: true,
      italic: false,
      underline: false,
    };
    let child = StyleProps {
      font_family: None,
      font_size: Some("24px".into()),
      color: None,
      bold: false,
      italic: true,
      underline: false,
    };
    base.merge_from(&child);
    assert_eq!(base.font_family.as_deref(), Some("Arial"));
    assert_eq!(base.font_size.as_deref(), Some("24px"));
    assert_eq!(base.color.as_deref(), Some("#FFFFFF"));
    assert!(base.bold);
    assert!(base.italic);
    assert!(!base.underline);
  }
}
