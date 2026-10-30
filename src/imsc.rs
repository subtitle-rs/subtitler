//! IMSC 1.1 profile validation for TTML-family documents.
//!
//! Checks are grounded in the W3C IMSC 1.1 Recommendation
//! (https://www.w3.org/TR/ttml-imsc1.1/): the profile constrains which
//! attributes, values and syntax forms a document MAY use. This module
//! implements the structural subset that applies to text documents
//! (the Image profile's picture constraints are out of scope).

use crate::model::ValidationIssue;

/// Validate a TTML-family document (TTML/DFXP/iTT/EBU-TT-D) against the
/// IMSC 1.1 text-profile constraints that are checkable statically.
pub fn imsc_validate(content: &str) -> Vec<ValidationIssue> {
  let mut issues = Vec::new();

  // IMSC requires xml:lang on the <tt> root (the profile examples and
  // §5.2 both carry it; TTML makes it optional — IMSC does not).
  let root = match root_tt_tag(content) {
    Some(r) => r,
    None => return issues, // not a TTML document at all
  };
  if !has_attribute(&root, "xml:lang") && !has_attribute(&root, "lang") {
    issues.push(ValidationIssue::ImscMissingAttribute {
      element: "tt".into(),
      attribute: "xml:lang".into(),
    });
  }

  // Timebase: IMSC's media timebase is the permitted one — the
  // #timeBase-media feature is permitted while clock/SMTE timebases
  // are not part of the text profile constraints.
  if let Some(value) = attribute_value(&root, "ttp:timeBase") {
    let v = value.to_lowercase();
    if v != "media" {
      issues.push(ValidationIssue::ImscProhibited {
        detail: format!(
          "ttp:timeBase=\"{value}\" — the IMSC text profile supports the media timebase only"
        ),
      });
    }
  }

  // Frame-based time expressions require ttp:frameRate (the
  // #time-offset-with-frames feature depends on it).
  if uses_frames_time_expression(content) && !content.contains("ttp:frameRate") {
    issues.push(ValidationIssue::ImscProhibited {
      detail: "frame-based time expressions used without ttp:frameRate on <tt>".into(),
    });
  }

  // Strictly negative length expressions are prohibited except for
  // tts:disparity and tts:textShadow (§7.3 length expressions).
  for m in negative_length_attrs(content) {
    issues.push(ValidationIssue::ImscProhibited {
      detail: format!("negative length on tts:{} is prohibited", m),
    });
  }

  // tts:textShadow SHALL NOT have more than 4 <shadow> values.
  for m in text_shadow_overflow(content) {
    issues.push(ValidationIssue::ImscProhibited {
      detail: format!("tts:textShadow has {} <shadow> components (maximum 4)", m),
    });
  }

  // rh/rw (root-container-relative) unit misuse: rh is vertical-only,
  // rw is horizontal-only.
  for detail in rh_rw_misuse(content) {
    issues.push(ValidationIssue::ImscProhibited { detail });
  }

  issues
}

/// Return the `<tt …>` root tag text, if the document is TTML-family.
fn root_tt_tag(content: &str) -> Option<String> {
  let start = content.find("<tt")?;
  let end = content[start..].find('>')? + start;
  Some(content[start..=end].to_string())
}

fn has_attribute(tag: &str, name: &str) -> bool {
  // Match the attribute name followed by '=' with optional whitespace.
  let pattern = format!("{}=", name);
  tag.to_lowercase().contains(&pattern.to_lowercase())
}

fn attribute_value(tag: &str, name: &str) -> Option<String> {
  let lower = tag.to_lowercase();
  let needle = format!("{}=\"", name.to_lowercase());
  let start = lower.find(&needle)? + needle.len();
  let end = tag[start..].find('"')? + start;
  Some(tag[start..end].to_string())
}

