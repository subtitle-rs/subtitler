use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Timestamp {
  pub start: u64,
  pub end: u64,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub settings: Option<String>,
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

/// Shared ASS/SSA structure. Used by both the `Ass` (v4+) and `Ssa` (v4)
/// variants of `SubtitleFile`, which differ only in their `format()` tag.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AssData {
  #[serde(skip_serializing_if = "std::collections::HashMap::is_empty", default)]
  pub info: std::collections::HashMap<String, String>,
  #[serde(skip_serializing_if = "Vec::is_empty", default)]
  pub styles: Vec<AssStyle>,
  pub subtitles: Vec<super::subtitle::Subtitle>,
}
