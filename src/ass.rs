use crate::error::SubtitleError;
use crate::model::convert::{MS_PER_HOUR, MS_PER_MINUTE, MS_PER_SECOND};
use crate::model::{
  AssData, AssFont, AssStyle, CuePosition, Format, HorizontalAlign, StyleProps, Subtitle,
  SubtitleFile, VerticalAlign,
};
use crate::types::AnyResult;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;
#[cfg(not(target_arch = "wasm32"))]
use tokio::io::AsyncWriteExt;

static RE_DIALOGUE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"^(?:Dialogue|Comment):\s*(?:\d+,)?(\d+):(\d+):(\d+)[,.](\d+),(\d+):(\d+):(\d+)[,.](\d+),(?:([^,]*),)?(?:([^,]*),)?(?:(-?\d+),)?(?:(-?\d+),)?(?:(-?\d+),)?(?:([^,]*),)?(?:(\{.*\})?,)?(.+)$").unwrap()
});

static RE_STYLE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"^Style:\s*([^,]*),([^,]*),(\d+),([^,]*),([^,]*),([^,]*),([^,]*),(-?\d+),(-?\d+),(-?\d+),(-?\d+),(-?[\d.]+),(-?[\d.]+),(-?[\d.]+),(-?[\d.]+),(\d+),(-?[\d.]+),(-?[\d.]+),(\d+),(\d+),(\d+),(\d+),(\d+)").unwrap()
});

static RE_INFO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([^:]+):\s*(.*)").unwrap());

static RE_ASS_TAG_INLINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{([^}]*)\}").unwrap());

/// SubStation Alpha's uuencode-like binary encoding
mod uuencode {
  const CHARS_PER_LINE: usize = 80;

  fn decode_char(b: u8) -> Option<u8> {
    b.checked_sub(33).filter(|&v| v < 64)
  }

  pub fn decode(lines: &[String]) -> Option<Vec<u8>> {
    let mut data = Vec::new();
    let mut src = [0u8; 4];
    let mut len = 0;
    for line in lines {
      for &b in line.as_bytes() {
        if b == 0 || b == b'\n' || b == b'\r' {
          continue;
        }
        src[len] = decode_char(b)?;
        len += 1;
        if len == 4 {
          data.push((src[0] << 2) | (src[1] >> 4));
          data.push(((src[1] & 0x0F) << 4) | (src[2] >> 2));
          data.push(((src[2] & 0x03) << 6) | src[3]);
          len = 0;
        }
      }
    }
    if len > 1 {
      data.push((src[0] << 2) | (src[1] >> 4));
    }
    if len > 2 {
      data.push(((src[1] & 0x0F) << 4) | (src[2] >> 2));
    }
    // len == 1 is stray padding; no byte is recoverable.
    Some(data)
  }

  pub fn encode(data: &[u8]) -> String {
    let mut out = String::new();
    let mut written = 0usize;
    for pos in (0..data.len()).step_by(3) {
      let rem = data.len() - pos;
      let b0 = data[pos];
      let b1 = data.get(pos + 1).copied().unwrap_or(0);
      let b2 = data.get(pos + 2).copied().unwrap_or(0);
      let dst = [
        b0 >> 2,
        ((b0 & 0x03) << 4) | (b1 >> 4),
        ((b1 & 0x0F) << 2) | (b2 >> 6),
        b2 & 0x3F,
      ];
      for &v in dst.iter().take((rem + 1).min(4)) {
        out.push((v + 33) as char);
        written += 1;
        if written == CHARS_PER_LINE && pos + 3 < data.len() {
          written = 0;
          out.push('\n');
        }
      }
    }
    out
  }

  #[cfg(test)]
  mod tests {
    use super::*;

    #[test]
    fn test_decode_known_vector() {
      let lines = vec!["!\"#A".to_string(), "BC".to_string()];
      assert_eq!(decode(&lines).unwrap(), vec![0, 16, 160, 134]);
    }

    #[test]
    fn test_decode_ignores_cr_lf_and_nul() {
      // Aegisub-style "\r\n" line breaks and NUL padding are skipped.
      let lines = vec!["!\"#\r".to_string(), "\nA\x00".to_string()];
      assert_eq!(decode(&lines).unwrap(), vec![0, 16, 160]);
    }

    #[test]
    fn test_round_trip_two_byte_tail_uses_three_chars() {
      let data = vec![0xAB, 0xCD];
      let lines: Vec<String> = encode(&data).lines().map(str::to_string).collect();
      assert_eq!(lines, vec!["K]U".to_string()]);
      assert_eq!(decode(&lines).unwrap(), data);
    }

    #[test]
    fn test_decode_rejects_out_of_range_char() {
      let lines = vec!["! !".to_string()]; // space (32) is below '!'
      assert_eq!(decode(&lines), None);
    }

    #[test]
    fn test_decode_accepts_backtick() {
      // '`' (96) encodes value 63 — the top of the range.
      let lines = vec!["```".to_string()];
      assert_eq!(decode(&lines).unwrap(), vec![255, 255]);
    }

    #[test]
    fn test_round_trip_odd_length_across_line_wrap() {
      let data: Vec<u8> = (0..=255u8).cycle().take(997).collect();
      let encoded = encode(&data);
      let lines: Vec<String> = encoded.lines().map(str::to_string).collect();
      assert!(lines[..lines.len() - 1].iter().all(|l| l.len() == 80));
      assert!(lines.last().unwrap().len() < 80);
      assert_eq!(decode(&lines).unwrap(), data);
    }

