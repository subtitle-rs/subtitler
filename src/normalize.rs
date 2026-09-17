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

// ── Language character filtering ──

/// Languages selectable for character filtering, mirroring the language
/// list of editingtools.io's subtitle cleaner plus Chinese. Used by
/// [`remove_other_language_chars`] to strip letters that do not occur in
/// any kept language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Language {
  English,
  Spanish,
  French,
  German,
  Italian,
  Polish,
  Portuguese,
  Finnish,
  Norwegian,
  Swedish,
  Danish,
  Turkish,
  Vietnamese,
  Ukrainian,
  Russian,
  Hebrew,
  Arabic,
  Thai,
  Japanese,
  Korean,
  Chinese,
}

/// Basic Latin letters. Included only for Latin-script languages —
/// selecting e.g. Chinese or Japanese must strip Latin letters (the main
/// bilingual-cleanup use case), matching editingtools.io's behavior.
const LATIN_BASE: &[(char, char)] = &[('A', 'Z'), ('a', 'z')];
/// Latin-1 Supplement letters (ä ö ü ß é ñ ç ã å ø …).
const LATIN_1: &[(char, char)] = &[('\u{C0}', '\u{FF}')];
/// Latin Extended-A (Polish ą ć ę ł, Turkish ğ ı ş, Vietnamese đ ơ ư, œ …).
const LATIN_EXT_A: &[(char, char)] = &[('\u{100}', '\u{17F}')];
/// Latin Extended Additional (Vietnamese tone-marked vowels ạ ế ộ …).
const LATIN_EXT_ADD: &[(char, char)] = &[('\u{1E00}', '\u{1EFF}')];
/// Cyrillic + Cyrillic Supplement.
const CYRILLIC: &[(char, char)] = &[('\u{400}', '\u{52F}')];
/// Hebrew block.
const HEBREW: &[(char, char)] = &[('\u{590}', '\u{5FF}')];
/// Arabic block.
const ARABIC: &[(char, char)] = &[('\u{600}', '\u{6FF}')];
/// Thai block.
const THAI: &[(char, char)] = &[('\u{E00}', '\u{E7F}')];
const HIRAGANA: &[(char, char)] = &[('\u{3041}', '\u{309F}')];
const KATAKANA: &[(char, char)] = &[('\u{30A0}', '\u{30FF}')];
/// CJK Unified Ideographs + Extension A.
const HAN: &[(char, char)] = &[('\u{3400}', '\u{4DBF}'), ('\u{4E00}', '\u{9FFF}')];
/// Hangul Jamo + Compatibility Jamo + Syllables.
const HANGUL: &[(char, char)] = &[
  ('\u{1100}', '\u{11FF}'),
  ('\u{3130}', '\u{318F}'),
  ('\u{AC00}', '\u{D7A3}'),
];

impl Language {
  /// Unicode blocks whose letters occur in this language.
  fn letter_ranges(self) -> &'static [&'static [(char, char)]] {
    match self {
      Language::English => &[LATIN_BASE],
      Language::Spanish
      | Language::French
      | Language::German
      | Language::Italian
      | Language::Polish
      | Language::Portuguese
      | Language::Finnish
      | Language::Norwegian
      | Language::Swedish
      | Language::Danish
      | Language::Turkish => &[LATIN_BASE, LATIN_1, LATIN_EXT_A],
      Language::Vietnamese => &[LATIN_BASE, LATIN_1, LATIN_EXT_A, LATIN_EXT_ADD],
      Language::Ukrainian | Language::Russian => &[CYRILLIC],
      Language::Hebrew => &[HEBREW],
      Language::Arabic => &[ARABIC],
      Language::Thai => &[THAI],
      Language::Japanese => &[HIRAGANA, KATAKANA, HAN],
      Language::Korean => &[HANGUL, HAN],
      Language::Chinese => &[HAN],
    }
  }
}

/// Remove letters that do not occur in any of `keep`.
///
/// Only *alphabetic* characters are filtered: digits, whitespace,
/// punctuation, symbols and emoji are never touched, so e.g. CJK or
/// Arabic punctuation survives an English filter. Unicode block ranges
/// are coarse — a stray letter from an unlisted block of the same script
/// family may survive, but the common cases (mixed bilingual subtitles)
/// clean up correctly.
pub fn remove_other_language_chars(text: &str, keep: &[Language]) -> String {
  if keep.is_empty() {
    return text.to_string();
  }
  let groups: Vec<&[(char, char)]> = keep
    .iter()
    .flat_map(|lang| lang.letter_ranges().iter().copied())
    .collect();
  text
    .chars()
    .filter(|c| {
      if !c.is_alphabetic() {
        return true;
      }
      groups
        .iter()
        .any(|ranges| ranges.iter().any(|&(lo, hi)| (lo..=hi).contains(c)))
    })
    .collect()
}

/// Filter text to keep only characters from a specified language Unicode block.
///
/// `lang` is an ISO-style code: `"en"`, `"zh"`, `"ja"`, `"ko"`, `"ar"`,
/// `"he"` map to the corresponding [`Language`]; for the wider European
/// set use [`remove_other_language_chars`] with [`Language`] directly.
/// Unknown `lang` returns the input unchanged. Since v2.4.0 this no
/// longer strips punctuation — only letters are filtered.
pub fn filter_language(text: &str, lang: &str) -> String {
  let language = match lang {
    "en" => Language::English,
    "zh" => Language::Chinese,
    "ja" => Language::Japanese,
    "ko" => Language::Korean,
    "ar" => Language::Arabic,
    "he" => Language::Hebrew,
    _ => return text.to_string(),
  };
  remove_other_language_chars(text, &[language])
}

