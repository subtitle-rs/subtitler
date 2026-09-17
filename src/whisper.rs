//! Whisper AI JSON transcript format parser and generator.
//!
//! OpenAI's Whisper speech recognition outputs JSON transcripts in the format:
//! ```json
//! {"text": "Hello world", "segments": [{"start": 0.0, "end": 2.0, "text": "Hello world", ...}]}
//! ```
//!
//! This module parses that JSON into `SubtitleFile`, converting seconds
//! (f64) to milliseconds (u64 via ×1000 round).

use crate::model::{Format, Subtitle, SubtitleFile};
use crate::types::AnyResult;
use serde::Deserialize;

/// A single segment from a Whisper JSON transcript.
#[derive(Debug, Deserialize)]
struct WhisperSegment {
  start: f64,
  end: f64,
  text: String,
}

/// Top-level Whisper JSON transcript.
#[derive(Debug, Deserialize)]
struct WhisperTranscript {
  segments: Vec<WhisperSegment>,
}

/// Detect Whisper JSON format — looks for the segment shape
/// (`"segments"` with `"start"`/`"end"`) or the word-level shape
/// (`"words"` with `"start"`/`"end"`).
pub fn detect_format(data: &[u8]) -> Option<Format> {
  let text = crate::encoding::try_decode_for_detection(data)?;
  let has_times = text.contains("\"start\"") && text.contains("\"end\"");
  if (text.contains("\"segments\"") || text.contains("\"words\"")) && has_times {
    return Some(Format::Whisper);
  }
  None
}

/// Parse Whisper JSON content.
pub fn parse_content(content: &str) -> AnyResult<SubtitleFile> {
  let transcript: WhisperTranscript =
    serde_json::from_str(content).map_err(|e| anyhow::anyhow!("invalid Whisper JSON: {}", e))?;

  let subtitles: Vec<Subtitle> = transcript
    .segments
    .into_iter()
    .enumerate()
    .map(|(i, seg)| {
      let start_ms = (seg.start * 1000.0).round() as u64;
      let end_ms = (seg.end * 1000.0).round() as u64;
      Subtitle::new(start_ms, end_ms, &seg.text).with_index(i + 1)
    })
    .collect();

  Ok(SubtitleFile::Whisper(subtitles))
}

/// Parse Whisper JSON bytes — auto-detect encoding then parse.
pub fn parse_bytes(data: &[u8]) -> AnyResult<SubtitleFile> {
  let content = crate::encoding::decode_to_string(data)?;
  parse_content(&content)
}

/// Parse Whisper JSON from file.
#[cfg(not(target_arch = "wasm32"))]
pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<SubtitleFile> {
  let text = tokio::fs::read_to_string(path).await?;
  parse_content(&text)
}

/// Parse Whisper JSON from URL (requires `http` feature).
#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<SubtitleFile> {
  let response = reqwest::get(url).await?;
  let text = response.text().await?;
  parse_content(&text)
}

/// Serialize subtitles to Whisper JSON format.
pub fn to_string(subtitles: &[Subtitle]) -> String {
  let segments: Vec<serde_json::Value> = subtitles
    .iter()
    .map(|sub| {
      serde_json::json!({
        "id": sub.index.unwrap_or(0),
        "start": sub.start as f64 / 1000.0,
        "end": sub.end as f64 / 1000.0,
        "text": sub.text,
        "seek": 0,
        "tokens": [],
        "temperature": 0.0,
        "avg_logprob": -1.0,
        "compression_ratio": 1.0,
        "no_speech_prob": 0.0,
      })
    })
    .collect();

  let full_text: String = subtitles
    .iter()
    .map(|s| s.text.as_str())
    .collect::<Vec<_>>()
    .join(" ");

  serde_json::to_string_pretty(&serde_json::json!({
    "text": full_text,
    "segments": segments,
  }))
  .unwrap_or_else(|_| "{}".to_string())
}

/// Write subtitles to a file in Whisper JSON format.
#[cfg(not(target_arch = "wasm32"))]
pub async fn generate(
  subtitles: &[Subtitle],
  file_path: impl AsRef<std::path::Path>,
  policy: Option<crate::model::WritePolicy>,
) -> AnyResult<String> {
  let content = to_string(subtitles);
  let path = file_path.as_ref();
  crate::io::write_with_policy(path, content.as_bytes(), policy).await?;
  Ok(path.to_string_lossy().into_owned())
}

// ── Word-level transcript merging ──

/// One word from a word-level Whisper transcript.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WhisperWord {
  /// The word as transcribed (may carry trailing punctuation).
  pub word: String,
  /// Word onset in milliseconds.
  pub start_ms: u64,
  /// Word offset in milliseconds.
  pub end_ms: u64,
}

/// Grouping rules for [`merge_words`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WordGroupingOptions {
  /// Maximum characters per cue, including separating spaces.
  pub max_chars: usize,
  /// Maximum cue duration in milliseconds.
  pub max_duration_ms: u64,
  /// Start a new cue when the silence before a word exceeds this.
  pub max_gap_ms: u64,
  /// Maximum words per cue; `0` means unlimited.
  pub max_words: usize,
}