/// Detect frame-form time expressions: `HH:MM:SS:FF` (colon before the
/// last field, no fractional seconds).
fn uses_frames_time_expression(content: &str) -> bool {
  // Match 00:00:01:12-style values inside begin/end attributes.
  let re = regex::Regex::new(r#"(?:begin|end|dur)="\d+:\d{2}:\d{2}:\d{2}""#).unwrap();
  re.is_match(content)
}

/// Find `tts:<attr>` attributes whose value starts with a minus sign
/// (strictly negative length), excluding the two permitted attributes.
fn negative_length_attrs(content: &str) -> Vec<String> {
  let re = regex::Regex::new(r#"tts:([a-zA-Z]+)="(-[\d.]+)"#).unwrap();
  let mut out = Vec::new();
  for caps in re.captures_iter(content) {
    let attr = caps[1].to_string();
    if attr == "disparity" || attr == "textShadow" {
      continue;
    }
    if !out.contains(&attr) {
      out.push(attr);
    }
  }
  out
}

/// Count the <shadow> components of each tts:textShadow value; report
/// the counts that exceed 4.
fn text_shadow_overflow(content: &str) -> Vec<usize> {
  let re = regex::Regex::new(r#"tts:textShadow="([^"]+)""#).unwrap();
  let mut out = Vec::new();
  for caps in re.captures_iter(content) {
    // <shadow> = "dx dy [blur] [color]" — count whitespace-separated
    // groups conservatively: a new group starts at a signed length.
    let value = caps[1].to_string();
    let components = count_shadow_components(&value);
    if components > 4 {
      out.push(components);
    }
  }
  out
}

fn count_shadow_components(value: &str) -> usize {
  // A shadow is 2-4 tokens (dx dy [blur] [color]); colors start with
  // '#' or a named color. Count signed-length tokens as group starts.
  let tokens: Vec<&str> = value.split_whitespace().collect();
  let mut groups = 0usize;
  let mut in_color = false;
  for t in &tokens {
    let starts_group = t.starts_with('-')
      || t.starts_with('+')
      || t.chars().next().is_some_and(|c| c.is_ascii_digit());
    if starts_group && !in_color {
      groups += 1;
      in_color = false;
    } else if t.starts_with('#') {
      in_color = true;
    }
  }
  groups.max(1)
}

/// rh units must not appear in horizontal length components
/// (tts:origin/tts:extent x-components, tts:left/right/width),
/// rw must not appear in vertical ones (y-components, top/height).
fn rh_rw_misuse(content: &str) -> Vec<String> {
  let mut out = Vec::new();
  let re = regex::Regex::new(r#"tts:(origin|extent)="([^"]*(?:rh|rw)[^"]*)""#).unwrap();
  for caps in re.captures_iter(content) {
    let attr = caps[1].to_string();
    let value = caps[2].to_string();
    // origin = "x y"; extent = "w h"
    let comps: Vec<&str> = value.split_whitespace().collect();
    if comps.len() == 2 {
      let (horizontal, vertical) = (comps[0], comps[1]);
      if (horizontal.contains("rh") && !attr.contains("height")) || vertical.contains("rw") {
        out.push(format!(
          "tts:{}=\"{}\" — rh units are vertical-only, rw horizontal-only",
          attr, value
        ));
      }
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn missing_xml_lang_is_reported() {
    let doc = r#"<tt xmlns="http://www.w3.org/ns/ttml"><body/></tt>"#;
    let issues = imsc_validate(doc);
    assert!(issues.iter().any(
      |i| matches!(i, ValidationIssue::ImscMissingAttribute { element, attribute }
        if element == "tt" && attribute == "xml:lang")
    ));
  }

  #[test]
  fn present_xml_lang_is_clean() {
    let doc = r#"<tt xml:lang="en" xmlns="http://www.w3.org/ns/ttml"><body><p begin="00:00:01.000" end="00:00:02.000">Hi</p></body></tt>"#;
    assert!(imsc_validate(doc).is_empty());
  }

  #[test]
  fn negative_length_is_prohibited() {
    let doc = r#"<tt xml:lang="en" xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling"><body><p begin="00:00:01.000" end="00:00:02.000" tts:origin="-10px 10px">Hi</p></body></tt>"#;
    let issues = imsc_validate(doc);
    assert!(
      issues
        .iter()
        .any(|i| matches!(i, ValidationIssue::ImscProhibited { detail }
        if detail.contains("negative length on tts:origin")))
    );
  }

  #[test]
  fn negative_disparity_is_permitted() {
    let doc = r#"<tt xml:lang="en" xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling"><body><p begin="00:00:01.000" end="00:00:02.000" tts:disparity="-3px">Hi</p></body></tt>"#;
    assert!(imsc_validate(doc).is_empty());
  }

  #[test]
  fn frames_without_framerate_is_reported() {
    let doc = r#"<tt xml:lang="en" xmlns="http://www.w3.org/ns/ttml"><body><p begin="00:00:01:12" end="00:00:02:00">Hi</p></body></tt>"#;
    let issues = imsc_validate(doc);
    assert!(
      issues
        .iter()
        .any(|i| matches!(i, ValidationIssue::ImscProhibited { detail }
        if detail.contains("ttp:frameRate")))
    );
  }

  #[test]
  fn clock_timebase_is_flagged() {
    let doc = r#"<tt xml:lang="en" ttp:timeBase="clock" xmlns="http://www.w3.org/ns/ttml" xmlns:ttp="http://www.w3.org/ns/ttml#parameter"><body><p begin="00:00:01.000" end="00:00:02.000">Hi</p></body></tt>"#;
    let issues = imsc_validate(doc);
    assert!(
      issues
        .iter()
        .any(|i| matches!(i, ValidationIssue::ImscProhibited { detail }
        if detail.contains("clock")))
    );
  }
}
