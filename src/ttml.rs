//! TTML (Timed Text Markup Language) subtitle parser and generator.
//!
//! Parses `begin`/`end` timed `<p>` elements with inline `<span>` styling.
//! Supports the IMSC 1.0/1.1 profile common in streaming (Netflix, etc.).
//!
//! Uses `quick-xml` for streaming pull parsing — no DOM build.

use crate::model::{Subtitle, TextPart};
use crate::types::AnyResult;
use crate::utils::parse_timestamp;
use anyhow::anyhow;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::io::Cursor;

fn ttml_to_ms(attr: &str) -> Option<u64> {
  // Accept hh:mm:ss.mmm or hh:mm:ss,mmm or offset format
  if attr.contains(':') {
    return parse_timestamp(attr).ok();
  }
  // Could add frame-based parsing here later
  None
}

/// Parse TTML content into a vector of subtitles.
pub fn parse_content(content: &str) -> AnyResult<Vec<Subtitle>> {
  let mut reader = Reader::from_str(content);
  reader.config_mut().trim_text(false);
  let mut buf = Vec::new();

  let mut subtitles = Vec::new();
  let mut in_p = false;
  let mut current_start: Option<u64> = None;
  let mut current_end: Option<u64> = None;
  let mut current_text = String::new();
  let mut parts: Vec<TextPart> = Vec::new();
  let mut in_span = false;
  // Track span-level styling (simplified: only tts:color for now)
  let mut span_color: Option<String> = None;

  loop {
    match reader.read_event_into(&mut buf) {
      Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
        let tag = e.name().as_ref().to_vec();
        match tag.as_slice() {
          b"p" | b"tt:p" => {
            in_p = true;
            current_text.clear();
            parts.clear();
            // Parse begin/end
            if let Some(b) = e
              .attributes()
              .flatten()
              .find(|a| a.key.as_ref() == b"begin" || a.key.as_ref() == b"tts:begin")
            {
              current_start = ttml_to_ms(&String::from_utf8_lossy(&b.value));
            }
            if let Some(e_attr) = e
              .attributes()
              .flatten()
              .find(|a| a.key.as_ref() == b"end" || a.key.as_ref() == b"tts:end")
            {
              current_end = ttml_to_ms(&String::from_utf8_lossy(&e_attr.value));
            }
          }
          b"span" | b"tt:span" => {
            in_span = true;
            span_color = e
              .attributes()
              .flatten()
              .find(|a| {
                let k = a.key.as_ref();
                k == b"tts:color" || k == b"color"
              })
              .map(|a| String::from_utf8_lossy(&a.value).to_string());
          }
          _ => {}
        }
      }
      Ok(Event::Text(ref e)) => {
        let text = e.unescape()?;
        if in_p && !text.trim().is_empty() {
          if in_span && !text.trim().is_empty() {
            let segment = text.to_string();
            current_text.push_str(&segment);
            parts.push(TextPart {
              text: segment,
              bold: false,
              italic: false,
              underline: false,
              color: span_color.clone(),
              voice: None,
            });
          } else if !in_span && !text.trim().is_empty() {
            let segment = text.to_string();
            current_text.push_str(&segment);
            parts.push(TextPart::plain(segment));
          }
        }
      }
      Ok(Event::End(ref e)) => {
        let tag = e.name().as_ref().to_vec();
        match tag.as_slice() {
          b"p" | b"tt:p" => {
            if let (Some(start), Some(end)) = (current_start, current_end) {
              let mut sub = Subtitle::new(start, end, &current_text);
              sub.text_parts = std::mem::take(&mut parts);
              subtitles.push(sub);
            }
            in_p = false;
            current_start = None;
            current_end = None;
          }
          b"span" | b"tt:span" => {
            in_span = false;
            span_color = None;
          }
          _ => {}
        }
      }
      Ok(Event::Eof) => break,
      Err(e) => return Err(anyhow!("TTML parse error: {}", e)),
      _ => {}
    }
    buf.clear();
  }

  Ok(subtitles)
}