impl Default for WordGroupingOptions {
  fn default() -> Self {
    WordGroupingOptions {
      max_chars: 42,
      max_duration_ms: 5_000,
      max_gap_ms: 1_000,
      max_words: 0,
    }
  }
}

#[derive(Debug, serde::Deserialize)]
struct RawWord {
  word: String,
  start: f64,
  end: f64,
}

#[derive(Debug, serde::Deserialize)]
struct WordsContainer {
  #[serde(default)]
  words: Vec<RawWord>,
}

#[derive(Debug, serde::Deserialize)]
struct SegmentedWords {
  #[serde(default)]
  segments: Vec<WordsContainer>,
}

/// Extract the word-level timeline from Whisper JSON — either the
/// top-level `"words"` array or the per-segment `segments[].words` arrays.
pub fn parse_words_json(content: &str) -> AnyResult<Vec<WhisperWord>> {
  let mut raw: Vec<RawWord> = Vec::new();
  match serde_json::from_str::<WordsContainer>(content) {
    Ok(c) if !c.words.is_empty() => raw = c.words,
    _ => {
      if let Ok(s) = serde_json::from_str::<SegmentedWords>(content) {
        raw = s.segments.into_iter().flat_map(|seg| seg.words).collect();
      }
    }
  }
  if raw.is_empty() {
    anyhow::bail!("no word-level timestamps found in Whisper JSON");
  }
  Ok(
    raw
      .into_iter()
      .map(|w| WhisperWord {
        word: w.word,
        start_ms: (w.start * 1000.0).round() as u64,
        end_ms: (w.end * 1000.0).round() as u64,
      })
      .collect(),
  )
}

/// Group a word-level timeline into subtitle cues.
///
/// Greedy left-to-right packing: a word joins the current cue unless it
/// would exceed `max_chars`, stretch the cue past `max_duration_ms`,
/// follow a silence longer than `max_gap_ms`, or push the cue over
/// `max_words`. A word ending in sentence punctuation (`.`, `!`, `?`)
/// closes the cue after itself — simple and punctuation-naive ("Mr."
/// breaks early), matching the experimental tools this mirrors.
pub fn merge_words(words: &[WhisperWord], opts: &WordGroupingOptions) -> Vec<Subtitle> {
  let mut cues: Vec<Subtitle> = Vec::new();
  let mut text = String::new();
  let mut start = 0u64;
  let mut end = 0u64;
  let mut count = 0usize;

  for w in words {
    let word = w.word.trim();
    if word.is_empty() {
      continue;
    }
    let gap = w.start_ms.saturating_sub(end);
    let joined_len = if text.is_empty() {
      word.chars().count()
    } else {
      text.chars().count() + 1 + word.chars().count()
    };
    let must_close = count > 0
      && (gap > opts.max_gap_ms
        || joined_len > opts.max_chars
        || w.end_ms.saturating_sub(start) > opts.max_duration_ms
        || (opts.max_words > 0 && count >= opts.max_words));
    if must_close {
      cues.push(Subtitle::new(start, end, &text).with_index(cues.len() + 1));
      text.clear();
      count = 0;
    }
    if count == 0 {
      start = w.start_ms;
    }
    if !text.is_empty() {
      text.push(' ');
    }
    text.push_str(word);
    end = end.max(w.end_ms);
    count += 1;

    if word.ends_with(['.', '!', '?']) {
      cues.push(Subtitle::new(start, end, &text).with_index(cues.len() + 1));
      text.clear();
      count = 0;
    }
  }
  if !text.is_empty() {
    cues.push(Subtitle::new(start, end, &text).with_index(cues.len() + 1));
  }
  cues
}

