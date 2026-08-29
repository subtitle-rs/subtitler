//! TTML (Timed Text Markup Language) subtitle parser and generator.
//!
//! Parses `begin`/`end` timed `<p>` elements with inline `<span>` styling.
//! Supports the IMSC 1.0/1.1 profile common in streaming (Netflix, etc.).
//!
//! Uses `quick-xml` for streaming pull parsing — no DOM build.

use crate::error::SubtitleError;
use crate::model::{Format, StyleProps, Subtitle, SubtitleFile, TextPart};
use crate::types::AnyResult;
use crate::utils::parse_timestamp;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use smallvec::SmallVec;
use std::collections::HashMap;
use std::io::Cursor;

/// Parse a TTML time value to milliseconds.
/// Supports: `hh:mm:ss.mmm`, `hh:mm:ss,mmm`, or seconds like `12.5s`.
fn ttml_to_ms(attr: &str) -> Option<u64> {
  let attr = attr.trim();
  if attr.contains(':') {
    // Check for SMPTE frame format: HH:MM:SS:FF (3 colons, used by iTT)
    let colons = attr.chars().filter(|&c| c == ':').count();
    if colons == 3 {
      // Parse HH:MM:SS:FF → assume 29.97fps non-drop (iTT/SMPTE)
      let parts: Vec<&str> = attr.split(':').collect();
      if parts.len() == 4 {
        let h: u64 = parts[0].parse().ok()?;
        let m: u64 = parts[1].parse().ok()?;
        let s: u64 = parts[2].parse().ok()?;
        let f: u64 = parts[3].parse().ok()?;
        // frames → ms at 29.97fps
        let total_frames = h * 3600 * 30 + m * 60 * 30 + s * 30 + f;
        return Some(((total_frames as f64) / 29.97 * 1000.0).round() as u64);
      }
    }
    return parse_timestamp(attr, Format::Ttml).ok();
  }
  // Handle "123.456s" format (seconds with optional 's' suffix)
  let num_str = attr.strip_suffix('s').unwrap_or(attr);
  num_str
    .parse::<f64>()
    .ok()
    .map(|secs| (secs * 1000.0).round() as u64)
}

/// Extract the local name from a potentially namespaced tag.
/// `tt:p` → `p`, `p` → `p`
fn local_name(name: &[u8]) -> &[u8] {
  match name.iter().position(|&b| b == b':') {
    Some(pos) => &name[pos + 1..],
    None => name,
  }
}

/// Raw `<style>` definition: own props plus parent style references.
#[derive(Default)]
struct RawStyle {
  parents: Vec<String>,
  props: StyleProps,
}

fn merge_style_attribute(props: &mut StyleProps, key: &[u8], val: &str) {
  match key {
    b"fontFamily" => props.font_family = Some(val.to_string()),
    b"fontSize" => props.font_size = Some(val.to_string()),
    b"color" => props.color = Some(val.to_string()),
    b"fontWeight" => props.bold = val == "bold",
    b"fontStyle" => props.italic = val == "italic",
    b"textDecoration" => props.underline = val.split_whitespace().any(|t| t == "underline"),
    _ => {}
  }
}

fn parse_style_tag(e: &BytesStart, styles: &mut HashMap<String, RawStyle>) {
  let mut id = None;
  let mut parents = Vec::new();
  let mut props = StyleProps::default();
  for attr in e.attributes().flatten() {
    let key = local_name(attr.key.as_ref());
    let val = String::from_utf8_lossy(&attr.value);
    match key {
      b"id" => id = Some(val.into_owned()),
      b"style" => parents = val.split_whitespace().map(str::to_string).collect(),
      _ => merge_style_attribute(&mut props, key, &val),
    }
  }
  if let Some(id) = id {
    styles.insert(id, RawStyle { parents, props });
  }
}

/// Resolve a style id to its effective props, walking parent references.
fn resolve_style(id: &str, styles: &HashMap<String, RawStyle>) -> StyleProps {
  fn recurse(
    id: &str,
    styles: &HashMap<String, RawStyle>,
    visited: &mut Vec<String>,
  ) -> StyleProps {
    let mut props = StyleProps::default();
    if visited.iter().any(|v| v == id) {
      return props;
    }
    visited.push(id.to_string());
    if let Some(raw) = styles.get(id) {
      for parent in &raw.parents {
        props.merge_from(&recurse(parent, styles, visited));
      }
      props.merge_from(&raw.props);
    }
    props
  }

  recurse(id, styles, &mut Vec::new())
}

struct ParagraphAttrs {
  start: Option<u64>,
  end: Option<u64>,
  style: Option<String>,
  props: StyleProps,
}