    #[test]
    fn test_round_trip_single_byte_uses_two_char_tail() {
      let data = vec![0xAB];
      let lines: Vec<String> = encode(&data).lines().map(str::to_string).collect();
      assert_eq!(lines.len(), 1);
      assert_eq!(lines[0].len(), 2);
      assert_eq!(decode(&lines).unwrap(), data);
    }

    #[test]
    fn test_round_trip_empty() {
      assert_eq!(encode(&[]), "");
      assert_eq!(decode(&[]).unwrap(), Vec::<u8>::new());
    }
  }
}

pub fn detect_format(data: &[u8]) -> Option<crate::model::Format> {
  if let Some(text) = crate::encoding::try_decode_for_detection(data) {
    if text.contains("[Script Info]") {
      if text.contains("V4+ Styles") || text.contains("V4 Styles") {
        return Some(crate::model::Format::Ass);
      }
      return Some(crate::model::Format::Ssa);
    }
  }
  None
}

fn parse_ass_time(h: &str, m: &str, s: &str, ms: &str) -> Result<u64, SubtitleError> {
  let hours: u64 = h.parse().map_err(|_| SubtitleError::InvalidTimestamp {
    format: Format::Ass,
    value: h.to_string(),
  })?;
  let minutes: u64 = m.parse().map_err(|_| SubtitleError::InvalidTimestamp {
    format: Format::Ass,
    value: m.to_string(),
  })?;
  let seconds: u64 = s.parse().map_err(|_| SubtitleError::InvalidTimestamp {
    format: Format::Ass,
    value: s.to_string(),
  })?;
  let centiseconds: u64 = ms.parse().map_err(|_| SubtitleError::InvalidTimestamp {
    format: Format::Ass,
    value: ms.to_string(),
  })?;
  Ok(hours * MS_PER_HOUR + minutes * MS_PER_MINUTE + seconds * MS_PER_SECOND + centiseconds * 10)
}

fn parse_ass_style_line(line: &str) -> Option<AssStyle> {
  let caps = RE_STYLE.captures(line)?;
  Some(AssStyle {
    name: caps[1].to_string(),
    fontname: caps[2].to_string(),
    fontsize: caps[3].parse().unwrap_or(48),
    primary_color: caps[4].to_string(),
    secondary_color: caps[5].to_string(),
    outline_color: caps[6].to_string(),
    back_color: caps[7].to_string(),
    bold: caps[8].parse::<i32>().unwrap_or(0) < 0,
    italic: caps[9].parse::<i32>().unwrap_or(0) < 0,
    underline: caps[10].parse::<i32>().unwrap_or(0) < 0,
    strikeout: caps[11].parse::<i32>().unwrap_or(0) < 0,
    scale_x: caps[12].parse().unwrap_or(100.0),
    scale_y: caps[13].parse().unwrap_or(100.0),
    spacing: caps[14].parse().unwrap_or(0.0),
    angle: caps[15].parse().unwrap_or(0.0),
    border_style: caps[16].parse().unwrap_or(1),
    outline: caps[17].parse().unwrap_or(2.0),
    shadow: caps[18].parse().unwrap_or(2.0),
    alignment: caps[19].parse().unwrap_or(2),
    margin_l: caps[20].parse().unwrap_or(10),
    margin_r: caps[21].parse().unwrap_or(10),
    margin_v: caps[22].parse().unwrap_or(10),
    encoding: caps[23].parse().unwrap_or(1),
  })
}

fn parse_ass_dialogue(line: &str) -> Option<Subtitle> {
  let caps = RE_DIALOGUE.captures(line)?;
  let start = parse_ass_time(&caps[1], &caps[2], &caps[3], &caps[4]).ok()?;
  let end = parse_ass_time(&caps[5], &caps[6], &caps[7], &caps[8]).ok()?;

  let style = caps.get(9).and_then(|m| {
    let s = m.as_str().trim();
    if s.is_empty() {
      None
    } else {
      Some(s.to_string())
    }
  });
  let actor = caps.get(10).and_then(|m| {
    let s = m.as_str().trim();
    if s.is_empty() {
      None
    } else {
      Some(s.to_string())
    }
  });
  let text = caps.get(16).map_or("", |m| m.as_str());

  // Check if this is a comment line:
  // 1. Line starts with "Comment:" (case-insensitive)
  // 2. OR Effect field (capture group 14) contains "Comment"
  let is_comment = line.trim().to_lowercase().starts_with("comment:")
    || caps.get(14).is_some_and(|m| m.as_str().contains("Comment"));

  let mut subtitle = Subtitle::new(start, end, text);
  subtitle.style = style;
  subtitle.actor = actor;
  subtitle.is_comment = is_comment;
  subtitle.text_parts = parse_ass_tags(text).into_iter().collect();
  Some(subtitle)
}

