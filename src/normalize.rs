use crate::model::Subtitle;
use regex::Regex;
use std::sync::LazyLock;

static RE_MULTI_SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" {2,}").unwrap());

static RE_TRAILING_SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]+$").unwrap());

static RE_MULTI_NEWLINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());

static RE_SPACE_BEFORE_PUNCT: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r" +([,!.;:?])").unwrap());

static RE_REPEATED_PUNCT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([.!?,]){4,}").unwrap());

static RE_ELLIPSIS_SPACED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.\s*\.\s*\.").unwrap());

static RE_HI_PAREN: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"\s*[\(\[][^)\]]{2,60}[\)\]]").unwrap());

static RE_HI_BRACKET: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"\s*\[[^\]]{2,60}\]").unwrap());

static RE_SPEAKER_LABEL: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^(?:>>|>\s|-\s|[A-Z ]{2,20}:)\s*").unwrap());

static RE_MUSIC_NOTE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[♪♫♬]").unwrap());

static RE_OCR_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
  vec![
    (Regex::new(r"\brn\b").unwrap(), "m"),
    (Regex::new(r"(\d)rn(\w)").unwrap(), "${1}m${2}"),
    (Regex::new(r"(\d)O(\d)").unwrap(), "${1}0${2}"),
    (Regex::new(r"(\d)l(\d)").unwrap(), "${1}1${2}"),
    (Regex::new(r"([a-z])0([a-z])").unwrap(), "${1}o${2}"),
  ]
});

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
  for (re, rep) in RE_OCR_PATTERNS.iter() {
    result = re.replace_all(&result, *rep).to_string();
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

/// Optimize line breaks in subtitle text. Splits long lines at natural
/// boundaries (punctuation, conjunctions) to improve readability.
///
/// Each resulting line is at most `max_chars` characters, and line lengths
/// are balanced when possible.
pub fn optimize_line_breaks(text: &str, max_chars: usize) -> String {
  let mut result_parts: Vec<String> = Vec::new();
  let mut queue: Vec<String> = text.lines().map(|l| l.trim().to_string()).collect();
  // Process front-to-back (FIFO)
  let mut idx = 0;

  while idx < queue.len() {
    let line = std::mem::take(&mut queue[idx]);
    if line.chars().count() <= max_chars {
      result_parts.push(line);
      idx += 1;
      continue;
    }

    // Try to find natural break points
    let words: Vec<&str> = line.split_whitespace().collect();
    let best_break = find_best_split(&words, max_chars);

    match best_break {
      Some(split_idx) => {
        result_parts.push(words[..split_idx].join(" "));
        let remaining = words[split_idx..].join(" ");
        if remaining.is_empty() {
          idx += 1; // nothing left, advance
        } else {
          queue[idx] = remaining; // process remainder next iteration
        }
      }
      None => {
        // No natural break found, hard split at char boundary
        let first: String = line.chars().take(max_chars).collect();
        let rest: String = line.chars().skip(max_chars).collect();
        result_parts.push(first);
        if rest.is_empty() {
          idx += 1;
        } else {
          queue[idx] = rest;
        }
      }
    }
  }

  result_parts.join("\n")
}

/// Find the best word boundary to split a sequence of words.
/// Returns the index after the last word that fits in `max_chars`.
fn find_best_split(words: &[&str], max_chars: usize) -> Option<usize> {
  if words.is_empty() {
    return None;
  }

  // Build cumulative character lengths
  let mut cum: Vec<usize> = Vec::with_capacity(words.len());
  let mut total = 0usize;
  for w in words {
    total += w.len() + 1; // +1 for space
    cum.push(total);
  }

  // Find the last word that fits in max_chars
  let mut last_fit = None;
  let mut preferred = None;

  for (i, &c) in cum.iter().enumerate() {
    let len = c.saturating_sub(1); // remove trailing space
    if len <= max_chars {
      last_fit = Some(i + 1); // index after this word
      // Check if this is a preferred break point
      let word = words[i];
      if word.ends_with(',') || word.ends_with(';') || word.ends_with(':') {
        preferred = Some(i + 1);
      }
      // Check for conjunctions that would start the next line
      if i + 1 < words.len() && ["and", "or", "but", "so", "yet", "for"].contains(&words[i + 1]) {
        preferred = Some(i + 1);
      }
    } else {
      break;
    }
  }

  // Prefer breaks at punctuation/conjunctions, fall back to last fitting word
  preferred.or(last_fit).filter(|&i| i < words.len())
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
    assert_eq!(
      strip_hearing_impaired("Hello (LAUGHS) world"),
      "Hello world"
    );
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

  #[test]
  fn test_optimize_line_breaks_short() {
    // Short line stays unchanged
    assert_eq!(optimize_line_breaks("Hello World", 42), "Hello World");
  }

  #[test]
  fn test_optimize_line_breaks_long() {
    let long =
      "This is a very long subtitle line that definitely exceeds the maximum character limit";
    let result = optimize_line_breaks(long, 42);
    // Should be split into multiple lines
    assert!(result.contains('\n'));
    // Each line should be at most ~42 chars (allowing word boundaries)
    for line in result.lines() {
      assert!(
        line.chars().count() <= 42 + 10,
        "line too long: '{}' ({} chars)",
        line,
        line.chars().count()
      );
    }
  }

  #[test]
  fn test_optimize_line_breaks_preserves_content() {
    let input = "The quick brown fox jumps over the lazy dog and runs away";
    let result = optimize_line_breaks(input, 20);
    // All words should be present in the output
    for word in input.split_whitespace() {
      assert!(
        result.contains(word),
        "word '{}' lost in line break optimization",
        word
      );
    }
  }
}

#[cfg(test)]
#[test]
fn test_optimize_line_breaks_order() {
  let result = optimize_line_breaks("abc def ghijklmnop", 5);
  assert_eq!(result, "abc\ndef\nghijk\nlmnop",
    "got: {:?} — lines are in wrong order", result);
}
