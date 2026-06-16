use crate::model::Subtitle;
use regex::Regex;
use std::sync::LazyLock;

static RE_MULTI_SPACE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r" {2,}").unwrap());

static RE_TRAILING_SPACE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"[ \t]+$").unwrap());

static RE_MULTI_NEWLINE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());

static RE_SPACE_BEFORE_PUNCT: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r" +([,!.;:?])").unwrap());

static RE_REPEATED_PUNCT: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"([.!?,]){4,}").unwrap());

static RE_ELLIPSIS_SPACED: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"\.\s*\.\s*\.").unwrap());

static RE_HI_PAREN: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"\s*[\(\[][^)\]]{2,60}[\)\]]").unwrap());

static RE_HI_BRACKET: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"\s*\[[^\]]{2,60}\]").unwrap());

static RE_SPEAKER_LABEL: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^(?:>>|>\s|-\s|[A-Z ]{2,20}:)\s*").unwrap());

static RE_MUSIC_NOTE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"[♪♫♬]").unwrap());

pub fn normalize_whitespace(text: &str) -> String {
  let lines: Vec<String> = text
    .lines()
    .map(|line| {
      let trimmed = line.trim();
      RE_MULTI_SPACE.replace_all(trimmed, " ").to_string()
    })
    .collect();
  let mut result = lines.join("\n");
  result = RE_MULTI_NEWLINE.replace_all(&result, "\n\n").to_string();
  result = RE_TRAILING_SPACE.replace_all(&result, "").to_string();
  result.trim().to_string()
}

pub fn normalize_quotes(text: &str) -> String {
  text
    .replace(['\u{201C}', '\u{201D}'], "\"")
    .replace(['\u{2018}', '\u{2019}'], "'")
    .replace('\u{2013}', "-")
    .replace('\u{2014}', "--")
}

pub fn normalize_punctuation(text: &str) -> String {
  let mut result = text.to_string();
  result = RE_SPACE_BEFORE_PUNCT.replace_all(&result, "$1").to_string();
  result = RE_REPEATED_PUNCT.replace_all(&result, "$1$1$1").to_string();
  result = RE_ELLIPSIS_SPACED.replace_all(&result, "…").to_string();
  result = result.replace("....", "…");
  result
}

pub fn fix_ocr_errors(text: &str) -> String {
  let mut result = text.to_string();
  let patterns: &[(&str, &str)] = &[
    (r"\brn\b", "m"),
    (r"(\d)rn(\w)", "${1}m${2}"),
    (r"(\d)O(\d)", "${1}0${2}"),
    (r"(\d)l(\d)", "${1}1${2}"),
    (r"([a-z])0([a-z])", "${1}o${2}"),
  ];
  for (pat, rep) in patterns {
    if let Ok(re) = Regex::new(pat) {
      result = re.replace_all(&result, *rep).to_string();
    }
  }
  result
}

pub fn strip_hearing_impaired(text: &str) -> String {
  let mut result = text.to_string();
  result = RE_HI_PAREN.replace_all(&result, "").to_string();
  result = RE_HI_BRACKET.replace_all(&result, "").to_string();
  result = RE_SPEAKER_LABEL.replace_all(&result, "").to_string();
  result = RE_MUSIC_NOTE.replace_all(&result, "").to_string();
  result = result.trim().to_string();
  if result.is_empty() {
    return String::new();
  }
  normalize_whitespace(&result)
}

pub fn normalize_text(text: &str) -> String {
  let result = normalize_quotes(text);
  let result = normalize_punctuation(&result);
  normalize_whitespace(&result)
}

pub fn normalize_subtitle(sub: &mut Subtitle) {
  sub.text = normalize_text(&sub.text);
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_normalize_whitespace() {
    assert_eq!(normalize_whitespace("hello   world"), "hello world");
    assert_eq!(normalize_whitespace("  hello  "), "hello");
    assert_eq!(normalize_whitespace("a\n\n\n\nb"), "a\n\nb");
  }

  #[test]
  fn test_normalize_quotes() {
    assert_eq!(normalize_quotes("\u{201C}hello\u{201D}"), "\"hello\"");
    assert_eq!(normalize_quotes("\u{2018}it's\u{2019}"), "'it's'");
    assert_eq!(normalize_quotes("a\u{2013}b"), "a-b");
  }

  #[test]
  fn test_normalize_punctuation() {
    assert_eq!(normalize_punctuation("hello , world"), "hello, world");
    assert_eq!(normalize_punctuation("what????"), "what???");
    assert_eq!(normalize_punctuation(". . ."), "…");
  }

  #[test]
  fn test_fix_ocr_errors() {
    assert_eq!(fix_ocr_errors("12O456"), "120456");
    assert_eq!(fix_ocr_errors("1l0"), "110");
    assert_eq!(fix_ocr_errors("w0rd"), "word");
  }

  #[test]
  fn test_strip_hearing_impaired() {
    assert_eq!(strip_hearing_impaired("Hello (LAUGHS) world"), "Hello world");
    assert_eq!(strip_hearing_impaired("[APPLAUSE] Nice"), "Nice");
    assert_eq!(strip_hearing_impaired(">> Hello there"), "Hello there");
    assert_eq!(strip_hearing_impaired("JOHN: What's up?"), "What's up?");
    assert_eq!(strip_hearing_impaired("♪ Music ♪"), "Music");
  }

  #[test]
  fn test_normalize_subtitle() {
    let mut sub = Subtitle::new(0, 1000, "Hello   \u{201C}world\u{201D} !");
    normalize_subtitle(&mut sub);
    assert_eq!(sub.text, "Hello \"world\"!");
  }
}