/// Scan ASS override tags for layout info: the first `\pos(x,y)` or
/// `\move(x1,y1,…)` (collapsed to its start point — animation is not
/// modeled) gives pixel coordinates, the first `\anN` gives a numpad
/// alignment override. Tags inside `\t(...)` transforms are ignored,
/// matching `parse_ass_tags`.
fn scan_ass_layout(text: &str) -> (Option<(f64, f64)>, Option<u32>) {
  let mut pos = None;
  let mut an = None;
  let mut in_transform = false;

  for caps in RE_ASS_TAG_INLINE.captures_iter(text) {
    for tag in caps[1].split('\\') {
      let tag = tag.trim();
      if in_transform {
        if tag.contains(')') {
          in_transform = false;
        }
        continue;
      }
      if let Some(rest) = tag.strip_prefix("t(") {
        in_transform = !rest.contains(')');
        continue;
      }
      if pos.is_none() {
        let args = tag
          .strip_prefix("pos(")
          .or_else(|| tag.strip_prefix("move("));
        if let Some(args) = args {
          let mut nums = args
            .trim_end_matches(')')
            .split(',')
            .filter_map(|n| n.trim().parse::<f64>().ok());
          if let (Some(x), Some(y)) = (nums.next(), nums.next()) {
            pos = Some((x, y));
          }
          continue;
        }
      }
      let an_candidate = tag
        .strip_prefix("an")
        .and_then(|d| d.parse::<u32>().ok())
        .filter(|n| (1..=9).contains(n));
      if an.is_none() && an_candidate.is_some() {
        an = an_candidate;
      }
    }
  }
  (pos, an)
}

fn ass_alignment_to_align(an: u32) -> (HorizontalAlign, VerticalAlign) {
  let h = match an {
    1 | 4 | 7 => HorizontalAlign::Left,
    3 | 6 | 9 => HorizontalAlign::Right,
    _ => HorizontalAlign::Center,
  };
  let v = match an {
    7..=9 => VerticalAlign::Top,
    4..=6 => VerticalAlign::Center,
    _ => VerticalAlign::Bottom,
  };
  (h, v)
}

fn round2(v: f64) -> f64 {
  (v * 100.0).round() / 100.0
}

pub fn parse_content(content: &str) -> AnyResult<SubtitleFile> {
  let mut info = HashMap::new();
  let estimated_subs = (content.len() / 300).max(32);
  let mut styles = Vec::new();
  let mut subtitles: Vec<Subtitle> = Vec::with_capacity(estimated_subs);
  let mut fonts = Vec::new();
  let mut font_name: Option<String> = None;
  let mut font_lines: Vec<String> = Vec::new();

  let mut flush_font = |font_name: &mut Option<String>, font_lines: &mut Vec<String>| {
    if let Some(name) = font_name.take() {
      if let Some(data) = uuencode::decode(font_lines) {
        fonts.push(AssFont { name, data });
      }
      font_lines.clear();
    }
  };

  let mut section = Section::None;

  for line in content.lines() {
    let trimmed = line.trim();
    if trimmed.is_empty() {
      continue;
    }

    if trimmed.starts_with('[') && trimmed.ends_with(']') {
      flush_font(&mut font_name, &mut font_lines);

      let section_name = &trimmed[1..trimmed.len() - 1].to_lowercase();
      section = match section_name.as_str() {
        "script info" => Section::Info,
        "v4+ styles" | "v4 styles" => Section::Styles,
        "events" => Section::Events,
        "fonts" => Section::Fonts,
        _ => Section::Other,
      };
      continue;
    }

    // Skip Format: lines in Events section
    if section == Section::Events && trimmed.starts_with("Format:") {
      continue;
    }

    match section {
      Section::Info => {
        if let Some(caps) = RE_INFO.captures(trimmed) {
          info.insert(caps[1].to_string(), caps[2].trim().to_string());
        }
      }
      Section::Styles => {
        if trimmed.starts_with("Format:") {
          continue;
        }
        if let Some(style) = parse_ass_style_line(trimmed) {
          styles.push(style);
        }
      }
      Section::Events => {
        if let Some(subtitle) = parse_ass_dialogue(trimmed) {
          subtitles.push(subtitle);
        }
      }
      Section::Fonts => {
        if let Some(name) = trimmed.strip_prefix("fontname:") {
          flush_font(&mut font_name, &mut font_lines);
          font_name = Some(name.trim().to_string());
        } else if font_name.is_some() {
          font_lines.push(trimmed.to_string());
        }
      }
      Section::Other => {}
      Section::None => {}
    }
  }

  flush_font(&mut font_name, &mut font_lines);

  // PlayRes defaults per the ASS spec (384x288) when [Script Info] omits them;
  // \pos coordinates are converted to % of the play resolution.
  let play_res_x = info
    .get("PlayResX")
    .and_then(|v| v.trim().parse::<f64>().ok())
    .filter(|v| *v > 0.0)
    .unwrap_or(384.0);
  let play_res_y = info
    .get("PlayResY")
    .and_then(|v| v.trim().parse::<f64>().ok())
    .filter(|v| *v > 0.0)
    .unwrap_or(288.0);

  let style_map: HashMap<&str, &AssStyle> = styles.iter().map(|s| (s.name.as_str(), s)).collect();
  for sub in &mut subtitles {
    let style = sub.style.as_deref().and_then(|name| style_map.get(name));
    if let Some(style) = style {
      sub.style_props = Some(StyleProps {
        font_family: Some(style.fontname.clone()),
        font_size: Some(format!("{}px", style.fontsize)),
        color: ass_color_to_ttml(&style.primary_color),
        bold: style.bold,
        italic: style.italic,
        underline: style.underline,
      });
    }

    let (pos, an) = scan_ass_layout(&sub.text);
    let alignment = an.or(style.map(|s| s.alignment));
    if pos.is_some() || alignment.is_some() {
      let (h_align, v_align) = ass_alignment_to_align(alignment.unwrap_or(2));
      let (x, y) = match pos {
        Some((px, py)) => (
          Some(round2(px / play_res_x * 100.0)),
          Some(round2(py / play_res_y * 100.0)),
        ),
        None => (None, None),
      };
      sub.position = Some(CuePosition {
        x,
        y,
        h_align,
        v_align,
      });
    }
  }

  Ok(SubtitleFile::Ass(AssData {
    info,
    styles,
    fonts,
    subtitles,
  }))
}

