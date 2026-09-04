//! TTML (Timed Text Markup Language) subtitle parser and generator.
//!
//! Parses `begin`/`end` timed `<p>` elements with inline `<span>` styling.
//! Supports the IMSC 1.0/1.1 profile common in streaming (Netflix, etc.).
//!
//! Uses `quick-xml` for streaming pull parsing — no DOM build.

use crate::error::SubtitleError;
use crate::model::{
  CuePosition, Format, HorizontalAlign, StyleProps, Subtitle, SubtitleFile, TextPart, VerticalAlign,
};
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
  bold: Option<bool>,
  italic: Option<bool>,
  underline: Option<bool>,
  text_align: Option<HorizontalAlign>,
}

/// Raw `<region>` definition from `<layout>`.
#[derive(Default)]
struct RawRegion {
  x: Option<f64>,
  y: Option<f64>,
  w: Option<f64>,
  h: Option<f64>,
  display_align: Option<VerticalAlign>,
  text_align: Option<HorizontalAlign>,
}

fn parse_text_align(val: &str) -> Option<HorizontalAlign> {
  match val {
    "left" | "start" => Some(HorizontalAlign::Left),
    "right" | "end" => Some(HorizontalAlign::Right),
    "center" => Some(HorizontalAlign::Center),
    _ => None,
  }
}

fn parse_display_align(val: &str) -> Option<VerticalAlign> {
  match val {
    "before" => Some(VerticalAlign::Top),
    "center" => Some(VerticalAlign::Center),
    "after" => Some(VerticalAlign::Bottom),
    _ => None,
  }
}

fn round2(v: f64) -> f64 {
  (v * 100.0).round() / 100.0
}

fn parse_pct_pair(val: &str) -> (Option<f64>, Option<f64>) {
  let pct = |s: Option<&str>| {
    s.and_then(|s| s.strip_suffix('%'))
      .and_then(|n| n.parse::<f64>().ok())
  };
  let mut parts = val.split_whitespace();
  (pct(parts.next()), pct(parts.next()))
}

fn parse_region_tag(e: &BytesStart, regions: &mut HashMap<String, RawRegion>) {
  let mut id = None;
  let mut region = RawRegion::default();
  for attr in e.attributes().flatten() {
    let key = local_name(attr.key.as_ref());
    let val = String::from_utf8_lossy(&attr.value);
    match key {
      b"id" => id = Some(val.into_owned()),
      b"origin" => {
        let (x, y) = parse_pct_pair(&val);
        region.x = x;
        region.y = y;
      }
      b"extent" => {
        let (w, h) = parse_pct_pair(&val);
        region.w = w;
        region.h = h;
      }
      b"displayAlign" => region.display_align = parse_display_align(&val),
      b"textAlign" => region.text_align = parse_text_align(&val),
      _ => {}
    }
  }
  if let Some(id) = id {
    regions.insert(id, region);
  }
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
  let mut raw = RawStyle::default();
  for attr in e.attributes().flatten() {
    let key = local_name(attr.key.as_ref());
    let val = String::from_utf8_lossy(&attr.value);
    match key {
      b"id" => id = Some(val.into_owned()),
      b"style" => raw.parents = val.split_whitespace().map(str::to_string).collect(),
      b"fontWeight" => raw.bold = Some(val == "bold"),
      b"fontStyle" => raw.italic = Some(val == "italic"),
      b"textDecoration" => raw.underline = Some(val.split_whitespace().any(|t| t == "underline")),
      b"textAlign" => raw.text_align = parse_text_align(&val),
      _ => merge_style_attribute(&mut raw.props, key, &val),
    }
  }
  if let Some(id) = id {
    styles.insert(id, raw);
  }
}

/// Style with inheritance applied. Boolean flags stay tri-state during
/// resolution so a later explicit `Some(false)` overrides an earlier
/// `Some(true)`; they collapse to concrete bools only at the end.
#[derive(Default)]
struct ResolvedStyle {
  props: StyleProps,
  bold: Option<bool>,
  italic: Option<bool>,
  underline: Option<bool>,
  text_align: Option<HorizontalAlign>,
}

impl ResolvedStyle {
  /// `other` wins: `Some` fields overwrite, tri-state flags overwrite if set.
  fn merge_from(&mut self, other: &ResolvedStyle) {
    self.props.merge_from(&other.props);
    if other.bold.is_some() {
      self.bold = other.bold;
    }
    if other.italic.is_some() {
      self.italic = other.italic;
    }
    if other.underline.is_some() {
      self.underline = other.underline;
    }
    if other.text_align.is_some() {
      self.text_align = other.text_align;
    }
  }

  fn into_props(self) -> StyleProps {
    let mut props = self.props;
    props.bold = self.bold.unwrap_or(false);
    props.italic = self.italic.unwrap_or(false);
    props.underline = self.underline.unwrap_or(false);
    props
  }
}