/// Parse Whisper JSON and build cues from its word-level timestamps
/// instead of the segment level (the "merge word-by-word transcript
/// into subtitle" flow). Fails when the file has no word timeline.
pub fn parse_content_as_words(
  content: &str,
  opts: &WordGroupingOptions,
) -> AnyResult<SubtitleFile> {
  let words = parse_words_json(content)?;
  Ok(SubtitleFile::Whisper(merge_words(&words, opts)))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::SubtitleFormat;

  #[test]
  fn test_detect_whisper() {
    let data = br#"{"text": "Hello world", "segments": [{"start": 0.0, "end": 2.0, "text": "Hello world"}]}"#;
    assert_eq!(detect_format(data), Some(Format::Whisper));
  }

  #[test]
  fn test_detect_whisper_not_srt() {
    assert_eq!(
      detect_format(b"1\n00:00:01,000 --> 00:00:03,500\nHello\n"),
      None
    );
  }

  #[test]
  fn test_parse_whisper() {
    let json = r#"{"text": "Hello world", "segments": [{"start": 0.0, "end": 2.0, "text": "Hello world"}, {"start": 3.0, "end": 5.0, "text": "Goodbye"}]}"#;
    let file = parse_content(json).unwrap();
    let subs = file.subtitles();
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0].start, 0);
    assert_eq!(subs[0].end, 2000);
    assert_eq!(subs[0].text, "Hello world");
    assert_eq!(subs[1].start, 3000);
  }

  #[test]
  fn test_round_trip() {
    let json =
      r#"{"text": "Hello world", "segments": [{"start": 1.5, "end": 3.7, "text": "Hello world"}]}"#;
    let file = parse_content(json).unwrap();
    let out = to_string(file.subtitles());
    let reparsed = parse_content(&out).unwrap();
    assert_eq!(reparsed.subtitles().len(), 1);
    assert_eq!(reparsed.subtitles()[0].text, "Hello world");
  }

  fn word(w: &str, start: u64, end: u64) -> WhisperWord {
    WhisperWord {
      word: w.to_string(),
      start_ms: start,
      end_ms: end,
    }
  }

  #[test]
  fn test_parse_words_json_top_level() {
    let json = r#"{"text": "a b", "words": [
      {"word": "a", "start": 0.0, "end": 0.1},
      {"word": "b", "start": 0.2, "end": 0.3}]}"#;
    let words = parse_words_json(json).unwrap();
    assert_eq!(words.len(), 2);
    assert_eq!(words[1].start_ms, 200);
  }

  #[test]
  fn test_parse_words_json_per_segment() {
    let json = r#"{"segments": [
      {"words": [{"word": "a", "start": 0.0, "end": 0.1}]},
      {"words": [{"word": "b", "start": 0.2, "end": 0.3}]}]}"#;
    let words = parse_words_json(json).unwrap();
    assert_eq!(words.len(), 2);
  }

  #[test]
  fn test_parse_words_json_missing_words_fails() {
    let json = r#"{"text": "Hello", "segments": [{"start": 0.0, "end": 1.0, "text": "Hello"}]}"#;
    assert!(parse_words_json(json).is_err());
  }

  #[test]
  fn test_merge_words_groups_by_chars() {
    let words = vec![
      word("aaa", 0, 100),
      word("bbb", 150, 250),
      word("ccc", 300, 400),
    ];
    let opts = WordGroupingOptions {
      max_chars: 7, // "aaa bbb" = 7 fits, + " ccc" = 11 does not
      ..Default::default()
    };
    let cues = merge_words(&words, &opts);
    assert_eq!(cues.len(), 2);
    assert_eq!(cues[0].text, "aaa bbb");
    assert_eq!(cues[0].start, 0);
    assert_eq!(cues[0].end, 250);
    assert_eq!(cues[1].text, "ccc");
  }

  #[test]
  fn test_merge_words_breaks_on_gap() {
    let words = vec![
      word("before", 0, 500),
      word("after", 2_000, 2_500), // 1500 ms silence
    ];
    let opts = WordGroupingOptions {
      max_gap_ms: 1_000,
      ..Default::default()
    };
    let cues = merge_words(&words, &opts);
    assert_eq!(cues.len(), 2);
    assert_eq!(cues[0].text, "before");
    assert_eq!(cues[1].text, "after");
    assert_eq!(cues[1].start, 2_000);
  }

  #[test]
  fn test_merge_words_breaks_on_max_words() {
    let words = vec![
      word("one", 0, 100),
      word("two", 150, 250),
      word("three", 300, 400),
    ];
    let opts = WordGroupingOptions {
      max_words: 2,
      ..Default::default()
    };
    let cues = merge_words(&words, &opts);
    assert_eq!(cues.len(), 2);
    assert_eq!(cues[0].text, "one two");
    assert_eq!(cues[1].text, "three");
  }

  #[test]
  fn test_merge_words_breaks_on_sentence_punctuation() {
    let words = vec![
      word("Hello", 0, 100),
      word("world.", 150, 300),
      word("Next", 350, 500),
    ];
    let cues = merge_words(&words, &WordGroupingOptions::default());
    assert_eq!(cues.len(), 2);
    assert_eq!(cues[0].text, "Hello world.");
    assert_eq!(cues[1].text, "Next");
  }

  #[test]
  fn test_merge_words_breaks_on_duration() {
    let words = vec![
      word("slow", 0, 100),
      word("speech", 200, 6_000), // cue would stretch past 5 s
    ];
    let opts = WordGroupingOptions {
      max_duration_ms: 5_000,
      ..Default::default()
    };
    let cues = merge_words(&words, &opts);
    assert_eq!(cues.len(), 2);
    assert_eq!(cues[1].text, "speech");
    assert_eq!(cues[1].start, 200);
  }

  #[test]
  fn test_parse_content_as_words() {
    let json = r#"{"text": "Hi there", "words": [
      {"word": "Hi", "start": 0.0, "end": 0.2},
      {"word": "there", "start": 0.3, "end": 0.6}]}"#;
    let file = parse_content_as_words(json, &WordGroupingOptions::default()).unwrap();
    let subs = file.subtitles();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].text, "Hi there");
    assert_eq!(subs[0].end, 600);
  }
}