pub fn parse_bytes(data: &[u8]) -> AnyResult<SubtitleFile> {
  let text = crate::encoding::decode_to_string(data)?;
  parse_content(&text)
}

#[cfg(not(target_arch = "wasm32"))]
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

#[derive(PartialEq)]
enum Section {
  None,
  Info,
  Styles,
  Events,
  Fonts,
  Other,
}

fn format_ass_color(color: &str) -> String {
  if color.is_empty() {
    "&H00FFFFFF".to_string()
  } else {
    color.to_string()
  }
}

/// "&HAABBGGRR" (or SSA's "&HBBGGRR") → "#RRGGBB".
/// Alpha is dropped: TTML1 tts:color has no alpha channel.
fn ass_color_to_ttml(color: &str) -> Option<String> {
  let hex = color
    .strip_prefix("&H")
    .or_else(|| color.strip_prefix("&h"))?;
  if hex.len() != 8 && hex.len() != 6 {
    return None;
  }
  let n = u32::from_str_radix(hex, 16).ok()?;
  Some(format!(
    "#{:02X}{:02X}{:02X}",
    n & 0xFF,
    (n >> 8) & 0xFF,
    (n >> 16) & 0xFF
  ))
}

/// Write subtitles to a file in ASS format.
///
/// `policy` controls overwrite behavior (None = default Overwrite).
/// Uses an empty `[Script Info]` and a single default style — for
/// custom info/styles, call `to_string` directly and write the result
/// with `tokio::fs::write`.
///
/// This also serves SSA output: ASS and SSA share the same body
/// syntax; the only difference is the `ScriptType` line in
/// `[Script Info]`, which `to_string` handles.
#[cfg(not(target_arch = "wasm32"))]
pub async fn generate(
  subtitles: &[Subtitle],
  file_path: impl AsRef<std::path::Path>,
  policy: Option<crate::model::WritePolicy>,
) -> AnyResult<String> {
  let content = to_string(
    &HashMap::new(),
    &[AssStyle::default_style()],
    subtitles,
    &[],
  );
  let path = file_path.as_ref();
  crate::io::write_with_policy(path, content.as_bytes(), policy).await?;
  Ok(path.to_string_lossy().into_owned())
}

pub fn to_string(
  info: &HashMap<String, String>,
  styles: &[AssStyle],
  subtitles: &[Subtitle],
  fonts: &[AssFont],
) -> String {
  let mut buf = String::new();

  buf.push_str("[Script Info]\n");
  if info.is_empty() {
    buf.push_str("Title: <untitled>\n");
    buf.push_str("ScriptType: v4.00+\n");
    buf.push_str("PlayResX: 384\n");
    buf.push_str("PlayResY: 288\n");
    buf.push_str("WrapStyle: 0\n");
  } else {
    for (key, value) in info {
      buf.push_str(&format!("{}: {}\n", key, value));
    }
  }
  buf.push('\n');

  buf.push_str("[V4+ Styles]\n");
  buf.push_str("Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n");
  for style in styles {
    buf.push_str(&format!(
      "Style: {},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
      style.name,
      style.fontname,
      style.fontsize,
      format_ass_color(&style.primary_color),
      format_ass_color(&style.secondary_color),
      format_ass_color(&style.outline_color),
      format_ass_color(&style.back_color),
      if style.bold { -1 } else { 0 },
      if style.italic { -1 } else { 0 },
      if style.underline { -1 } else { 0 },
      if style.strikeout { -1 } else { 0 },
      style.scale_x,
      style.scale_y,
      style.spacing,
      style.angle,
      style.border_style,
      style.outline,
      style.shadow,
      style.alignment,
      style.margin_l,
      style.margin_r,
      style.margin_v,
      style.encoding,
    ));
  }
  buf.push('\n');

  buf.push_str("[Events]\n");
  buf.push_str("Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n");
  for sub in subtitles {
    let start = format_ass_timestamp(sub.start);
    let end = format_ass_timestamp(sub.end);
    let style = sub.style.as_deref().unwrap_or("Default");
    let actor = sub.actor.as_deref().unwrap_or("");
    let margin_l = 0;
    let margin_r = 0;
    let margin_v = 0;
    let effect = "";
    let layer = 0;
    let line_type = if sub.is_comment {
      "Comment"
    } else {
      "Dialogue"
    };
    buf.push_str(&format!(
      "{}: {},{},{},{},{},{},{},{},{},{}\n",
      line_type, layer, start, end, style, actor, margin_l, margin_r, margin_v, effect, sub.text
    ));
  }

  if !fonts.is_empty() {
    buf.push_str("[Fonts]\n");
    for font in fonts {
      buf.push_str(&format!("fontname: {}\n", font.name));
      buf.push_str(&uuencode::encode(&font.data));
      buf.push('\n');
    }
    buf.push('\n');
  }

  buf
}