/// Resolve a style id to its effective props, walking parent references.
fn resolve_style(id: &str, styles: &HashMap<String, RawStyle>) -> ResolvedStyle {
  fn recurse(
    id: &str,
    styles: &HashMap<String, RawStyle>,
    visited: &mut Vec<String>,
  ) -> ResolvedStyle {
    let mut resolved = ResolvedStyle::default();
    if visited.iter().any(|v| v == id) {
      return resolved;
    }
    visited.push(id.to_string());
    if let Some(raw) = styles.get(id) {
      for parent in &raw.parents {
        resolved.merge_from(&recurse(parent, styles, visited));
      }
      resolved.props.merge_from(&raw.props);
      if raw.bold.is_some() {
        resolved.bold = raw.bold;
      }
      if raw.italic.is_some() {
        resolved.italic = raw.italic;
      }
      if raw.underline.is_some() {
        resolved.underline = raw.underline;
      }
      if raw.text_align.is_some() {
        resolved.text_align = raw.text_align;
      }
    }
    visited.pop();
    resolved
  }

  recurse(id, styles, &mut Vec::new())
}

fn sanitize_style_id(name: &str) -> String {
  let mut id: String = name
    .chars()
    .map(|c| {
      if c.is_alphanumeric() || matches!(c, '.' | '_' | '-') {
        c
      } else {
        '_'
      }
    })
    .collect();
  if id
    .chars()
    .next()
    .is_none_or(|c| !(c.is_alphabetic() || c == '_'))
  {
    id.insert_str(0, "s_");
  }
  id
}

struct ParagraphAttrs {
  start: Option<u64>,
  end: Option<u64>,
  style: Option<String>,
  props: StyleProps,
  position: Option<CuePosition>,
}