/// Read <p> attributes: timing, style references (resolved), and direct
/// tts:* overrides (applied in a second pass so they win over references).
fn read_paragraph_attrs(e: &BytesStart, styles: &HashMap<String, RawStyle>) -> ParagraphAttrs {
  let mut out = ParagraphAttrs {
    start: None,
    end: None,
    style: None,
    props: StyleProps::default(),
  };

  for attr in e.attributes().flatten() {
    if local_name(attr.key.as_ref()) == b"style" {
      let val = String::from_utf8_lossy(&attr.value);
      for id in val.split_whitespace() {
        if out.style.is_none() {
          out.style = Some(id.to_string());
        }

        out.props.merge_from(&resolve_style(id, styles));
      }
    }
  }

  for attr in e.attributes().flatten() {
    let key = local_name(attr.key.as_ref());
    let val = String::from_utf8_lossy(&attr.value);
    match key {
      b"begin" => out.start = ttml_to_ms(&val),
      b"end" => out.end = ttml_to_ms(&val),
      b"dur" if out.end.is_none() => {
        if let (Some(s), Some(d)) = (out.start, ttml_to_ms(&val)) {
          out.end = Some(s + d);
        }
      }
      _ => merge_style_attribute(&mut out.props, key, &val),
    }
  }
  out
}

/// Parse TTML content into a SubtitleFile.
pub fn parse_content(content: &str) -> AnyResult<SubtitleFile> {
  let mut reader = Reader::from_str(content);
  reader.config_mut().trim_text(false);
  let mut buf = Vec::new();

  let mut subtitles: Vec<Subtitle> = Vec::with_capacity((content.len() / 300).max(16));
  // Assumes <head> precedes <body>, so style defs are known before <p> events.
  let mut styles: HashMap<String, RawStyle> = HashMap::new();
  let mut in_p = false;
  let mut current_start: Option<u64> = None;
  let mut current_end: Option<u64> = None;
  let mut current_style: Option<String> = None;
  let mut current_props = StyleProps::default();
  let mut current_text = String::new();
  let mut parts: SmallVec<[TextPart; 4]> = SmallVec::new();
  let mut in_span = false;
  let mut span_props = StyleProps::default();

  loop {
    match reader.read_event_into(&mut buf) {
      Ok(Event::Start(ref e)) => {
        let tag = local_name(e.name().as_ref()).to_vec();
        match tag.as_slice() {
          b"p" => {
            in_p = true;
            current_text.clear();
            parts.clear();
            let attrs = read_paragraph_attrs(e, &styles);
            current_start = attrs.start;
            current_end = attrs.end;
            current_style = attrs.style;
            current_props = attrs.props;
          }
          b"style" => parse_style_tag(e, &mut styles),
          b"span" => {
            in_span = true;
            span_props = StyleProps::default();
            for attr in e.attributes().flatten() {
              let key = local_name(attr.key.as_ref());
              let val = String::from_utf8_lossy(&attr.value);
              merge_style_attribute(&mut span_props, key, &val);
            }
          }
          b"br" if in_p => {
            current_text.push('\n');
          }
          _ => {}
        }
      }
      Ok(Event::Empty(ref e)) => {
        let tag = local_name(e.name().as_ref()).to_vec();
        if tag.as_slice() == b"br" && in_p {
          current_text.push('\n');
        } else if tag.as_slice() == b"style" {
          parse_style_tag(e, &mut styles);
        } else if tag.as_slice() == b"p" {
          let attrs = read_paragraph_attrs(e, &styles);
          if let (Some(s), Some(e)) = (attrs.start, attrs.end) {
            let mut sub = Subtitle::new(s, e, "");
            sub.style = attrs.style;
            if !attrs.props.is_default() {
              sub.style_props = Some(attrs.props);
            }
            subtitles.push(sub);
          }
        }
      }
      Ok(Event::Text(ref e)) => {
        let text = e.decode().map_err(|e| SubtitleError::Xml {
          format: Format::Ttml,
          error: e.to_string(),
        })?;
        if in_p && !text.trim().is_empty() {
          let segment = text.to_string();
          current_text.push_str(&segment);
          if in_span || !span_props.is_default() {
            let mut part = TextPart::new(
              &segment,
              span_props.bold,
              span_props.italic,
              span_props.underline,
            );
            part.color = span_props.color.clone();
            parts.push(part);
          }
        }
      }
      Ok(Event::End(ref e)) => {
        let tag = local_name(e.name().as_ref()).to_vec();
        match tag.as_slice() {
          b"p" => {
            if let (Some(start), Some(end)) = (current_start, current_end) {
              let mut sub = Subtitle::new(start, end, &current_text);
              sub.text_parts = std::mem::take(&mut parts);
              sub.style = current_style.take();
              let props = std::mem::take(&mut current_props);
              if !props.is_default() {
                sub.style_props = Some(props);
              }
              subtitles.push(sub);
            }
            in_p = false;
            current_start = None;
            current_end = None;
          }
          b"span" => {
            in_span = false;
            span_props = StyleProps::default();
          }
          _ => {}
        }
      }
      Ok(Event::Eof) => break,
      Err(e) => {
        return Err(
          SubtitleError::Xml {
            format: Format::Ttml,
            error: e.to_string(),
          }
          .into(),
        );
      }
      _ => {}
    }
    buf.clear();
  }

  Ok(SubtitleFile::Ttml {
    header: None,
    subtitles,
  })
}

