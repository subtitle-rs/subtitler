//! TTML (Timed Text Markup Language) subtitle parser and generator.
//!
//! Parses `begin`/`end` timed `<p>` elements with inline `<span>` styling.
//! Supports the IMSC 1.0/1.1 profile common in streaming (Netflix, etc.).
//!
//! Uses `quick-xml` for streaming pull parsing — no DOM build.

use crate::error::SubtitleError;
use crate::model::{Format, Subtitle, SubtitleFile, TextPart};
use crate::types::AnyResult;
use crate::utils::parse_timestamp;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use smallvec::SmallVec;
use std::io::Cursor;

/// Parse a TTML time value to milliseconds.
/// Supports: `hh:mm:ss.mmm`, `hh:mm:ss,mmm`, or seconds like `12.5s`.
fn ttml_to_ms(attr: &str) -> Option<u64> {
  let attr = attr.trim();
  if attr.contains(':') {
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

/// Parse TTML content into a SubtitleFile.
pub fn parse_content(content: &str) -> AnyResult<SubtitleFile> {
  let mut reader = Reader::from_str(content);
  reader.config_mut().trim_text(false);
  let mut buf = Vec::new();

  let mut subtitles: Vec<Subtitle> = Vec::with_capacity((content.len() / 300).max(16));
  let mut in_p = false;
  let mut current_start: Option<u64> = None;
  let mut current_end: Option<u64> = None;
  let mut current_text = String::new();
  let mut parts: SmallVec<[TextPart; 4]> = SmallVec::new();
  let mut in_span = false;
  let mut span_bold = false;
  let mut span_italic = false;
  let mut span_color: Option<String> = None;

  loop {
    match reader.read_event_into(&mut buf) {
      Ok(Event::Start(ref e)) => {
        let tag = local_name(e.name().as_ref()).to_vec();
        match tag.as_slice() {
          b"p" => {
            in_p = true;
            current_text.clear();
            parts.clear();
            current_start = None;
            current_end = None;
            for attr in e.attributes().flatten() {
              let key = local_name(attr.key.as_ref());
              let val = String::from_utf8_lossy(&attr.value);
              match key {
                b"begin" => current_start = ttml_to_ms(&val),
                b"end" => current_end = ttml_to_ms(&val),
                b"dur" if current_end.is_none() => {
                  if let (Some(s), Some(d)) = (current_start, ttml_to_ms(&val)) {
                    current_end = Some(s + d);
                  }
                }
                _ => {}
              }
            }
          }
          b"span" => {
            in_span = true;
            span_bold = false;
            span_italic = false;
            span_color = None;
            for attr in e.attributes().flatten() {
              let key = local_name(attr.key.as_ref());
              let val = String::from_utf8_lossy(&attr.value);
              match key {
                b"color" => span_color = Some(val.to_string()),
                b"fontWeight" => span_bold = val == "bold",
                b"fontStyle" => span_italic = val == "italic",
                _ => {}
              }
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
        } else if tag.as_slice() == b"p" {
          // Self-closing <p/> — parse begin/end/dur from attributes
          let mut start = None;
          let mut end = None;
          for attr in e.attributes().flatten() {
            let key = local_name(attr.key.as_ref());
            let val = String::from_utf8_lossy(&attr.value);
            match key {
              b"begin" => start = ttml_to_ms(&val),
              b"end" => end = ttml_to_ms(&val),
              b"dur" if end.is_none() => {
                if let (Some(s), Some(d)) = (start, ttml_to_ms(&val)) {
                  end = Some(s + d);
                }
              }
              _ => {}
            }
          }
          if let (Some(s), Some(e)) = (start, end) {
            subtitles.push(Subtitle::new(s, e, ""));
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
          if in_span || span_bold || span_italic || span_color.is_some() {
            let mut part = TextPart::new(&segment, span_bold, span_italic, false);
            part.color = span_color.clone();
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
              subtitles.push(sub);
            }
            in_p = false;
            current_start = None;
            current_end = None;
          }
          b"span" => {
            in_span = false;
            span_bold = false;
            span_italic = false;
            span_color = None;
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
  if text.contains("<tt")
    && (text.contains("http://www.w3.org/ns/ttml")
      || text.contains("http://www.w3.org/2006/10/ttaf1"))
  {
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
/// `None` omits the `<head>` block entirely.
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
  if let Some(h) = header.filter(|s| !s.is_empty()) {
    let _ = writer.write_event(Event::Start(BytesStart::new("head")));
    let _ = writer.write_event(Event::Text(BytesText::from_escaped(h)));
    let _ = writer.write_event(Event::End(BytesEnd::new("head")));
  }

  let _ = writer.write_event(Event::Start(BytesStart::new("body")));
  let _ = writer.write_event(Event::Start(BytesStart::new("div")));

  for sub in subtitles {
    let start = crate::utils::format_timestamp(sub.start, "WebVTT");
    let end = crate::utils::format_timestamp(sub.end, "WebVTT");
    // TTML uses '.' separator (same as WebVTT), no conversion needed

    let p =
      BytesStart::new("p").with_attributes([("begin", start.as_str()), ("end", end.as_str())]);
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
    let result = parse_content("<?xml version=\"1.0\"?><tt><body><div><p begin=\"00:00:01.000\" end=\"00:00:03.000\">Hello");
    assert!(result.is_ok(), "malformed XML should not crash (current behavior)");
  }
}