/// Read <p> attributes: timing, style references (resolved), and direct
/// tts:* overrides (applied in a second pass so they win over references).
/// Position comes from the referenced <region> plus textAlign, with
/// precedence p-attr > style > region for horizontal alignment.
fn read_paragraph_attrs(
  e: &BytesStart,
  styles: &HashMap<String, RawStyle>,
  regions: &HashMap<String, RawRegion>,
) -> ParagraphAttrs {
  let mut out = ParagraphAttrs {
    start: None,
    end: None,
    style: None,
    props: StyleProps::default(),
    position: None,
  };
  let mut style_text_align: Option<HorizontalAlign> = None;

  let mut resolved = ResolvedStyle::default();
  for attr in e.attributes().flatten() {
    if local_name(attr.key.as_ref()) == b"style" {
      let val = String::from_utf8_lossy(&attr.value);
      for id in val.split_whitespace() {
        if out.style.is_none() {
          out.style = Some(id.to_string());
        }

        resolved.merge_from(&resolve_style(id, styles));
        if style_text_align.is_none() {
          style_text_align = resolved.text_align;
        }
      }
    }
  }
  out.props = resolved.into_props();

  let mut region_id: Option<String> = None;
  let mut p_text_align: Option<HorizontalAlign> = None;
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
      b"region" => region_id = Some(val.into_owned()),
      b"textAlign" => p_text_align = parse_text_align(&val),
      _ => merge_style_attribute(&mut out.props, key, &val),
    }
  }

  let region = region_id.as_deref().and_then(|id| regions.get(id));
  let h_align = p_text_align
    .or(style_text_align)
    .or(region.and_then(|r| r.text_align));
  if region.is_some() || h_align.is_some() {
    let h = h_align.unwrap_or_default();
    let v = region.and_then(|r| r.display_align).unwrap_or_default();
    let h_frac = match h {
      HorizontalAlign::Left => 0.0,
      HorizontalAlign::Center => 0.5,
      HorizontalAlign::Right => 1.0,
    };
    let v_frac = match v {
      VerticalAlign::Top => 0.0,
      VerticalAlign::Center => 0.5,
      VerticalAlign::Bottom => 1.0,
    };
    let anchor = |origin: Option<f64>, size: Option<f64>, frac: f64| match (origin, size) {
      (Some(o), Some(s)) => Some(round2(o + s * frac)),
      (o, _) => o,
    };
    out.position = Some(CuePosition {
      x: region.and_then(|r| anchor(r.x, r.w, h_frac)),
      y: region.and_then(|r| anchor(r.y, r.h, v_frac)),
      h_align: h,
      v_align: v,
    });
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
  let mut regions: HashMap<String, RawRegion> = HashMap::new();
  let mut in_p = false;
  let mut current_start: Option<u64> = None;
  let mut current_end: Option<u64> = None;
  let mut current_style: Option<String> = None;
  let mut current_props = StyleProps::default();
  let mut current_position: Option<CuePosition> = None;
  let mut current_text = String::new();
  let mut parts: SmallVec<[TextPart; 4]> = SmallVec::new();
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
            let attrs = read_paragraph_attrs(e, &styles, &regions);
            current_start = attrs.start;
            current_end = attrs.end;
            current_style = attrs.style;
            current_props = attrs.props;
            current_position = attrs.position;
          }
          b"style" => parse_style_tag(e, &mut styles),
          b"region" => parse_region_tag(e, &mut regions),
          b"span" => {
            span_props = StyleProps::default();
            for attr in e.attributes().flatten() {
              let key = local_name(attr.key.as_ref());
              let val = String::from_utf8_lossy(&attr.value);
              merge_style_attribute(&mut span_props, key, &val);
            }
          }
          b"br" if in_p => {
            current_text.push('\n');
            parts.push(TextPart::plain("\n"));
          }
          _ => {}
        }
      }
      Ok(Event::Empty(ref e)) => {
        let tag = local_name(e.name().as_ref()).to_vec();
        if tag.as_slice() == b"br" && in_p {
          current_text.push('\n');
          parts.push(TextPart::plain("\n"));
        } else if tag.as_slice() == b"style" {
          parse_style_tag(e, &mut styles);
        } else if tag.as_slice() == b"region" {
          parse_region_tag(e, &mut regions);
        } else if tag.as_slice() == b"p" {
          let attrs = read_paragraph_attrs(e, &styles, &regions);
          if let (Some(s), Some(e)) = (attrs.start, attrs.end) {
            let mut sub = Subtitle::new(s, e, "");
            sub.style = attrs.style;
            sub.position = attrs.position;
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
      Ok(Event::GeneralRef(ref e)) => {
        if in_p {
          let name = e.decode().map_err(|e| SubtitleError::Xml {
            format: Format::Ttml,
            error: e.to_string(),
          })?;
          let resolved = if let Ok(Some(ch)) = e.resolve_char_ref() {
            ch.to_string()
          } else {
            match name.as_ref() {
              "amp" => "&".to_string(),
              "lt" => "<".to_string(),
              "gt" => ">".to_string(),
              "quot" => "\"".to_string(),
              "apos" => "'".to_string(),
              other => format!("&{other};"),
            }
          };
          if !resolved.trim().is_empty() {
            current_text.push_str(&resolved);
            let mut part = TextPart::new(
              &resolved,
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
              sub.position = current_position.take();
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

/// Band regions for cues with alignment but no explicit coordinates.
fn band_region_id(v: VerticalAlign) -> &'static str {
  match v {
    VerticalAlign::Top => "r_top",
    VerticalAlign::Center => "r_mid",
    VerticalAlign::Bottom => "r_bot",
  }
}

/// (origin, extent) for a band region — thirds of the frame with a 10%
/// horizontal inset, sized so displayAlign anchors text at the band edge.
fn band_region_geometry(v: VerticalAlign) -> (&'static str, &'static str) {
  match v {
    VerticalAlign::Top => ("10% 10%", "80% 15%"),
    VerticalAlign::Center => ("10% 40%", "80% 20%"),
    VerticalAlign::Bottom => ("10% 80%", "80% 15%"),
  }
}

fn display_align_str(v: VerticalAlign) -> &'static str {
  match v {
    VerticalAlign::Top => "before",
    VerticalAlign::Center => "center",
    VerticalAlign::Bottom => "after",
  }
}

fn text_align_str(h: HorizontalAlign) -> &'static str {
  match h {
    HorizontalAlign::Left => "left",
    HorizontalAlign::Center => "center",
    HorizontalAlign::Right => "right",
  }
}

fn pos_region_geometry(pos: &CuePosition) -> (String, String) {
  let x = pos.x.unwrap_or(50.0);
  let y = pos.y.unwrap_or(50.0);
  let (ox, w) = match pos.h_align {
    HorizontalAlign::Left => (x, 100.0 - x),
    HorizontalAlign::Right => (0.0, x),
    HorizontalAlign::Center => {
      let m = x.min(100.0 - x);
      (x - m, 2.0 * m)
    }
  };
  let (oy, h) = match pos.v_align {
    VerticalAlign::Top => (y, 100.0 - y),
    VerticalAlign::Bottom => (0.0, y),
    VerticalAlign::Center => {
      let m = y.min(100.0 - y);
      (y - m, 2.0 * m)
    }
  };
  (format!("{ox}% {oy}%"), format!("{w}% {h}%"))
}

/// Region assignment result: per-subtitle region id, band-region usage
/// flags (Top/Center/Bottom), and the deduplicated explicit pos regions.
struct RegionAssignments {
  sub_regions: Vec<Option<String>>,
  band_used: [bool; 3],
  pos_regions: Vec<(String, CuePosition)>,
}

/// Assign a region id to each positioned subtitle.
///
/// Cues with explicit coordinates share deduplicated `posN` regions keyed by
/// (x, y, horizontal align, vertical align) — all four feed the region
/// geometry; cues with only an alignment share one of the three band
/// regions.
fn assign_regions(subtitles: &[Subtitle]) -> RegionAssignments {
  let mut band_used = [false; 3];
  let mut pos_regions: Vec<(String, CuePosition)> = Vec::new();
  let mut sub_regions: Vec<Option<String>> = Vec::with_capacity(subtitles.len());

  for sub in subtitles {
    let Some(pos) = &sub.position else {
      sub_regions.push(None);
      continue;
    };
    let id = if pos.x.is_some() && pos.y.is_some() {
      match pos_regions.iter().find(|(_, p)| {
        p.x == pos.x && p.y == pos.y && p.h_align == pos.h_align && p.v_align == pos.v_align
      }) {
        Some((id, _)) => id.clone(),
        None => {
          let id = format!("pos{}", pos_regions.len() + 1);
          pos_regions.push((id.clone(), pos.clone()));
          id
        }
      }
    } else {
      let idx = match pos.v_align {
        VerticalAlign::Top => 0,
        VerticalAlign::Center => 1,
        VerticalAlign::Bottom => 2,
      };
      band_used[idx] = true;
      band_region_id(pos.v_align).to_string()
    };
    sub_regions.push(Some(id));
  }
  RegionAssignments {
    sub_regions,
    band_used,
    pos_regions,
  }
}

/// The text a writer would emit for this cue: concatenated parts when
/// present, else the raw text.
fn visible_text(sub: &Subtitle) -> String {
  if sub.text_parts.is_empty() {
    sub.text.clone()
  } else {
    sub.text_parts.iter().map(|p| p.text.as_str()).collect()
  }
}

/// Prepare cues for TTML output.
fn filter_for_output(subtitles: &[Subtitle]) -> Vec<Subtitle> {
  let visible: Vec<&Subtitle> = subtitles
    .iter()
    .filter(|s| visible_text(s).chars().any(|c| !c.is_whitespace()))
    .collect();
  let mut seen = std::collections::HashSet::new();
  let mut kept: Vec<&Subtitle> = Vec::with_capacity(visible.len());
  for sub in visible.iter().rev() {
    let key = (
      sub.start,
      sub.end,
      visible_text(sub),
      sub.position.as_ref().map(|p| {
        (
          p.x.map(f64::to_bits),
          p.y.map(f64::to_bits),
          p.h_align,
          p.v_align,
        )
      }),
    );
    if seen.insert(key) {
      kept.push(sub);
    }
  }
  kept.reverse();

  let mut owned: Vec<Subtitle> = kept.into_iter().cloned().collect();
  deoverlap_positioned(&mut owned);
  owned
}

/// Trim time overlaps between cues pinned to the same position row.
fn deoverlap_positioned(subs: &mut [Subtitle]) {
  use std::collections::HashMap;
  let mut groups: HashMap<(Option<String>, u64, VerticalAlign), Vec<usize>> = HashMap::new();
  for (i, sub) in subs.iter().enumerate() {
    let Some(pos) = &sub.position else { continue };
    let (Some(_x), Some(y)) = (pos.x, pos.y) else {
      continue;
    };
    groups
      .entry((sub.style.clone(), y.to_bits(), pos.v_align))
      .or_default()
      .push(i);
  }
  for idx in groups.values_mut() {
    idx.sort_by_key(|&i| subs[i].start);
    for k in 0..idx.len() {
      let (start, end) = (subs[idx[k]].start, subs[idx[k]].end);
      for &j in &idx[k + 1..] {
        let next_start = subs[j].start;
        if next_start > start {
          if next_start < end {
            subs[idx[k]].end = next_start;
          }
          break;
        }
      }
    }
  }
}

/// Serialize subtitles to TTML format.
pub fn to_string(subtitles: &[Subtitle], header: Option<&str>) -> String {
  let subtitles = filter_for_output(subtitles);
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
  for sub in &subtitles {
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
    let base = sanitize_style_id(&base);
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

  let regions = assign_regions(&subtitles);
  let has_regions = regions.band_used.iter().any(|u| *u) || !regions.pos_regions.is_empty();

  let header = header.filter(|s| !s.is_empty());
  if header.is_some() || !entries.is_empty() || has_regions {
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
    if has_regions {
      let _ = writer.write_event(Event::Start(BytesStart::new("layout")));
      for v in [
        VerticalAlign::Top,
        VerticalAlign::Center,
        VerticalAlign::Bottom,
      ] {
        let idx = match v {
          VerticalAlign::Top => 0,
          VerticalAlign::Center => 1,
          VerticalAlign::Bottom => 2,
        };
        if !regions.band_used[idx] {
          continue;
        }
        let (origin, extent) = band_region_geometry(v);
        let region = BytesStart::new("region").with_attributes([
          ("xml:id", band_region_id(v)),
          ("tts:origin", origin),
          ("tts:extent", extent),
          ("tts:displayAlign", display_align_str(v)),
        ]);
        let _ = writer.write_event(Event::Empty(region));
      }
      for (id, pos) in &regions.pos_regions {
        let (origin, extent) = pos_region_geometry(pos);
        let region = BytesStart::new("region").with_attributes([
          ("xml:id", id.as_str()),
          ("tts:origin", origin.as_str()),
          ("tts:extent", extent.as_str()),
          ("tts:displayAlign", display_align_str(pos.v_align)),
        ]);
        let _ = writer.write_event(Event::Empty(region));
      }
      let _ = writer.write_event(Event::End(BytesEnd::new("layout")));
    }
    let _ = writer.write_event(Event::End(BytesEnd::new("head")));
  }

  let _ = writer.write_event(Event::Start(BytesStart::new("body")));
  let _ = writer.write_event(Event::Start(BytesStart::new("div")));

  for ((sub, style_id), region_id) in subtitles.iter().zip(&sub_ids).zip(&regions.sub_regions) {
    let start = crate::utils::format_timestamp(sub.start, "WebVTT");
    let end = crate::utils::format_timestamp(sub.end, "WebVTT");
    // TTML uses '.' separator (same as WebVTT), no conversion needed

    let mut p = BytesStart::new("p");
    p.push_attribute(("begin", start.as_str()));
    p.push_attribute(("end", end.as_str()));
    if let Some(id) = style_id {
      p.push_attribute(("style", id.as_str()));
    }
    if let Some(rid) = region_id {
      p.push_attribute(("region", rid.as_str()));
    }
    if let Some(pos) = &sub.position {
      p.push_attribute(("tts:textAlign", text_align_str(pos.h_align)));
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
    // Both the styled span and the unstyled segment are kept, so parts
    // always concatenate back to the full cue text.
    assert_eq!(subs.subtitles()[1].text_parts.len(), 2);
    assert_eq!(
      subs.subtitles()[1].text_parts[0].color,
      Some("yellow".to_string())
    );
    assert_eq!(subs.subtitles()[1].text_parts[1].text, " text");
  }

  #[test]
  fn test_style_id_sanitized() {
    // ASS style names can contain spaces ("OP - Eng"); emitting them
    // verbatim as xml:id is invalid TTML and reparses as multiple style
    // references, collapsing distinct styles into one.
    let props1 = StyleProps {
      font_size: Some("66px".to_string()),
      ..StyleProps::default()
    };
    let props2 = StyleProps {
      font_size: Some("53px".to_string()),
      ..StyleProps::default()
    };
    let mut sub1 = Subtitle::new(1000, 2000, "one");
    sub1.style = Some("OP".to_string());
    sub1.style_props = Some(props1);
    let mut sub2 = Subtitle::new(3000, 4000, "two");
    sub2.style = Some("OP - Eng".to_string());
    sub2.style_props = Some(props2);

    let output = to_string(&[sub1, sub2], None);
    assert!(!output.contains("xml:id=\"OP - Eng\""));
    assert!(output.contains("xml:id=\"OP_-_Eng\""));

    let reparsed = parse_content(&output).unwrap();
    let sizes: Vec<_> = reparsed
      .subtitles()
      .iter()
      .map(|s| s.style_props.as_ref().unwrap().font_size.clone())
      .collect();
    assert_eq!(
      sizes,
      vec![Some("66px".to_string()), Some("53px".to_string())]
    );
  }

  #[test]
  fn test_mixed_content_round_trip() {
    // Unstyled text between spans must survive regeneration (previously
    // `<p><span>red</span> plain <span>bold</span></p>` came back `redbold`).
    let xml = r##"<?xml version="1.0"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling"><body><div>
<p begin="00:00:01.000" end="00:00:02.000"><span tts:color="#FF0000">red</span> plain <span tts:fontWeight="bold">bold</span></p>
</div></body></tt>"##;
    let subs = parse_content(xml).unwrap();
    let joined: String = subs.subtitles()[0]
      .text_parts
      .iter()
      .map(|p| p.text.as_str())
      .collect();
    assert_eq!(joined, subs.subtitles()[0].text);

    let output = to_string(subs.subtitles(), None);
    let reparsed = parse_content(&output).unwrap();
    assert_eq!(reparsed.subtitles()[0].text, "red plain bold");
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
  fn test_parse_entities() {
    // quick-xml emits entity references as GeneralRef events; they must be
    // resolved, not dropped (previously `A &amp; B` parsed as `A  B`).
    let xml = r#"<?xml version="1.0"?>
<tt xmlns="http://www.w3.org/ns/ttml"><body><div>
<p begin="00:00:01.000" end="00:00:02.000">A &amp; B &lt; C &gt; D &quot;E&quot; G&apos;H</p>
<p begin="00:00:03.000" end="00:00:04.000">&#65;&#x42;c &amp;#38;</p>
</div></body></tt>"#;
    let subs = parse_content(xml).unwrap();
    assert_eq!(subs.subtitles()[0].text, "A & B < C > D \"E\" G'H");
    assert_eq!(subs.subtitles()[1].text, "ABc &#38;");
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
  fn test_diamond_style_inheritance_resolves_shared_parent_on_each_branch() {
    // A -> [B, C], B -> [D], C -> [D]; D sets red, B overrides green.
    // Each branch must re-resolve D: C's chain re-applies red over B's green.
    let content = "<tt xmlns=\"http://www.w3.org/ns/ttml\" xmlns:tts=\"http://www.w3.org/ns/ttml#styling\">\
      <head><styling>\
      <style xml:id=\"d\" tts:color=\"#FF0000\"/>\
      <style xml:id=\"b\" style=\"d\" tts:color=\"#00FF00\"/>\
      <style xml:id=\"c\" style=\"d\"/>\
      <style xml:id=\"a\" style=\"b c\"/>\
      </styling></head>\
      <body><div>\
      <p begin=\"00:00:01.000\" end=\"00:00:02.000\" style=\"a\">Hi</p>\
      </div></body></tt>";
    let file = parse_content(content).unwrap();
    let props = file.subtitles()[0].style_props.as_ref().unwrap();
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
  fn test_style_inheritance_child_resets_inherited_bold() {
    // tts:fontWeight="normal" on a child style must unset the parent's bold,
    // matching how direct <p tts:fontWeight="normal"> overrides behave.
    let content = "<tt xmlns=\"http://www.w3.org/ns/ttml\" xmlns:tts=\"http://www.w3.org/ns/ttml#styling\">\
      <head><styling>\
      <style xml:id=\"base\" tts:fontWeight=\"bold\" tts:fontStyle=\"italic\" tts:color=\"#FF0000\"/>\
      <style xml:id=\"plain\" style=\"base\" tts:fontWeight=\"normal\"/>\
      </styling></head>\
      <body><div>\
      <p begin=\"00:00:01.000\" end=\"00:00:02.000\" style=\"plain\">Hi</p>\
      </div></body></tt>";
    let file = parse_content(content).unwrap();
    let props = file.subtitles()[0].style_props.as_ref().unwrap();
    assert!(
      !props.bold,
      "child style reset must win over inherited bold"
    );
    // untouched inherited props still apply
    assert!(props.italic);
    assert_eq!(props.color.as_deref(), Some("#FF0000"));
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

  #[test]
  fn test_write_band_region_for_alignment_only() {
    let mut sub = Subtitle::new(1000, 2000, "title");
    sub.position = Some(CuePosition {
      x: None,
      y: None,
      h_align: HorizontalAlign::Center,
      v_align: VerticalAlign::Top,
    });
    let out = to_string(&[sub], None);
    assert!(out.contains("<layout>"), "got: {out}");
    assert!(
      out.contains(
        "<region xml:id=\"r_top\" tts:origin=\"10% 10%\" tts:extent=\"80% 15%\" tts:displayAlign=\"before\"/>"
      ),
      "got: {out}"
    );
    assert!(out.contains("region=\"r_top\""), "got: {out}");
    assert!(out.contains("tts:textAlign=\"center\""), "got: {out}");
  }

  #[test]
  fn test_write_pos_region_dedup() {
    let pos = CuePosition {
      x: Some(50.0),
      y: Some(25.0),
      h_align: HorizontalAlign::Left,
      v_align: VerticalAlign::Top,
    };
    let sub1 = Subtitle::new(1000, 2000, "one").with_position(pos.clone());
    let sub2 = Subtitle::new(3000, 4000, "two").with_position(pos);
    let out = to_string(&[sub1, sub2], None);
    // One shared region for both cues.
    assert_eq!(out.matches("<region ").count(), 1, "got: {out}");
    assert!(out.contains("tts:origin=\"50% 25%\""), "got: {out}");
    assert_eq!(out.matches("region=\"pos1\"").count(), 2, "got: {out}");
    assert!(out.contains("tts:textAlign=\"left\""), "got: {out}");
  }

  #[test]
  fn test_write_no_layout_without_position() {
    // Plain cues (position = None) must produce byte-comparable output to
    // before positioning support: no <head>, no <layout>, no region attr.
    let out = to_string(&[Subtitle::new(1000, 2000, "plain")], None);
    assert!(!out.contains("<head>"), "got: {out}");
    assert!(!out.contains("region"), "got: {out}");
    assert!(!out.contains("textAlign"), "got: {out}");
  }

  #[test]
  fn test_parse_region_and_text_align() {
    let xml = r#"<?xml version="1.0"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling">
  <head>
    <layout>
      <region xml:id="pos1" tts:origin="50% 25%" tts:displayAlign="before"/>
      <region xml:id="bot" tts:origin="10% 80%" tts:extent="80% 15%" tts:displayAlign="after"/>
    </layout>
  </head>
  <body><div>
    <p begin="00:00:01.000" end="00:00:02.000" region="pos1" tts:textAlign="left">A</p>
    <p begin="00:00:03.000" end="00:00:04.000" region="bot">B</p>
  </div></body>
</tt>"#;
    let subs = parse_content(xml).unwrap();
    let p0 = subs.subtitles()[0].position.as_ref().unwrap();
    assert_eq!(p0.x, Some(50.0));
    assert_eq!(p0.y, Some(25.0));
    assert_eq!(p0.h_align, HorizontalAlign::Left);
    assert_eq!(p0.v_align, VerticalAlign::Top);
    // Region with extent: the anchor is reconstructed from origin + extent
    // + alignment — center of "10% 80%" + "80% 15%" is (50%, 95%).
    let p1 = subs.subtitles()[1].position.as_ref().unwrap();
    assert_eq!(p1.x, Some(50.0));
    assert_eq!(p1.y, Some(95.0));
    assert_eq!(p1.h_align, HorizontalAlign::Center);
    assert_eq!(p1.v_align, VerticalAlign::Bottom);
  }

  #[test]
  fn test_parse_text_align_from_style() {
    let xml = r#"<?xml version="1.0"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling">
  <head><styling>
    <style xml:id="righty" tts:textAlign="right"/>
  </styling></head>
  <body><div>
    <p begin="00:00:01.000" end="00:00:02.000" style="righty">A</p>
  </div></body>
</tt>"#;
    let subs = parse_content(xml).unwrap();
    let pos = subs.subtitles()[0].position.as_ref().unwrap();
    assert_eq!(pos.h_align, HorizontalAlign::Right);
    assert_eq!(pos.x, None);
    assert_eq!(pos.y, None);
  }

  #[test]
  fn test_write_drops_empty_cues() {
    // ASS \p1 vector-drawing events produce a single empty part; the cue
    // renders nothing and must not become an empty <p>.
    let mut drawing = Subtitle::new(1000, 2000, "m 0 0 l 100 100");
    drawing.text_parts.push(TextPart::plain(""));
    let normal = Subtitle::new(3000, 4000, "hello");
    let out = to_string(&[drawing, normal], None);
    assert!(!out.contains("></p>"), "got: {out}");
    assert_eq!(out.matches("<p ").count(), 1, "got: {out}");
    assert!(out.contains("hello"), "got: {out}");
  }

  #[test]
  fn test_write_dedupes_identical_layered_cues() {
    // ASS glow/shadow passes: same text, timing and \pos, different colors.
    // TTML <p>s in one region flow as stacked lines, so only the last
    // (visually dominant) copy is kept.
    let pos = CuePosition {
      x: Some(27.11),
      y: Some(7.69),
      h_align: HorizontalAlign::Center,
      v_align: VerticalAlign::Center,
    };
    let mut top = Subtitle::new(1000, 2000, "c").with_position(pos.clone());
    top.text_parts.push({
      let mut p = TextPart::plain("c");
      p.color = Some("#C6EAE8".into());
      p
    });
    let mut bottom = Subtitle::new(1000, 2000, "c").with_position(pos);
    bottom.text_parts.push({
      let mut p = TextPart::plain("c");
      p.color = Some("#D37B4C".into());
      p
    });
    let out = to_string(&[top, bottom], None);
    assert_eq!(out.matches("<p ").count(), 1, "got: {out}");
    assert!(out.contains("#D37B4C"), "got: {out}");
    assert!(!out.contains("#C6EAE8"), "got: {out}");
  }

  #[test]
  fn test_deoverlap_trims_crossfade() {
    // ASS \fad crossfade: line A fades out while line B fades in at the
    // same \pos row (34.89-38.81 / 38.60-40.69 in the wild). TTML players
    // hard-switch cues, so A must end when B starts.
    let row = |x: f64| CuePosition {
      x: Some(x),
      y: Some(7.69),
      h_align: HorizontalAlign::Center,
      v_align: VerticalAlign::Center,
    };
    let mut a = Subtitle::new(34890, 38810, "aranu").with_position(row(30.0));
    a.style = Some("OP".into());
    let mut b = Subtitle::new(38600, 40690, "yume").with_position(row(40.0));
    b.style = Some("OP".into());
    let out = to_string(&[a, b], None);
    assert!(
      out.contains("begin=\"00:00:34.890\" end=\"00:00:38.600\""),
      "got: {out}"
    );
    assert!(
      out.contains("begin=\"00:00:38.600\" end=\"00:00:40.690\""),
      "got: {out}"
    );
  }

  #[test]
  fn test_deoverlap_keeps_same_start_glyphs() {
    // Glyphs of one typeset line share identical start times — never trim.
    let row = |x: f64| CuePosition {
      x: Some(x),
      y: Some(7.69),
      h_align: HorizontalAlign::Center,
      v_align: VerticalAlign::Center,
    };
    let mut a = Subtitle::new(1000, 3000, "c").with_position(row(20.0));
    a.style = Some("OP".into());
    let mut b = Subtitle::new(1000, 3000, "h").with_position(row(30.0));
    b.style = Some("OP".into());
    let out = to_string(&[a, b], None);
    assert_eq!(out.matches("end=\"00:00:03.000\"").count(), 2, "got: {out}");
  }

  #[test]
  fn test_deoverlap_ignores_different_rows_and_styles() {
    let pos = |y: f64| CuePosition {
      x: Some(50.0),
      y: Some(y),
      h_align: HorizontalAlign::Center,
      v_align: VerticalAlign::Center,
    };
    // Different y row: overlapping time is legitimate (two stacked lines).
    let mut a = Subtitle::new(1000, 3000, "one").with_position(pos(7.69));
    a.style = Some("OP".into());
    let mut b = Subtitle::new(2000, 4000, "two").with_position(pos(12.18));
    b.style = Some("OP - Eng".into());
    let out = to_string(&[a, b], None);
    assert!(
      out.contains("begin=\"00:00:01.000\" end=\"00:00:03.000\""),
      "got: {out}"
    );
  }

  #[test]
  fn test_deoverlap_ignores_band_cues() {
    // Alignment-only cues stack via normal TTML block flow — no trimming.
    let band = || CuePosition {
      x: None,
      y: None,
      h_align: HorizontalAlign::Center,
      v_align: VerticalAlign::Top,
    };
    let a = Subtitle::new(1000, 3000, "one").with_position(band());
    let b = Subtitle::new(2000, 4000, "two").with_position(band());
    let out = to_string(&[a, b], None);
    assert!(
      out.contains("begin=\"00:00:01.000\" end=\"00:00:03.000\""),
      "got: {out}"
    );
    assert!(
      out.contains("begin=\"00:00:02.000\" end=\"00:00:04.000\""),
      "got: {out}"
    );
  }

  #[test]
  fn test_write_keeps_distinct_positions() {
    // Same text/timing but different \pos: not duplicates.
    let a = Subtitle::new(1000, 2000, "c").with_position(CuePosition {
      x: Some(10.0),
      y: Some(10.0),
      ..CuePosition::default()
    });
    let b = Subtitle::new(1000, 2000, "c").with_position(CuePosition {
      x: Some(20.0),
      y: Some(10.0),
      ..CuePosition::default()
    });
    let out = to_string(&[a, b], None);
    assert_eq!(out.matches("<p ").count(), 2, "got: {out}");
  }

  #[test]
  fn test_position_round_trip() {
    // Explicit position + alignment survive a write → parse cycle.
    let sub = Subtitle::new(1000, 2000, "x").with_position(CuePosition {
      x: Some(50.0),
      y: Some(25.0),
      h_align: HorizontalAlign::Right,
      v_align: VerticalAlign::Top,
    });
    let out = to_string(std::slice::from_ref(&sub), None);
    let reparsed = parse_content(&out).unwrap();
    assert_eq!(
      reparsed.subtitles()[0].position.as_ref().unwrap(),
      sub.position.as_ref().unwrap()
    );
  }

  #[test]
  fn test_center_anchor_region_is_symmetric() {
    // Regression: a top-center anchored cue (ASS \an8 \pos(320,40)
    // @640x360 → 50%, 11.11%) was emitted as origin="50% 11.11%" with no
    // extent, so the region ran from the anchor to the screen corner and
    // textAlign centered text within the bottom-right quadrant instead of
    // the top middle. The region must be symmetric about the anchor:
    // origin "0% 11.11%", extent "100% 88.89%" (verified with python3).
    let sub = Subtitle::new(1000, 2000, "x").with_position(CuePosition {
      x: Some(50.0),
      y: Some(11.11),
      h_align: HorizontalAlign::Center,
      v_align: VerticalAlign::Top,
    });
    let out = to_string(std::slice::from_ref(&sub), None);
    assert!(out.contains("tts:origin=\"0% 11.11%\""), "got: {out}");
    assert!(out.contains("tts:extent=\"100% 88.89%\""), "got: {out}");
    assert!(out.contains("tts:displayAlign=\"before\""), "got: {out}");
    assert!(out.contains("tts:textAlign=\"center\""), "got: {out}");

    // And the inverse mapping recovers the anchor exactly.
    let reparsed = parse_content(&out).unwrap();
    assert_eq!(
      reparsed.subtitles()[0].position.as_ref().unwrap(),
      sub.position.as_ref().unwrap()
    );
  }

  #[test]
  fn test_center_anchor_off_center_clamps_to_frame() {
    // \pos at 30% width, center-anchored: mirror half is min(30, 70) = 30,
    // so origin "0%", extent "60%" (verified with python3).
    let sub = Subtitle::new(1000, 2000, "x").with_position(CuePosition {
      x: Some(30.0),
      y: Some(20.0),
      h_align: HorizontalAlign::Center,
      v_align: VerticalAlign::Top,
    });
    let out = to_string(std::slice::from_ref(&sub), None);
    assert!(out.contains("tts:origin=\"0% 20%\""), "got: {out}");
    assert!(out.contains("tts:extent=\"60% 80%\""), "got: {out}");
    let reparsed = parse_content(&out).unwrap();
    assert_eq!(
      reparsed.subtitles()[0].position.as_ref().unwrap(),
      sub.position.as_ref().unwrap()
    );
  }
}
