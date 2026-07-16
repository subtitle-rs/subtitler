use crate::model::{AssData, AssStyle, Subtitle, SubtitleFile};
use crate::types::AnyResult;
use anyhow::anyhow;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

static RE_DIALOGUE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"^Dialogue:\s*(?:\d+,)?(\d+):(\d+):(\d+)[,.](\d+),(\d+):(\d+):(\d+)[,.](\d+),(?:([^,]*),)?(?:([^,]*),)?(?:(-?\d+),)?(?:(-?\d+),)?(?:(-?\d+),)?(?:([^,]*),)?(?:(\{.*\})?,)?(.+)$").unwrap()
});

static RE_STYLE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"^Style:\s*([^,]*),([^,]*),(\d+),([^,]*),([^,]*),([^,]*),([^,]*),(-?\d+),(-?\d+),(-?\d+),(-?\d+),(\d+),(\d+),(\d+),(\d+),(\d+),(\d+),(\d+),(\d+),(\d+),(\d+),(\d+),(\d+)").unwrap()
});

static RE_INFO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([^:]+):\s*(.*)").unwrap());

static RE_ASS_TAG_INLINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{([^}]*)\}").unwrap());

pub fn detect_format(data: &[u8]) -> Option<crate::model::Format> {
  if let Some(text) = crate::encoding::try_decode_for_detection(data)
    && text.contains("[Script Info]")
  {
    if text.contains("V4+ Styles") || text.contains("V4 Styles") {
      return Some(crate::model::Format::Ass);
    }
    return Some(crate::model::Format::Ssa);
  }
  None
}

fn parse_ass_time(h: &str, m: &str, s: &str, ms: &str) -> AnyResult<u64> {
  let hours: u64 = h.parse().map_err(|_| anyhow!("Invalid hours: {h}"))?;
  let minutes: u64 = m.parse().map_err(|_| anyhow!("Invalid minutes: {m}"))?;
  let seconds: u64 = s.parse().map_err(|_| anyhow!("Invalid seconds: {s}"))?;
  let centiseconds: u64 = ms
    .parse()
    .map_err(|_| anyhow!("Invalid centiseconds: {ms}"))?;
  Ok(hours * 3600000 + minutes * 60000 + seconds * 1000 + centiseconds * 10)
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
  let is_comment = caps.get(14).is_some_and(|m| m.as_str().contains("Comment"));

  let mut subtitle = Subtitle::new(start, end, text);
  subtitle.style = style;
  subtitle.actor = actor;
  subtitle.is_comment = is_comment;
  Some(subtitle)
}

pub fn parse_content(content: &str) -> AnyResult<SubtitleFile> {
  let mut info = HashMap::new();
  let mut styles = Vec::new();
  let mut subtitles = Vec::new();
  let mut section = Section::None;

  for line in content.lines() {
    let trimmed = line.trim();
    if trimmed.is_empty() {
      continue;
    }

    if trimmed.starts_with('[') && trimmed.ends_with(']') {
      let section_name = &trimmed[1..trimmed.len() - 1].to_lowercase();
      section = match section_name.as_str() {
        "script info" => Section::Info,
        "v4+ styles" | "v4 styles" => Section::Styles,
        "events" => Section::Events,
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
      Section::Other => {}
      Section::None => {}
    }
  }

  Ok(SubtitleFile::Ass(AssData {
    info,
    styles,
    subtitles,
  }))
}

pub fn parse_bytes(data: &[u8]) -> AnyResult<SubtitleFile> {
  let text = crate::encoding::decode_to_string(data)?;
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

#[derive(PartialEq)]
enum Section {
  None,
  Info,
  Styles,
  Events,
  Other,
}

fn format_ass_color(color: &str) -> String {
  if color.is_empty() {
    "&H00FFFFFF".to_string()
  } else {
    color.to_string()
  }
}

pub fn to_string(
  info: &HashMap<String, String>,
  styles: &[AssStyle],
  subtitles: &[Subtitle],
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
    buf.push_str(&format!(
      "Dialogue: {},{},{},{},{},{},{},{},{},{}\n",
      layer, start, end, style, actor, margin_l, margin_r, margin_v, effect, sub.text
    ));
  }

  buf
}

pub fn parse_ass_tags(text: &str) -> Vec<crate::model::TextPart> {
  let mut parts = Vec::new();
  let mut bold = false;
  let mut italic = false;
  let mut underline = false;
  let mut color: Option<String> = None;
  let mut current = String::new();

  let re = &RE_ASS_TAG_INLINE;
  let mut last_end = 0usize;

  for caps in re.captures_iter(text) {
    let m = caps.get(0).unwrap();
    let tag_start = m.start();
    let tag_end = m.end();

    if tag_start > last_end {
      let segment = &text[last_end..tag_start];
      let cleaned = segment
        .replace("\\N", "\n")
        .replace("\\n", "\n")
        .replace("\\h", " ");
      current.push_str(&cleaned);
    }

    if !current.is_empty() {
      parts.push(crate::model::TextPart {
        text: std::mem::take(&mut current),
        bold,
        italic,
        underline,
        color: color.clone(),
        voice: None,
      });
    }

    let tag_content = &caps[1];
    for tag in tag_content.split('\\') {
      if tag == "b1" || tag == "b" {
        bold = true;
      } else if tag == "b0" {
        bold = false;
      } else if tag == "i1" || tag == "i" {
        italic = true;
      } else if tag == "i0" {
        italic = false;
      } else if tag == "u1" || tag == "u" {
        underline = true;
      } else if tag == "u0" {
        underline = false;
      } else if let Some(c) = tag.strip_prefix("c&") {
        color = Some(format!("&{}", c));
      } else if tag == "r" {
        bold = false;
        italic = false;
        underline = false;
        color = None;
      }
    }

    last_end = tag_end;
  }

  if last_end < text.len() {
    let segment = &text[last_end..];
    let cleaned = segment
      .replace("\\N", "\n")
      .replace("\\n", "\n")
      .replace("\\h", " ");
    current.push_str(&cleaned);
  }

  if !current.is_empty() {
    parts.push(crate::model::TextPart {
      text: current,
      bold,
      italic,
      underline,
      color: color.clone(),
      voice: None,
    });
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
}