/// Parse TTML from a byte slice.
pub fn parse_bytes(data: &[u8]) -> AnyResult<SubtitleFile> {
  let text = std::str::from_utf8(data).map_err(|e| SubtitleError::InvalidEncoding {
    encoding: "UTF-8".to_string(),
    error: e.to_string(),
  })?;
  parse_content(text)
}

/// Parse a TTML file asynchronously.
#[cfg(not(target_arch = "wasm32"))]
pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<SubtitleFile> {
  let text = tokio::fs::read_to_string(path).await?;
  parse_content(&text)
}

/// Parse a TTML file from a URL (requires `http` feature).
#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<SubtitleFile> {
  let response = reqwest::get(url).await?;
  let content = response.text().await?;
  parse_content(&content)
}

/// Detect if data looks like TTML (contains `<tt` root element).
pub fn detect_format(data: &[u8]) -> Option<crate::model::Format> {
  let text = crate::encoding::try_decode_for_detection(data)?;
  if text.contains("<tt") && text.contains("http://www.w3.org/ns/ttml") {
    return Some(crate::model::Format::Ttml);
  }
  None
}

/// Serialize subtitles to a minimal TTML document.
///
/// Write subtitles to a file in TTML format.
///
/// `policy` controls overwrite behavior (None = default Overwrite).
/// Omits the optional `<head>` block; to include one, call `to_string`
/// directly and write the result with `tokio::fs::write`.
#[cfg(not(target_arch = "wasm32"))]
pub async fn generate(
  subtitles: &[Subtitle],
  file_path: impl AsRef<std::path::Path>,
  policy: Option<crate::model::WritePolicy>,
) -> AnyResult<String> {
  let content = to_string(subtitles, None);
  let path = file_path.as_ref();
  crate::io::write_with_policy(path, content.as_bytes(), policy).await?;
  Ok(path.to_string_lossy().into_owned())
}