/// `\b`/`\i`/`\u` argument: empty means on, an integer means on, iff > 0
/// (`\b700` is a bold weight). Non-numeric arguments (`ord3.6` from
/// `\bord`, `lur5` from `\blur`, …) are ignored.
fn parse_toggle(arg: &str, flag: &mut bool) {
  if arg.is_empty() {
    *flag = true;
  } else if let Ok(n) = arg.parse::<u32>() {
    *flag = n > 0;
  }
}

/// Parse ASS override tags into styled `TextPart`s.
pub fn parse_ass_tags(text: &str) -> Vec<crate::model::TextPart> {
  let mut parts = Vec::new();
  let mut bold = false;
  let mut italic = false;
  let mut underline = false;
  let mut color: Option<String> = None;
  // \pN (N>=1): following text is vector drawing commands, not visible text.
  let mut drawing = false;
  // Inside \t(...): inner tags are animated, don't apply them as state.
  let mut in_transform = false;
  let mut saw_drawing = false;
  let mut current = String::new();

  let re = &RE_ASS_TAG_INLINE;
  let mut last_end = 0usize;

  for caps in re.captures_iter(text) {
    let m = caps.get(0).unwrap();
    let tag_start = m.start();
    let tag_end = m.end();

    if tag_start > last_end && !drawing {
      let segment = &text[last_end..tag_start];
      let cleaned = segment
        .replace("\\N", "\n")
        .replace("\\n", "\n")
        .replace("\\h", " ");
      current.push_str(&cleaned);
    }

    if !current.is_empty() {
      let mut part =
        crate::model::TextPart::new(std::mem::take(&mut current), bold, italic, underline);
      part.color = color.clone();
      parts.push(part);
    }

    let tag_content = &caps[1];
    for tag in tag_content.split('\\') {
      let tag = tag.trim();
      if in_transform {
        if tag.contains(')') {
          in_transform = false;
        }
        continue;
      }

      match tag {
        "r" => {
          bold = false;
          italic = false;
          underline = false;
          color = None;
        }
        "b" => bold = true,
        "i" => italic = true,
        "u" => underline = true,
        // \t(...) transform; may be self-contained (\t(500,\bord1) splits
        // into "t(500," and "bord1)").
        t if t.starts_with("t(") => in_transform = !t[2..].contains(')'),
        // \r<StyleName>: reset to the named style's defaults.
        t if t.starts_with('r') && t[1..].starts_with(char::is_alphabetic) => {
          bold = false;
          italic = false;
          underline = false;
          color = None;
        }
        // \b0/\b1/\b<weight>, \i0/\i1, \u0/\u1; non-numeric arguments
        // (\bord, \blur, \iclip, …) are ignored.
        t if t.starts_with('b') => parse_toggle(&t[1..], &mut bold),
        t if t.starts_with('i') => parse_toggle(&t[1..], &mut italic),
        t if t.starts_with('u') => parse_toggle(&t[1..], &mut underline),
        // \c&HBBGGRR& / \1c&HBBGGRR& → primary text color.
        t if t.starts_with("c&") || t.starts_with("1c&") => {
          let c = t
            .strip_prefix("1c&")
            .unwrap_or(&t[2..])
            .trim_end_matches('&');
          let raw = if c.starts_with(['H', 'h']) {
            format!("&{c}")
          } else {
            format!("&H{c}")
          };
          color = ass_color_to_ttml(&raw);
        }
        // \pN toggles drawing mode; \pos(/\pbo) fails the int parse.
        t if t.starts_with('p') => {
          if let Ok(n) = t[1..].parse::<u32>() {
            drawing = n >= 1;
            saw_drawing |= drawing;
          }
        }
        _ => {}
      }
    }

    last_end = tag_end;
  }

  if last_end < text.len() && !drawing {
    let segment = &text[last_end..];
    let cleaned = segment
      .replace("\\N", "\n")
      .replace("\\n", "\n")
      .replace("\\h", " ");
    current.push_str(&cleaned);
  }

  if !current.is_empty() {
    let mut part = crate::model::TextPart::new(current, bold, italic, underline);
    part.color = color;
    parts.push(part);
  }

  if parts.is_empty() && (saw_drawing || re.is_match(text)) {
    parts.push(crate::model::TextPart::plain(""));
  }

  parts
}

pub fn ass_to_plaintext(text: &str) -> String {
  let stripped = RE_ASS_TAG_INLINE.replace_all(text, "");
  stripped
    .replace("\\N", "\n")
    .replace("\\n", "\n")
    .replace("\\h", " ")
    .to_string()
}

fn format_ass_timestamp(ms: u64) -> String {
  let total_seconds = ms / 1000;
  let centiseconds = (ms % 1000) / 10;
  let hours = total_seconds / 3600;
  let minutes = (total_seconds % 3600) / 60;
  let seconds = total_seconds % 60;
  format!(
    "{}:{:02}:{:02}.{:02}",
    hours, minutes, seconds, centiseconds
  )
}