/// Merge short lines (≤ `max_chars` characters) by removing newlines within
/// each subtitle's text. Lines longer than `max_chars` or containing explicit
/// breaks (double-newline) are preserved.
pub fn merge_short_lines(text: &str, max_chars: usize) -> String {
  let lines: Vec<&str> = text.lines().collect();
  let mut result = Vec::new();
  let mut buf = String::new();

  for line in lines {
    if line.is_empty() {
      // Double newline — paragraph break, flush buffer
      if !buf.is_empty() {
        result.push(std::mem::take(&mut buf));
      }
      result.push(String::new());
      continue;
    }
    if line.len() <= max_chars && !buf.is_empty() {
      buf.push(' ');
      buf.push_str(line);
    } else if line.len() <= max_chars {
      buf.push_str(line);
    } else {
      // Long line — keep as is
      if !buf.is_empty() {
        result.push(std::mem::take(&mut buf));
      }
      result.push(line.to_string());
    }
  }
  if !buf.is_empty() {
    result.push(buf);
  }
  result.join("\n")
}

/// Replace all newlines with spaces, collapsing multiple spaces.
pub fn remove_all_newlines(text: &str) -> String {
  let s = text.replace('\n', " ");
  let mut result = String::with_capacity(s.len());
  let mut prev_space = false;
  for c in s.chars() {
    if c == ' ' {
      if !prev_space {
        result.push(' ');
        prev_space = true;
      }
    } else {
      result.push(c);
      prev_space = false;
    }
  }
  result.trim().to_string()
}

/// Replace all newlines with a custom separator string.
pub fn replace_newlines(text: &str, separator: &str) -> String {
  text.lines().collect::<Vec<_>>().join(separator)
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

  #[test]
  fn test_filter_language_english() {
    let input = "Hello 你好 World 世界";
    assert_eq!(filter_language(input, "en"), "Hello  World ");
  }

  #[test]
  fn test_filter_language_chinese() {
    let input = "Hello 你好 World 世界";
    assert_eq!(filter_language(input, "zh"), " 你好  世界");
  }

  #[test]
  fn test_filter_language_keeps_punctuation() {
    // Non-ASCII punctuation and emoji survive — only letters are filtered.
    assert_eq!(filter_language("¡Hola! 你好… 🎬", "en"), "¡Hola! … 🎬");
  }

  #[test]
  fn test_filter_language_unknown_code_unchanged() {
    assert_eq!(filter_language("Hello 你好", "xx"), "Hello 你好");
  }

  #[test]
  fn test_remove_other_language_chars_mixed_scripts() {
    use Language::*;
    // Russian strips Latin base and Han — selecting a non-Latin language
    // removes Latin letters (the main bilingual-cleanup use case).
    assert_eq!(
      remove_other_language_chars("Привет OK 世界", &[Russian]),
      "Привет  "
    );
    // Japanese keeps kana + kanji, strips Latin letters, keeps digits/punct.
    assert_eq!(
      remove_other_language_chars("カタカナ test 42!", &[Japanese]),
      "カタカナ  42!"
    );
    // Korean keeps Hangul (and Han).
    assert_eq!(
      remove_other_language_chars("한국어 한국", &[Korean]),
      "한국어 한국"
    );
    // Hebrew plus English as second language keeps both scripts.
    assert_eq!(
      remove_other_language_chars("שלום peace", &[Hebrew, English]),
      "שלום peace"
    );
    // Vietnamese keeps tone-marked vowels from Latin Extended Additional.
    assert_eq!(
      remove_other_language_chars("Xin chào thế giới", &[Vietnamese]),
      "Xin chào thế giới"
    );
    // English drops accented Latin-1 letters.
    assert_eq!(
      remove_other_language_chars("café naïve", &[English]),
      "caf nave"
    );
    // Turkish keeps ğ ı ş from Latin Extended-A / Latin-1.
    assert_eq!(
      remove_other_language_chars("yemek şişi", &[Turkish]),
      "yemek şişi"
    );
  }

  #[test]
  fn test_remove_other_language_chars_empty_keep() {
    assert_eq!(remove_other_language_chars("Hello 你好", &[]), "Hello 你好");
  }

  #[test]
  fn test_language_serde_round_trip() {
    let json = serde_json::to_string(&Language::Vietnamese).unwrap();
    assert_eq!(json, "\"Vietnamese\"");
    let back: Language = serde_json::from_str(&json).unwrap();
    assert_eq!(back, Language::Vietnamese);
  }

  #[test]
  fn test_merge_short_lines() {
    let input = "short\nline\nhere\nLONG_LINE_EXCEEDS_TEN_CHARS";
    let result = merge_short_lines(input, 10);
    assert!(result.contains("short line here"));
    assert!(result.contains("LONG_LINE_EXCEEDS_TEN_CHARS"));
  }

  #[test]
  fn test_remove_all_newlines() {
    assert_eq!(remove_all_newlines("a\nb\nc"), "a b c");
    assert_eq!(remove_all_newlines("a\n\nb"), "a b"); // double newline → single space
  }

  #[test]
  fn test_replace_newlines() {
    assert_eq!(replace_newlines("a\nb\nc", "|"), "a|b|c");
  }
}

#[cfg(test)]
#[test]
fn test_optimize_line_breaks_order() {
  let result = optimize_line_breaks("abc def ghijklmnop", 5);
  assert_eq!(
    result, "abc\ndef\nghijk\nlmnop",
    "got: {:?} — lines are in wrong order",
    result
  );
}