/// Serialize subtitles to TTML format.
///
/// `header`, if provided, is injected verbatim into a `<head>` block
/// between `<tt>` and `<body>`. The caller is responsible for ensuring
/// `header` is well-formed XML fragment (e.g. `<metadata>...</metadata>`).
/// The `<head>` block is also emitted when any subtitle carries
/// `style_props` — a `<styling>` block is generated from them and each
/// `<p>` references its style by id.
///
/// **Note**: the parse path does not yet round-trip the header back into
/// `SubtitleFile::Ttml { header, .. }` (it stays `None`). Round-trip
/// preservation is planned for a future release. For now, `header` is
/// write-only.
pub fn to_string(subtitles: &[Subtitle], header: Option<&str>) -> String {
  let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

  let _ = writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)));
  let tt = BytesStart::new("tt").with_attributes([
    ("xmlns", "http://www.w3.org/ns/ttml"),
    ("xmlns:tts", "http://www.w3.org/ns/ttml#styling"),
    ("xml:lang", "en"),
  ]);
  let _ = writer.write_event(Event::Start(tt));

  // Optional <head> block — inject header verbatim as escaped XML text.
  // BytesText::from_escaped prevents double-escaping of the caller's
  // already-formed XML fragment (e.g. "<metadata>...</metadata>").
  // Cue-level styles are collected from subtitle style_props, deduplicated
  // by (style name, props) in first-appearance order, and emitted as
  // <style> elements.
  let mut entries: Vec<(String, StyleProps)> = Vec::new();
  let mut sub_ids: Vec<Option<String>> = Vec::with_capacity(subtitles.len());
  for sub in subtitles {
    let Some(props) = sub.style_props.as_ref().filter(|p| !p.is_default()) else {
      sub_ids.push(None);
      continue;
    };
    // Reuse an existing style only when the cue's own style name matches
    // the name that entry was created from; identical props under different
    // names stay distinct so each cue keeps its style id on round-trip.
    let name = sub
      .style
      .as_deref()
      .map(|s| s.replace(char::is_whitespace, "_"));
    if let Some((id, _)) = entries
      .iter()
      .find(|(existing_id, p)| p == props && name.as_deref().is_none_or(|n| existing_id == n))
    {
      sub_ids.push(Some(id.clone()));
      continue;
    }

    // Style names from other formats (ASS, SubViewer) may contain spaces,
    // but TTML ids are NCNames and `style` refs are whitespace-separated
    // lists — so the emitted id must have whitespace mapped to '_'.
    let base = name
      .clone()
      .unwrap_or_else(|| format!("s{}", entries.len() + 1));
    let mut id = base.clone();
    let mut n = 2;

    // preventing collisions
    while entries.iter().any(|(existing, _)| existing == &id) {
      id = format!("{base}_{n}");
      n += 1;
    }

    entries.push((id.clone(), props.clone()));
    sub_ids.push(Some(id));
  }

  let header = header.filter(|s| !s.is_empty());
  if header.is_some() || !entries.is_empty() {
    let _ = writer.write_event(Event::Start(BytesStart::new("head")));
    if let Some(h) = header {
      let _ = writer.write_event(Event::Text(BytesText::from_escaped(h)));
    }
    if !entries.is_empty() {
      let _ = writer.write_event(Event::Start(BytesStart::new("styling")));
      for (id, props) in &entries {
        let mut style = BytesStart::new("style");
        style.push_attribute(("xml:id", id.as_str()));
        if let Some(ff) = &props.font_family {
          style.push_attribute(("tts:fontFamily", ff.as_str()));
        }
        if let Some(fs) = &props.font_size {
          style.push_attribute(("tts:fontSize", fs.as_str()));
        }
        if let Some(c) = &props.color {
          style.push_attribute(("tts:color", c.as_str()));
        }
        if props.bold {
          style.push_attribute(("tts:fontWeight", "bold"));
        }
        if props.italic {
          style.push_attribute(("tts:fontStyle", "italic"));
        }
        if props.underline {
          style.push_attribute(("tts:textDecoration", "underline"));
        }
        let _ = writer.write_event(Event::Empty(style));
      }
      let _ = writer.write_event(Event::End(BytesEnd::new("styling")));
    }
    let _ = writer.write_event(Event::End(BytesEnd::new("head")));
  }

  let _ = writer.write_event(Event::Start(BytesStart::new("body")));
  let _ = writer.write_event(Event::Start(BytesStart::new("div")));

  for (sub, style_id) in subtitles.iter().zip(&sub_ids) {
    let start = crate::utils::format_timestamp(sub.start, "WebVTT");
    let end = crate::utils::format_timestamp(sub.end, "WebVTT");
    // TTML uses '.' separator (same as WebVTT), no conversion needed

    let mut p = BytesStart::new("p");
    p.push_attribute(("begin", start.as_str()));
    p.push_attribute(("end", end.as_str()));
    if let Some(id) = style_id {
      p.push_attribute(("style", id.as_str()));
    }
    let _ = writer.write_event(Event::Start(p));

    if sub.text_parts.is_empty() {
      let _ = writer.write_event(Event::Text(BytesText::new(&sub.text)));
    } else {
      for part in &sub.text_parts {
        if part.color.is_some() || part.bold() || part.italic() || part.underline() {
          let mut span = BytesStart::new("span");
          if let Some(ref color) = part.color {
            span.push_attribute(("tts:color", color.as_str()));
          }
          if part.bold() {
            span.push_attribute(("tts:fontWeight", "bold"));
          }
          if part.italic() {
            span.push_attribute(("tts:fontStyle", "italic"));
          }
          if part.underline() {
            span.push_attribute(("tts:textDecoration", "underline"));
          }
          let _ = writer.write_event(Event::Start(span));
          let _ = writer.write_event(Event::Text(BytesText::new(&part.text)));
          let _ = writer.write_event(Event::End(BytesEnd::new("span")));
        } else {
          let _ = writer.write_event(Event::Text(BytesText::new(&part.text)));
        }
      }
    }
    let _ = writer.write_event(Event::End(BytesEnd::new("p")));
  }

  let _ = writer.write_event(Event::End(BytesEnd::new("div")));
  let _ = writer.write_event(Event::End(BytesEnd::new("body")));
  let _ = writer.write_event(Event::End(BytesEnd::new("tt")));

  String::from_utf8(writer.into_inner().into_inner()).unwrap_or_else(|e| {
    // quick-xml Writer produces bytes from &str input, so UTF-8 is always
    // valid in practice. If it somehow fails, log and return an empty string.
    tracing::warn!(error = %e, "TTML writer produced invalid UTF-8");
    String::new()
  })
}

