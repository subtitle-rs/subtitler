//! Quality analysis and translation tooling for subtitles.
//!
//! ## Quality Report
//! Generate structured reports (JSON-serializable) with per-subtitle metrics,
//! timing issues, readability scores, and formatting suggestions.
//!
//! ## Translation API
//! The `Translator` trait defines an interface for machine translation
//! of subtitle content. Implementations can wrap any API.

use crate::model::{Subtitle, SubtitleFormat, ValidationIssue};
use serde::{Deserialize, Serialize};

// ── Quality Report ──

/// Quality metrics for a single subtitle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleQuality {
  pub index: usize,
  pub text: String,
  pub duration_ms: u64,
  pub chars_per_second: f64,
  pub words_per_minute: f64,
  pub char_count: usize,
  pub word_count: usize,
  pub issues: Vec<ValidationIssue>,
  pub has_poor_line_break: bool,
}

/// Overall quality report for a subtitle file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityReport {
  pub total_subtitles: usize,
  pub total_issues: usize,
  pub avg_duration_ms: u64,
  pub avg_cps: f64,
  pub avg_wpm: f64,
  pub subtitles: Vec<SubtitleQuality>,
}

/// Generate a quality report for a set of subtitles.
pub fn generate_report(
  subtitles: &[Subtitle],
  max_chars: usize,
  max_gap_ms: u64,
  max_cps: f64,
) -> QualityReport {
  let sub_qualities: Vec<SubtitleQuality> = subtitles
    .iter()
    .enumerate()
    .map(|(i, sub)| {
      let validate = crate::model::SubtitleFile::Srt(vec![sub.clone()])
        .validate_extended(max_chars, max_gap_ms, max_cps);
      let cps = sub.chars_per_second();
      let wpm = sub.reading_speed_wpm();
      let char_count = sub.plaintext().chars().count();
      let word_count = sub.plaintext().split_whitespace().count();
      let has_poor_break = sub.text.chars().count() > 42 && !sub.text.contains('\n');

      SubtitleQuality {
        index: i,
        text: sub.text.clone(),
        duration_ms: sub.duration_ms(),
        chars_per_second: cps,
        words_per_minute: wpm,
        char_count,
        word_count,
        issues: validate,
        has_poor_line_break: has_poor_break,
      }
    })
    .collect();

  let total_subtitles = subtitles.len();
  let total_issues: usize = sub_qualities.iter().map(|q| q.issues.len()).sum();
  let avg_duration_ms = if total_subtitles > 0 {
    sub_qualities.iter().map(|q| q.duration_ms).sum::<u64>() / total_subtitles as u64
  } else {
    0
  };
  let avg_cps = if total_subtitles > 0 {
    sub_qualities
      .iter()
      .map(|q| q.chars_per_second)
      .sum::<f64>()
      / total_subtitles as f64
  } else {
    0.0
  };
  let avg_wpm = if total_subtitles > 0 {
    sub_qualities
      .iter()
      .map(|q| q.words_per_minute)
      .sum::<f64>()
      / total_subtitles as f64
  } else {
    0.0
  };

  QualityReport {
    total_subtitles,
    total_issues,
    avg_duration_ms,
    avg_cps,
    avg_wpm,
    subtitles: sub_qualities,
  }
}

// ── Translation Trait ──

/// Interface for subtitle translation services.
///
/// Implementations can wrap cloud APIs (Google Translate, DeepL, etc.)
/// or local translation engines. The trait is intentionally simple to
/// allow multiple backends.
pub trait Translator: std::fmt::Debug {
  /// Translate a single line of subtitle text.
  fn translate(&self, text: &str, source_lang: &str, target_lang: &str) -> TranslatorResult;

  /// Translate all subtitles in a file, returning a new Vec.
  fn translate_file(
    &self,
    subtitles: &[Subtitle],
    source_lang: &str,
    target_lang: &str,
  ) -> Vec<Subtitle> {
    subtitles
      .iter()
      .map(|sub| {
        let mut s = sub.clone();
        if let Ok(t) = self.translate(&sub.text, source_lang, target_lang) {
          s.text = t;
        }
        s
      })
      .collect()
  }
}

/// Result type for translation operations.
pub type TranslatorResult = Result<String, String>;

/// A no-op translator that returns the input unchanged. Useful for testing
/// and as a default before connecting a real API.
#[derive(Debug)]
pub struct DummyTranslator;

impl Translator for DummyTranslator {
  fn translate(&self, text: &str, _source_lang: &str, _target_lang: &str) -> TranslatorResult {
    Ok(text.to_string())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::Subtitle;

  #[test]
  fn test_generate_report() {
    let subs = vec![
      Subtitle::new(1000, 3000, "Hello World"),
      Subtitle::new(
        4000,
        6000,
        "This is a very long subtitle line that exceeds recommended length",
      ),
    ];
    let report = generate_report(&subs, 42, 5000, 25.0);
    assert_eq!(report.total_subtitles, 2);
    assert!(report.total_issues > 0);
    assert!(report.subtitles[1].has_poor_line_break);
  }

  #[test]
  fn test_dummy_translator() {
    let t = DummyTranslator;
    let result = t.translate("Hello", "en", "es").unwrap();
    assert_eq!(result, "Hello");
  }
}