/// Parse TTML from a byte slice.
pub fn parse_bytes(data: &[u8]) -> AnyResult<Vec<Subtitle>> {
  let text = std::str::from_utf8(data).map_err(|e| anyhow!("Invalid UTF-8 in TTML: {}", e))?;
  parse_content(text)
}

/// Parse a TTML file asynchronously.
pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<Vec<Subtitle>> {
  let text = tokio::fs::read_to_string(path).await?;
  parse_content(&text)
}

/// Parse a TTML file from a URL (requires `http` feature).
#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<Vec<Subtitle>> {
  let response = reqwest::get(url).await?;
  let content = response.text().await?;
  parse_content(&content)
}

/// Detect if data looks like TTML (contains `<tt` root element).
pub fn detect_format(data: &[u8]) -> Option<crate::model::Format> {
  let text = std::str::from_utf8(data).ok()?;
  if text.contains("<tt")
    && (text.contains("http://www.w3.org/ns/ttml")
      || text.contains("http://www.w3.org/2006/10/ttaf1"))
  {
    return Some(crate::model::Format::Ttml);
  }
  None
}

/// Serialize subtitles to a minimal TTML document.
pub fn to_string(subtitles: &[Subtitle], _header: Option<&str>) -> String {
  let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

  let _ = writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)));
  let tt = BytesStart::new("tt").with_attributes([
    ("xmlns", "http://www.w3.org/ns/ttml"),
    ("xmlns:tts", "http://www.w3.org/ns/ttml#styling"),
    ("xml:lang", "en"),
  ]);
  let _ = writer.write_event(Event::Start(tt));
  let _ = writer.write_event(Event::Start(BytesStart::new("body")));
  let _ = writer.write_event(Event::Start(BytesStart::new("div")));

  for sub in subtitles {
    let start = crate::utils::format_timestamp(sub.start, "WebVTT");
    let end = crate::utils::format_timestamp(sub.end, "WebVTT");
    // Convert '.' separator to ',' for TTML
    let start_ttml = start.replace('.', ",");
    let end_ttml = end.replace('.', ",");

    let p = BytesStart::new("p")
      .with_attributes([("begin", start_ttml.as_str()), ("end", end_ttml.as_str())]);
    let _ = writer.write_event(Event::Start(p));

    if sub.text_parts.is_empty() {
      let _ = writer.write_event(Event::Text(BytesText::new(&sub.text)));
    } else {
      for part in &sub.text_parts {
        if part.color.is_some() || part.bold || part.italic || part.underline {
          let mut span = BytesStart::new("span");
          if let Some(ref color) = part.color {
            span.push_attribute(("tts:color", color.as_str()));
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

  String::from_utf8(writer.into_inner().into_inner())
    .expect("TTML writer always produces valid UTF-8")
}

#[cfg(test)]
mod tests {
  use super::*;

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
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0].start, 1000);
    assert_eq!(subs[0].end, 3500);
    assert_eq!(subs[0].text, "Hello World");
    assert_eq!(subs[1].start, 4000);
    assert_eq!(subs[1].end, 6500);
    assert_eq!(subs[1].text, "Colored text");
    assert_eq!(subs[1].text_parts.len(), 2);
    assert_eq!(subs[1].text_parts[0].color, Some("yellow".to_string()));
  }

  #[test]
  fn test_round_trip() {
    let subs = parse_content(SAMPLE_TTML).unwrap();
    let output = to_string(&subs, None);
    assert!(output.contains("<p"));
    assert!(output.contains("begin=\"00:00:01,000\""));
    assert!(output.contains("end=\"00:00:03,500\""));
    // Re-parse
    let reparsed = parse_content(&output).unwrap();
    assert_eq!(subs.len(), reparsed.len());
    assert_eq!(subs[0].start, reparsed[0].start);
  }

  #[test]
  fn test_detect() {
    assert!(detect_format(b"<tt xmlns='http://www.w3.org/ns/ttml'>").is_some());
    assert!(detect_format(b"WEBVTT").is_none());
  }
}