/// Write ASS/SSA subtitles to an async writer streamingly.
#[cfg(not(target_arch = "wasm32"))]
pub async fn write_stream<W: tokio::io::AsyncWrite + Unpin>(
  info: &HashMap<String, String>,
  styles: &[AssStyle],
  subtitles: &[Subtitle],
  fonts: &[AssFont],
  writer: &mut W,
) -> AnyResult<()> {
  // Write [Script Info]
  writer.write_all(b"[Script Info]\n").await?;
  if info.is_empty() {
    writer.write_all(b"Title: <untitled>\n").await?;
    writer.write_all(b"ScriptType: v4.00+\n").await?;
    writer.write_all(b"PlayResX: 384\n").await?;
    writer.write_all(b"PlayResY: 288\n").await?;
    writer.write_all(b"WrapStyle: 0\n").await?;
  } else {
    for (key, value) in info {
      writer
        .write_all(format!("{}: {}\n", key, value).as_bytes())
        .await?;
    }
  }
  writer.write_all(b"\n").await?;

  // Write [V4+ Styles]
  writer.write_all(b"[V4+ Styles]\n").await?;
  writer.write_all(b"Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n").await?;
  for style in styles {
    let line = format!(
      "Style: {},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
      style.name,
      style.fontname,
      style.fontsize,
      format_ass_color(&style.primary_color),
      format_ass_color(&style.secondary_color),
      format_ass_color(&style.outline_color),
      format_ass_color(&style.back_color),
      if style.bold { -1 } else { 0 },
      if style.italic { -1 } else { 0 },
      if style.underline { -1 } else { 0 },
      if style.strikeout { -1 } else { 0 },
      style.scale_x,
      style.scale_y,
      style.spacing,
      style.angle,
      style.border_style,
      style.outline,
      style.shadow,
      style.alignment,
      style.margin_l,
      style.margin_r,
      style.margin_v,
      style.encoding,
    );
    writer.write_all(line.as_bytes()).await?;
  }
  writer.write_all(b"\n").await?;

  if !fonts.is_empty() {
    writer.write_all(b"[Fonts]\n").await?;
    for font in fonts {
      writer
        .write_all(format!("fontname: {}\n", font.name).as_bytes())
        .await?;
      writer
        .write_all(uuencode::encode(&font.data).as_bytes())
        .await?;
      writer.write_all(b"\n").await?;
    }
    writer.write_all(b"\n").await?;
  }

  // Write [Events]
  writer.write_all(b"[Events]\n").await?;
  writer
    .write_all(b"Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n")
    .await?;
  for sub in subtitles {
    let start = format_ass_timestamp(sub.start);
    let end = format_ass_timestamp(sub.end);
    let style = sub.style.as_deref().unwrap_or("Default");
    let actor = sub.actor.as_deref().unwrap_or("");
    let layer = 0;
    let line_type = if sub.is_comment {
      "Comment"
    } else {
      "Dialogue"
    };
    let line = format!(
      "{}: {},{},{},{},{},0,0,0,,{}\n",
      line_type, layer, start, end, style, actor, sub.text
    );
    writer.write_all(line.as_bytes()).await?;
  }

  writer.flush().await?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::SubtitleFormat;

  #[test]
  fn test_parse_basic_ass() {
    let content = "[Script Info]\nTitle: Test\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: ...\nStyle: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,Hello!\n";
    let result = parse_content(content).unwrap();
    let subs = result.subtitles();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].start, 1000);
    assert_eq!(subs[0].end, 3500);
    assert_eq!(subs[0].text, "Hello!");
    assert_eq!(subs[0].style.as_deref(), Some("Default"));
  }

  #[test]
  fn test_parse_ass_multiple_cues() {
    let content = "[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: ...\nStyle: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,Line 1\nDialogue: 0,0:00:04.00,0:00:06.50,Default,,0,0,0,,Line 2\n";
    let result = parse_content(content).unwrap();
    assert_eq!(result.subtitles().len(), 2);
    assert_eq!(result.subtitles()[0].text, "Line 1");
    assert_eq!(result.subtitles()[1].text, "Line 2");
  }

  #[test]
  fn test_ass_round_trip() {
    let content = "[Script Info]\nTitle: Round Trip\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,Hello\nDialogue: 0,0:00:04.00,0:00:06.50,Default,,0,0,0,,World\n";

    let parsed = parse_content(content).unwrap();
    let regenerated = parsed.to_string();

    let reparsed = parse_content(&regenerated).unwrap();

    assert_eq!(reparsed.subtitles().len(), 2);
    assert_eq!(reparsed.subtitles()[0].start, 1000);
    assert_eq!(reparsed.subtitles()[0].text, "Hello");
    assert_eq!(reparsed.subtitles()[1].start, 4000);
    assert_eq!(reparsed.subtitles()[1].text, "World");
  }

  #[test]
  fn test_detect_format_ass() {
    let data = b"[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\n";
    assert_eq!(detect_format(data), Some(crate::model::Format::Ass));
  }

  #[test]
  fn test_parse_style_float_outline_shadow() {
    let content = "[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: OP,FOT-Rowdy Std EB,66,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,3.9,0,8,30,30,69,1\nStyle: Default,LTFinnegan Medium,72,&H00FFFFFF,&H00FFFFFF,&H00000000,&HA0000000,0,0,0,0,100,100,0.5,0.5,1,3.6,1.5,2,200,200,60,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:03.50,OP,,0,0,0,,Hello\n";
    let result = parse_content(content).unwrap();
    let SubtitleFile::Ass(ass) = &result else {
      panic!("expected ASS file");
    };
    assert_eq!(ass.styles.len(), 2);
    assert_eq!(ass.styles[0].name, "OP");
    assert_eq!(ass.styles[0].outline, 3.9);
    assert_eq!(ass.styles[1].spacing, 0.5);
    assert_eq!(ass.styles[1].shadow, 1.5);

    // The dialogue's style_props must resolve against the parsed style.
    let sub = &result.subtitles()[0];
    let props = sub.style_props.as_ref().expect("style_props missing");
    assert_eq!(props.font_family.as_deref(), Some("FOT-Rowdy Std EB"));
    assert_eq!(props.font_size.as_deref(), Some("66px"));
    assert_eq!(props.color.as_deref(), Some("#FFFFFF"));
  }

  #[test]
  fn test_ass_to_string_preserves_styles() {
    let content = "[Script Info]\nTitle: Test\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\nStyle: Custom,Arial,36,&H0000FFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:03.50,Custom,,0,0,0,,Custom Style Text\n";
    let parsed = parse_content(content).unwrap();
    let output = parsed.to_string();
    assert!(output.contains("Style: Custom,"));
  }

  #[test]
  fn test_parse_bytes() {
    let data = b"[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: ...\nStyle: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,Hello\n";
    let result = parse_bytes(data.as_ref()).unwrap();
    assert_eq!(result.subtitles().len(), 1);
    assert_eq!(result.subtitles()[0].text, "Hello");
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[tokio::test]
  async fn test_parse_file() {
    let content = "[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: ...\nStyle: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,FromFile\n";
    let path = "test_ass_parse_file.ass";
    std::fs::write(path, content).unwrap();
    let result = parse_file(path).await.unwrap();
    let _ = std::fs::remove_file(path);
    assert_eq!(result.subtitles()[0].text, "FromFile");
  }

  #[test]
  fn test_is_comment_uses_effect() {
    // Parsing a line with Comment in the Effect column (group 14, not 15)
    let content = "[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,Comment,Visible text\n";
    let parsed = parse_content(content).unwrap();
    assert!(parsed.subtitles()[0].is_comment);
    // Text must be "Visible text" even though "Comment" appears in the Effect column
    assert_eq!(parsed.subtitles()[0].text, "Visible text");
  }

  #[test]
  fn test_parse_fonts_section() {
    let content = "[Script Info]\nScriptType: v4.00+\n\n[Fonts]\nfontname: tiny.ttf\n!\"#\nABC\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Hi\n";
    let SubtitleFile::Ass(data) = parse_content(content).unwrap() else {
      panic!("expected ASS");
    };
    assert_eq!(data.fonts.len(), 1);
    assert_eq!(data.fonts[0].name, "tiny.ttf");
    assert_eq!(data.fonts[0].data, vec![0, 16, 160, 134]);
    assert_eq!(data.subtitles.len(), 1);
  }

  #[test]
  fn test_parse_fonts_multiple_and_odd_byte_tail() {
    // Second font exercises the 2-char group encoding a single byte (0xAB).
    let content = "[Script Info]\nScriptType: v4.00+\n\n[Fonts]\nfontname: a.ttf\n!\"#\nABC\nfontname: b.ttf\nKQ\n\n[Events]\n";
    let SubtitleFile::Ass(data) = parse_content(content).unwrap() else {
      panic!("expected ASS");
    };
    assert_eq!(data.fonts.len(), 2);
    assert_eq!(data.fonts[0].name, "a.ttf");
    assert_eq!(data.fonts[0].data, vec![0, 16, 160, 134]);
    assert_eq!(data.fonts[1].name, "b.ttf");
    assert_eq!(data.fonts[1].data, vec![0xAB]);
  }

  #[test]
  fn test_parse_fonts_skips_malformed_payload() {
    let content =
      "[Script Info]\nScriptType: v4.00+\n\n[Fonts]\nfontname: bad.ttf\n! !#\n\n[Events]\n";
    let SubtitleFile::Ass(data) = parse_content(content).unwrap() else {
      panic!("expected ASS");
    };
    assert!(data.fonts.is_empty());
  }

  #[test]
  fn test_fonts_round_trip_via_to_string() {
    // Odd length on purpose: crosses the 80-char wrap and ends on a 2-char group.
    let data: Vec<u8> = (0..=255u8).cycle().take(997).collect();
    let fonts = vec![AssFont {
      name: "big.ttf".into(),
      data: data.clone(),
    }];
    let out = to_string(&HashMap::new(), &[], &[], &fonts);
    assert!(out.contains("[Fonts]\nfontname: big.ttf\n"));
    let SubtitleFile::Ass(parsed) = parse_content(&out).unwrap() else {
      panic!("expected ASS");
    };
    assert_eq!(parsed.fonts.len(), 1);
    assert_eq!(parsed.fonts[0].data, data);
  }

  #[test]
  fn test_parse_fills_style_props() {
    let content = "[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Custom,Arial,36,&H00463827,&H000000FF,&H00000000,&H00000000,-1,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:02.00,Custom,,0,0,0,,Styled\nDialogue: 0,0:00:03.00,0:00:04.00,Missing,,0,0,0,,No such style\n";
    let SubtitleFile::Ass(data) = parse_content(content).unwrap() else {
      panic!("expected ASS");
    };
    let props = data.subtitles[0].style_props.as_ref().unwrap();
    assert_eq!(props.font_family.as_deref(), Some("Arial"));
    assert_eq!(props.font_size.as_deref(), Some("36px"));
    assert_eq!(props.color.as_deref(), Some("#273846"));
    assert!(props.bold);
    assert!(!props.italic);
    // Unknown style name: no props resolved.
    assert!(data.subtitles[1].style_props.is_none());
  }

  #[cfg(feature = "ttml")]
  #[test]
  fn test_ass_to_ttml_conversion_emits_font_family() {
    let content = "[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Custom,Arial,36,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,-1,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:02.00,Custom,,0,0,0,,Styled\n";
    let file = parse_content(content).unwrap();
    let out = file.to_string_with_format(&Format::Ttml);
    assert!(out.contains("tts:fontFamily=\"Arial\""), "got: {}", out);
    assert!(out.contains("style=\"Custom\""), "got: {}", out);
  }

  fn parse_positioned(content: &str) -> Vec<Subtitle> {
    let SubtitleFile::Ass(data) = parse_content(content).unwrap() else {
      panic!("expected ASS");
    };
    data.subtitles
  }

  const ASS_HEADER: &str = "[Script Info]\nScriptType: v4.00+\nPlayResX: 384\nPlayResY: 288\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\nStyle: TopRight,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,9,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n";

  #[test]
  fn test_position_from_style_alignment() {
    // Style alignment 2 (bottom-center): band position, no coordinates.
    let subs = parse_positioned(&format!(
      "{ASS_HEADER}Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Hi\n"
    ));
    let pos = subs[0].position.as_ref().unwrap();
    assert_eq!(pos.x, None);
    assert_eq!(pos.y, None);
    assert_eq!(pos.h_align, HorizontalAlign::Center);
    assert_eq!(pos.v_align, VerticalAlign::Bottom);
  }

  #[test]
  fn test_position_style_alignment_9() {
    // Alignment 9 → top-right.
    let subs = parse_positioned(&format!(
      "{ASS_HEADER}Dialogue: 0,0:00:01.00,0:00:02.00,TopRight,,0,0,0,,Hi\n"
    ));
    let pos = subs[0].position.as_ref().unwrap();
    assert_eq!(pos.h_align, HorizontalAlign::Right);
    assert_eq!(pos.v_align, VerticalAlign::Top);
  }

  #[test]
  fn test_position_an_overrides_style() {
    let subs = parse_positioned(&format!(
      "{ASS_HEADER}Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,{{\\an8}}Top\n"
    ));
    let pos = subs[0].position.as_ref().unwrap();
    assert_eq!(pos.h_align, HorizontalAlign::Center);
    assert_eq!(pos.v_align, VerticalAlign::Top);
  }

  #[test]
  fn test_position_pos_converts_via_playres() {
    // \pos(192,144) on PlayRes 384x288 → 50%, 50% (verified with python3).
    let subs = parse_positioned(&format!(
      "{ASS_HEADER}Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,{{\\pos(192,144)}}Mid\n"
    ));
    let pos = subs[0].position.as_ref().unwrap();
    assert_eq!(pos.x, Some(50.0));
    assert_eq!(pos.y, Some(50.0));
  }

  #[test]
  fn test_position_move_uses_start_point() {
    // \move(96,48,…) start point → 25%, 16.67 (rounded 2dp; verified with python3).
    let subs = parse_positioned(&format!(
      "{ASS_HEADER}Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,{{\\move(96,48,192,144)\\an7}}Go\n"
    ));
    let pos = subs[0].position.as_ref().unwrap();
    assert_eq!(pos.x, Some(25.0));
    assert_eq!(pos.y, Some(16.67));
    assert_eq!(pos.h_align, HorizontalAlign::Left);
    assert_eq!(pos.v_align, VerticalAlign::Top);
  }

  #[test]
  fn test_position_default_playres_when_missing() {
    // No PlayRes in [Script Info] → spec default 384x288.
    let content = "[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,{\\pos(38,29)}Hi\n";
    let subs = parse_positioned(content);
    let pos = subs[0].position.as_ref().unwrap();
    // 38/384 = 9.8958… → 9.9; 29/288 = 10.069… → 10.07 (verified with python3).
    assert_eq!(pos.x, Some(9.9));
    assert_eq!(pos.y, Some(10.07));
  }

  #[cfg(feature = "ttml")]
  #[test]
  fn test_ass_to_ttml_emits_layout() {
    let subs = parse_positioned(&format!(
      "{ASS_HEADER}Dialogue: 0,0:00:01.00,0:00:02.00,TopRight,,0,0,0,,{{\\pos(192,144)}}Mid\n"
    ));
    let file = SubtitleFile::Ass(AssData {
      info: HashMap::new(),
      styles: Vec::new(),
      fonts: Vec::new(),
      subtitles: subs,
    });
    let out = file.to_string_with_format(&Format::Ttml);
    assert!(out.contains("<layout>"), "got: {out}");
    assert!(out.contains("tts:origin=\"50% 50%\""), "got: {out}");
    assert!(out.contains("tts:displayAlign=\"before\""), "got: {out}");
    assert!(out.contains("tts:textAlign=\"right\""), "got: {out}");
  }
}