/// Stream TTML subtitles to an async writer.
///
/// Uses an internal in-memory buffer to bridge quick-xml's sync Writer
/// to tokio's AsyncWrite (writes the whole document in one chunk).
/// For true incremental streaming, a full async XML writer would be
/// needed — deferred to 3.0.
///
/// Prefer this over `write_stream` (deprecated) for new code.
#[cfg(not(target_arch = "wasm32"))]
pub async fn write_stream_async<W>(subtitles: &[Subtitle], writer: &mut W) -> AnyResult<()>
where
  W: tokio::io::AsyncWrite + Unpin + Send,
{
  use tokio::io::AsyncWriteExt;
  let mut buf = Vec::new();
  #[allow(deprecated)]
  write_stream(subtitles, &mut buf)?;
  writer.write_all(&buf).await?;
  writer.flush().await?;
  Ok(())
}

/// Write TTML subtitles to a synchronous writer streamingly.
/// Note: TTML uses quick-xml which requires std::io::Write, not AsyncWrite.
#[deprecated(since = "2.2.0", note = "use write_stream_async instead")]
pub fn write_stream<W: std::io::Write>(subtitles: &[Subtitle], writer: &mut W) -> AnyResult<()> {
  let mut xml_writer = Writer::new_with_indent(writer, b' ', 2);

  xml_writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
  let tt = BytesStart::new("tt").with_attributes([
    ("xmlns", "http://www.w3.org/ns/ttml"),
    ("xmlns:tts", "http://www.w3.org/ns/ttml#styling"),
    ("xml:lang", "en"),
  ]);
  xml_writer.write_event(Event::Start(tt))?;
  xml_writer.write_event(Event::Start(BytesStart::new("body")))?;
  xml_writer.write_event(Event::Start(BytesStart::new("div")))?;

  for sub in subtitles {
    let start = crate::utils::format_timestamp(sub.start, "WebVTT");
    let end = crate::utils::format_timestamp(sub.end, "WebVTT");

    let p =
      BytesStart::new("p").with_attributes([("begin", start.as_str()), ("end", end.as_str())]);
    xml_writer.write_event(Event::Start(p))?;

    if sub.text_parts.is_empty() {
      xml_writer.write_event(Event::Text(BytesText::new(&sub.text)))?;
    } else {
      for part in &sub.text_parts {
        if part.color.is_some() || part.bold() || part.italic() || part.underline() {
          let mut span = BytesStart::new("span");
          if let Some(ref color) = part.color {
            span.push_attribute(("tts:color", color.as_str()));
          }
          xml_writer.write_event(Event::Start(span))?;
          xml_writer.write_event(Event::Text(BytesText::new(&part.text)))?;
          xml_writer.write_event(Event::End(BytesEnd::new("span")))?;
        } else {
          xml_writer.write_event(Event::Text(BytesText::new(&part.text)))?;
        }
      }
    }
    xml_writer.write_event(Event::End(BytesEnd::new("p")))?;
  }

  xml_writer.write_event(Event::End(BytesEnd::new("div")))?;
  xml_writer.write_event(Event::End(BytesEnd::new("body")))?;
  xml_writer.write_event(Event::End(BytesEnd::new("tt")))?;

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::SubtitleFormat;

  const SAMPLE_TTML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling" xml:lang="en">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.500">Hello World</p>
      <p begin="00:00:04.000" end="00:00:06.500"><span tts:color="yellow">Colored</span> text</p>
    </div>
  </body>
</tt>"#;

  #[test]
  fn test_parse_basic() {
    let subs = parse_content(SAMPLE_TTML).unwrap();
    assert_eq!(subs.subtitles().len(), 2);
    assert_eq!(subs.subtitles()[0].start, 1000);
    assert_eq!(subs.subtitles()[0].end, 3500);
    assert_eq!(subs.subtitles()[0].text, "Hello World");
    assert_eq!(subs.subtitles()[1].start, 4000);
    assert_eq!(subs.subtitles()[1].end, 6500);
    assert_eq!(subs.subtitles()[1].text, "Colored text");
    assert_eq!(subs.subtitles()[1].text_parts.len(), 1);
    assert_eq!(
      subs.subtitles()[1].text_parts[0].color,
      Some("yellow".to_string())
    );
  }

  #[test]
  fn test_round_trip() {
    let subs = parse_content(SAMPLE_TTML).unwrap();
    let output = to_string(subs.subtitles(), None);
    assert!(output.contains("<p"));
    assert!(output.contains("begin=\"00:00:01.000\""));
    assert!(output.contains("end=\"00:00:03.500\""));
    let reparsed = parse_content(&output).unwrap();
    assert_eq!(subs.subtitles().len(), reparsed.subtitles().len());
    assert_eq!(subs.subtitles()[0].start, reparsed.subtitles()[0].start);
  }

  #[test]
  fn test_detect() {
    assert!(detect_format(b"<tt xmlns='http://www.w3.org/ns/ttml'>").is_some());
    assert!(detect_format(b"WEBVTT").is_none());
  }

  #[test]
  fn test_parse_br_tag() {
    let xml = r#"<?xml version="1.0"?>
<tt xmlns="http://www.w3.org/ns/ttml"><body><div>
<p begin="00:00:01.000" end="00:00:03.500">Line one<br/>Line two</p>
</div></body></tt>"#;
    let subs = parse_content(xml).unwrap();
    assert_eq!(subs.subtitles().len(), 1);
    assert_eq!(subs.subtitles()[0].text, "Line one\nLine two");
  }

  #[test]
  fn test_parse_dur_attribute() {
    let xml = r#"<?xml version="1.0"?>
<tt xmlns="http://www.w3.org/ns/ttml"><body><div>
<p begin="00:00:01.000" dur="2.5s">Duration test</p>
</div></body></tt>"#;
    let subs = parse_content(xml).unwrap();
    assert_eq!(subs.subtitles().len(), 1);
    assert_eq!(subs.subtitles()[0].start, 1000);
    assert_eq!(subs.subtitles()[0].end, 3500);
  }

  #[test]
  fn test_parse_font_style() {
    let xml = r#"<?xml version="1.0"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling"><body><div>
<p begin="00:00:01.000" end="00:00:03.500"><span tts:fontStyle="italic" tts:fontWeight="bold">Styled</span></p>
</div></body></tt>"#;
    let subs = parse_content(xml).unwrap();
    assert_eq!(subs.subtitles().len(), 1);
    assert_eq!(subs.subtitles()[0].text_parts.len(), 1);
    assert!(subs.subtitles()[0].text_parts[0].italic());
    assert!(subs.subtitles()[0].text_parts[0].bold());
  }

  #[test]
  fn test_parse_seconds_format() {
    assert_eq!(ttml_to_ms("5s"), Some(5000));
    assert_eq!(ttml_to_ms("2.5s"), Some(2500));
    assert_eq!(ttml_to_ms("00:00:05.000"), Some(5000));
  }

  #[test]
  fn test_ttml_header_preserved_in_output() {
    let subs = vec![Subtitle::new(1000, 2000, "hi")];
    // No header: output has no <head> block
    let no_hdr = to_string(&subs, None);
    assert!(
      !no_hdr.contains("<head>"),
      "expected no <head> when header=None, got: {}",
      no_hdr
    );
    // With header: output contains <head>...</head> wrapping the fragment
    let with_hdr = to_string(&subs, Some("<metadata>title=Hello</metadata>"));
    assert!(
      with_hdr.contains("<head>") && with_hdr.contains("</head>"),
      "expected <head> block in output, got: {}",
      with_hdr
    );
    assert!(
      with_hdr.contains("<metadata>title=Hello</metadata>"),
      "expected header fragment verbatim in output, got: {}",
      with_hdr
    );
    // Header placement: <head> comes after <tt> and before <body>
    let tt_idx = with_hdr.find("<tt").unwrap();
    let head_idx = with_hdr.find("<head>").unwrap();
    let body_idx = with_hdr.find("<body").unwrap();
    assert!(tt_idx < head_idx, "<head> must come after <tt>");
    assert!(head_idx < body_idx, "<head> must come before <body>");
  }

  #[test]
  fn test_ttml_empty_header_omitted() {
    let subs = vec![Subtitle::new(1000, 2000, "hi")];
    // Empty string header is treated as None
    let out = to_string(&subs, Some(""));
    assert!(
      !out.contains("<head>"),
      "empty header should be omitted, got: {}",
      out
    );
  }

  #[test]
  fn test_malformed_xml_graceful_degradation() {
    // TODO(3.0): truncated XML should return a typed error. Currently it
    // returns Ok with empty subtitles (quick-xml handles malformed input
    // gracefully internally).
    let result = parse_content(
      "<?xml version=\"1.0\"?><tt><body><div><p begin=\"00:00:01.000\" end=\"00:00:03.000\">Hello",
    );
    assert!(
      result.is_ok(),
      "malformed XML should not crash (current behavior)"
    );
  }

  #[test]
  fn test_smpte_frame_timecode() {
    // iTT-style SMPTE timecode: HH:MM:SS:FF at 29.97fps
    // 00:00:13:07 → 13*30 + 7 = 397 frames; 397/29.97*1000 ≈ 13247ms
    let ms = ttml_to_ms("00:00:13:07").unwrap();
    assert!(ms > 13_000 && ms < 14_000, "got {}", ms);
    // 01:00:00:00 → 3600*30 = 108000 frames = 3603604ms (non-drop)
    let ms = ttml_to_ms("01:00:00:00").unwrap();
    assert!(ms > 3_600_000 && ms < 3_610_000, "got {}", ms);
  }

  #[test]
  fn test_parse_style_elements_resolve_on_p() {
    let content = "<tt xmlns=\"http://www.w3.org/ns/ttml\" xmlns:tts=\"http://www.w3.org/ns/ttml#styling\">\
      <head><styling>\
      <style xml:id=\"base\" tts:fontFamily=\"Arial\" tts:fontSize=\"48px\"/>\
      <style xml:id=\"em\" style=\"base\" tts:fontStyle=\"italic\" tts:color=\"#FF0000\"/>\
      </styling></head>\
      <body><div>\
      <p begin=\"00:00:01.000\" end=\"00:00:02.000\" style=\"em\">Hello</p>\
      </div></body></tt>";
    let file = parse_content(content).unwrap();
    let sub = &file.subtitles()[0];
    assert_eq!(sub.style.as_deref(), Some("em"));
    let props = sub.style_props.as_ref().unwrap();
    // inherited from base
    assert_eq!(props.font_family.as_deref(), Some("Arial"));
    assert_eq!(props.font_size.as_deref(), Some("48px"));
    // own props
    assert!(props.italic);
    assert!(!props.bold);
    assert_eq!(props.color.as_deref(), Some("#FF0000"));
  }

  #[test]
  fn test_parse_p_direct_attrs_override_referenced_style() {
    let content = "<tt xmlns=\"http://www.w3.org/ns/ttml\" xmlns:tts=\"http://www.w3.org/ns/ttml#styling\">\
      <head><styling>\
      <style xml:id=\"base\" tts:fontFamily=\"Arial\" tts:fontSize=\"48px\"/>\
      </styling></head>\
      <body><div>\
      <p begin=\"00:00:01.000\" end=\"00:00:02.000\" style=\"base\" tts:fontSize=\"24px\">Hi</p>\
      </div></body></tt>";
    let file = parse_content(content).unwrap();
    let props = file.subtitles()[0].style_props.as_ref().unwrap();
    assert_eq!(props.font_family.as_deref(), Some("Arial"));
    assert_eq!(props.font_size.as_deref(), Some("24px"));
  }

  #[test]
  fn test_parse_span_text_decoration_underline() {
    let content = "<tt xmlns=\"http://www.w3.org/ns/ttml\" xmlns:tts=\"http://www.w3.org/ns/ttml#styling\">\
      <body><div>\
      <p begin=\"00:00:01.000\" end=\"00:00:02.000\"><span tts:textDecoration=\"underline\">U</span></p>\
      </div></body></tt>";
    let file = parse_content(content).unwrap();
    let parts = &file.subtitles()[0].text_parts;
    assert_eq!(parts.len(), 1);
    assert!(parts[0].underline());
  }

  #[test]
  fn test_parse_self_closing_p_carries_style() {
    let content = "<tt xmlns=\"http://www.w3.org/ns/ttml\" xmlns:tts=\"http://www.w3.org/ns/ttml#styling\">\
      <head><styling>\
      <style xml:id=\"s1\" tts:fontFamily=\"Courier\"/>\
      </styling></head>\
      <body><div>\
      <p begin=\"00:00:01.000\" end=\"00:00:02.000\" style=\"s1\"/>\
      </div></body></tt>";
    let file = parse_content(content).unwrap();
    let sub = &file.subtitles()[0];
    assert_eq!(sub.style.as_deref(), Some("s1"));
    assert_eq!(
      sub.style_props.as_ref().unwrap().font_family.as_deref(),
      Some("Courier")
    );
  }

  #[test]
  fn test_write_styling_block_and_p_refs() {
    let props = StyleProps {
      font_family: Some("Arial".into()),
      bold: true,
      ..StyleProps::default()
    };
    let subs = vec![
      Subtitle::new(1000, 2000, "a")
        .with_style("Custom")
        .with_style_props(props.clone()),
      // Same name + props: must reuse a single <style> entry.
      Subtitle::new(3000, 4000, "b")
        .with_style("Custom")
        .with_style_props(props),
    ];
    let out = to_string(&subs, None);
    assert!(out.contains("<styling>"), "got: {}", out);
    assert_eq!(out.matches("xml:id=\"Custom\"").count(), 1, "got: {}", out);
    assert!(out.contains("tts:fontFamily=\"Arial\""), "got: {}", out);
    assert!(out.contains("tts:fontWeight=\"bold\""), "got: {}", out);
    assert_eq!(out.matches("style=\"Custom\"").count(), 2, "got: {}", out);
  }

  #[test]
  fn test_write_dedup_keeps_distinct_style_names() {
    // Identical props under different style names must not collapse to one
    // id: each cue keeps its own style name across a round-trip.
    let props = StyleProps {
      font_family: Some("Arial".into()),
      bold: true,
      ..StyleProps::default()
    };
    let subs = vec![
      Subtitle::new(1000, 2000, "a")
        .with_style("Red")
        .with_style_props(props.clone()),
      Subtitle::new(3000, 4000, "b")
        .with_style("Crimson")
        .with_style_props(props),
    ];
    let out = to_string(&subs, None);
    assert!(out.contains("style=\"Red\""), "got: {}", out);
    assert!(out.contains("style=\"Crimson\""), "got: {}", out);
    let reparsed = parse_content(&out).unwrap();
    let subs = reparsed.subtitles();
    assert_eq!(
      subs[0].style.as_deref(),
      Some("Red"),
      "got: {:?}",
      subs[0].style
    );
    assert_eq!(
      subs[1].style.as_deref(),
      Some("Crimson"),
      "got: {:?}",
      subs[1].style
    );
    assert_eq!(subs[0].style_props, subs[1].style_props);
  }

  #[test]
  fn test_write_style_name_with_space_is_sanitized() {
    // Style names from other formats (ASS, SubViewer) may contain spaces;
    // TTML xml:id/style refs are whitespace-separated lists, so the
    // serializer must not emit the raw name.
    let props = StyleProps {
      bold: true,
      ..StyleProps::default()
    };
    let subs = vec![
      Subtitle::new(1000, 2000, "a")
        .with_style("Custom Style")
        .with_style_props(props),
    ];
    let out = to_string(&subs, None);
    assert!(out.contains("xml:id=\"Custom_Style\""), "got: {}", out);
    assert!(out.contains("style=\"Custom_Style\""), "got: {}", out);
    assert!(!out.contains("xml:id=\"Custom Style\""), "got: {}", out);
    assert!(!out.contains("style=\"Custom Style\""), "got: {}", out);
    // Sanitized document still resolves the style on reparse.
    let reparsed = parse_content(&out).unwrap();
    assert_eq!(
      reparsed.subtitles()[0].style.as_deref(),
      Some("Custom_Style")
    );
    assert!(reparsed.subtitles()[0].style_props.as_ref().unwrap().bold);
  }

  #[test]
  fn test_write_span_bold_italic_underline() {
    let mut sub = Subtitle::new(1000, 2000, "x");
    sub.text_parts.push(TextPart::new("x", true, true, true));
    let out = to_string(&[sub], None);
    assert!(out.contains("tts:fontWeight=\"bold\""), "got: {}", out);
    assert!(out.contains("tts:fontStyle=\"italic\""), "got: {}", out);
    assert!(
      out.contains("tts:textDecoration=\"underline\""),
      "got: {}",
      out
    );
  }

  #[test]
  fn test_style_roundtrip() {
    let content = "<tt xmlns=\"http://www.w3.org/ns/ttml\" xmlns:tts=\"http://www.w3.org/ns/ttml#styling\">\
      <head><styling>\
      <style xml:id=\"base\" tts:fontFamily=\"Arial\" tts:fontSize=\"48px\"/>\
      <style xml:id=\"em\" style=\"base\" tts:fontStyle=\"italic\"/>\
      </styling></head>\
      <body><div>\
      <p begin=\"00:00:01.000\" end=\"00:00:02.000\" style=\"em\">Hello</p>\
      <p begin=\"00:00:03.000\" end=\"00:00:04.000\" style=\"base\">World</p>\
      <p begin=\"00:00:05.000\" end=\"00:00:06.000\">Plain</p>\
      </div></body></tt>";
    let first = parse_content(content).unwrap();
    let out = first.to_string();
    assert!(out.contains("<styling>"), "got: {}", out);
    let second = parse_content(&out).unwrap();
    assert_eq!(first.subtitles(), second.subtitles());
  }
}
