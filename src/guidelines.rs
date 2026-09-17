//! Broadcast guideline presets for subtitle QC.
//!
//! Each preset encodes a broadcaster's published style guide as a plain
//! parameter set. A field set to `0` / `0.0` means "the guideline does not
//! state a hard limit" and the corresponding check is skipped.
//!
//! Sources (values verified against the published guides, not from memory):
//! - Netflix: Timed Text Style Guide, General Requirements + Subtitle Timing
//!   (partnerhelp.netflixstudios.com) — 42 chars/line, 2 lines, min duration
//!   5/6 s, max 7 s, min gap 2 frames, 20 CPS adult programs.
//! - TED: "Subtitling Tips" (ted.com/participate/translate/subtitling-tips) —
//!   42 chars/line, 2 lines, 1–7 s, max 21 CPS.
//! - ARD/ORF/SRF/ZDF: joint teletext subtitle standards (rbb-online.de,
//!   "Untertitel-Standards von ARD, ORF, SRF, ZDF") — 37 chars/line, 2 lines,
//!   min display time 1 s, reading speed base 13–15 CPS (upper bound used);
//!   max display time 4 s per untertitelrichtlinien.de.
//! - Channel 4: Subtitling Guidelines (channel4.com SG_FLP.pdf) — max 38
//!   chars/line, "allow 2 seconds per line" → 38/2 = 19 CPS.
//! - BBC: Subtitle Guidelines (bbc.co.uk/accessibility) — 37 chars/line
//!   (Teletext heritage), reading speed 160–180 wpm ≈ 17 CPS at ~5.7 chars
//!   per word. Duration limits are not stated as hard numbers → not checked.

use crate::model::{Subtitle, ValidationIssue};
use serde::{Deserialize, Serialize};

/// A subtitle QC rule set. `0` / `0.0` disables the corresponding check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Guideline {
  /// Display name of the guideline.
  pub name: String,
  /// Maximum characters per subtitle line (per line, not per cue).
  pub max_chars_per_line: usize,
  /// Maximum number of lines per subtitle.
  pub max_lines: usize,
  /// Minimum subtitle duration in milliseconds.
  pub min_duration_ms: u64,
  /// Maximum subtitle duration in milliseconds.
  pub max_duration_ms: u64,
  /// Minimum gap between consecutive (non-overlapping) subtitles, in ms.
  pub min_gap_ms: u64,
  /// Maximum reading speed in characters per second.
  pub max_cps: f64,
}

/// Built-in broadcaster guideline presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuidelinePreset {
  Netflix,
  Bbc,
  Ted,
  ArdOrfSrfZdf,
  Channel4,
}

impl GuidelinePreset {
  /// The parameter set published by this broadcaster.
  pub fn guideline(self) -> Guideline {
    match self {
      // 5/6 s = 833.3 ms (truncated); 2 frames at the 24 fps reference = 83.3 ms.
      GuidelinePreset::Netflix => Guideline {
        name: "Netflix".to_string(),
        max_chars_per_line: 42,
        max_lines: 2,
        min_duration_ms: 833,
        max_duration_ms: 7_000,
        min_gap_ms: 83,
        max_cps: 20.0,
      },
      GuidelinePreset::Bbc => Guideline {
        name: "BBC".to_string(),
        max_chars_per_line: 37,
        max_lines: 2,
        min_duration_ms: 0,
        max_duration_ms: 0,
        min_gap_ms: 0,
        max_cps: 17.0,
      },
      GuidelinePreset::Ted => Guideline {
        name: "TED".to_string(),
        max_chars_per_line: 42,
        max_lines: 2,
        min_duration_ms: 1_000,
        max_duration_ms: 7_000,
        min_gap_ms: 0,
        max_cps: 21.0,
      },
      GuidelinePreset::ArdOrfSrfZdf => Guideline {
        name: "ARD/ORF/SRF/ZDF".to_string(),
        max_chars_per_line: 37,
        max_lines: 2,
        min_duration_ms: 1_000,
        max_duration_ms: 4_000,
        min_gap_ms: 0,
        max_cps: 15.0,
      },
      GuidelinePreset::Channel4 => Guideline {
        name: "Channel 4".to_string(),
        max_chars_per_line: 38,
        max_lines: 2,
        min_duration_ms: 0,
        max_duration_ms: 0,
        min_gap_ms: 0,
        max_cps: 19.0,
      },
    }
  }
}

/// Validate subtitles against a guideline rule set.
///
/// Only the guideline's own checks are reported here; structural timing
/// issues (overlap, decreasing start, ...) come from `SubtitleFormat::validate`.
/// Gap checks run on the time-sorted order and skip overlapping pairs — an
/// overlap is already reported by `validate()` and double-flagging the same
/// pair as a short gap would be noise.
pub fn validate(subs: &[Subtitle], guideline: &Guideline) -> Vec<ValidationIssue> {
  let mut issues = Vec::new();

  for (i, sub) in subs.iter().enumerate() {
    let duration = sub.duration_ms();

    if guideline.min_duration_ms > 0 && duration < guideline.min_duration_ms {
      issues.push(ValidationIssue::TooShortDuration {
        index: i,
        duration_ms: duration,
        min_duration_ms: guideline.min_duration_ms,
      });
    }
    if guideline.max_duration_ms > 0 && duration > guideline.max_duration_ms {
      issues.push(ValidationIssue::TooLongDuration {
        index: i,
        duration_ms: duration,
        max_duration_ms: guideline.max_duration_ms,
      });
    }

    let lines: Vec<&str> = sub.text.lines().collect();
    if guideline.max_lines > 0 && lines.len() > guideline.max_lines {
      issues.push(ValidationIssue::LineCountExceeded {
        index: i,
        lines: lines.len(),
        max_lines: guideline.max_lines,
      });
    }

    if guideline.max_chars_per_line > 0 {
      if let Some(longest) = lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .filter(|&chars| chars > guideline.max_chars_per_line)
      {
        issues.push(ValidationIssue::TextTooLong {
          index: i,
          chars: longest,
          max_chars: guideline.max_chars_per_line,
        });
      }
    }

    if guideline.max_cps > 0.0 {
      let cps = sub.chars_per_second();
      if cps > guideline.max_cps {
        issues.push(ValidationIssue::CpsTooHigh {
          index: i,
          cps,
          max_cps: guideline.max_cps,
        });
      }
    }
  }

  if guideline.min_gap_ms > 0 {
    let mut order: Vec<usize> = (0..subs.len()).collect();
    order.sort_by_key(|&i| (subs[i].start, subs[i].end));
    for w in order.windows(2) {
      let (a, b) = (w[0], w[1]);
      if subs[b].start < subs[a].end {
        continue; // overlapping pair — reported by validate() as Overlap
      }
      let gap = subs[b].start - subs[a].end;
      if gap < guideline.min_gap_ms {
        issues.push(ValidationIssue::TooShortGap {
          index: b,
          gap_ms: gap,
          min_gap_ms: guideline.min_gap_ms,
        });
      }
    }
  }

  issues
}

#[cfg(test)]
mod tests {
  use super::*;

  fn netflix() -> Guideline {
    GuidelinePreset::Netflix.guideline()
  }

  #[test]
  fn test_preset_values_match_published_guides() {
    let g = netflix();
    assert_eq!(g.max_chars_per_line, 42);
    assert_eq!(g.max_lines, 2);
    assert_eq!(g.min_duration_ms, 833); // 5/6 s
    assert_eq!(g.max_duration_ms, 7_000);
    assert_eq!(g.min_gap_ms, 83); // 2 frames @ 24 fps
    assert_eq!(g.max_cps, 20.0);

    let ted = GuidelinePreset::Ted.guideline();
    assert_eq!(ted.max_cps, 21.0);
    assert_eq!(ted.min_duration_ms, 1_000);

    let ard = GuidelinePreset::ArdOrfSrfZdf.guideline();
    assert_eq!(ard.max_chars_per_line, 37);
    assert_eq!(ard.max_cps, 15.0);

    let c4 = GuidelinePreset::Channel4.guideline();
    assert_eq!(c4.max_chars_per_line, 38);
    assert_eq!(c4.max_cps, 19.0);

    let bbc = GuidelinePreset::Bbc.guideline();
    assert_eq!(bbc.max_chars_per_line, 37);
    assert_eq!(bbc.min_duration_ms, 0, "BBC states no hard min duration");
  }

  #[test]
  fn test_too_short_duration() {
    let subs = vec![Subtitle::new(1_000, 1_400, "too quick")]; // 400 ms < 833
    let issues = validate(&subs, &netflix());
    assert!(issues.iter().any(|i| matches!(
      i,
      ValidationIssue::TooShortDuration {
        duration_ms: 400,
        min_duration_ms: 833,
        ..
      }
    )));
  }

  #[test]
  fn test_too_long_duration() {
    let subs = vec![Subtitle::new(0, 8_000, "stays up too long")];
    let issues = validate(&subs, &netflix());
    assert!(issues.iter().any(|i| matches!(
      i,
      ValidationIssue::TooLongDuration {
        duration_ms: 8_000,
        ..
      }
    )));
  }

  #[test]
  fn test_line_count_exceeded() {
    let subs = vec![Subtitle::new(0, 2_000, "one\ntwo\nthree")];
    let issues = validate(&subs, &netflix());
    assert!(issues.iter().any(|i| matches!(
      i,
      ValidationIssue::LineCountExceeded {
        lines: 3,
        max_lines: 2,
        ..
      }
    )));
  }

  #[test]
  fn test_per_line_char_limit_not_per_cue() {
    // Two lines of 40 chars each = 80 chars total, but per-line it is fine.
    let line = "a".repeat(40);
    let subs = vec![Subtitle::new(0, 2_000, &format!("{line}\n{line}"))];
    let issues = validate(&subs, &netflix());
    assert!(
      !issues
        .iter()
        .any(|i| matches!(i, ValidationIssue::TextTooLong { .. })),
      "80 chars across two 40-char lines must not be flagged"
    );
  }

  #[test]
  fn test_line_too_long_flagged_with_offending_line_length() {
    let line = "b".repeat(45);
    let subs = vec![Subtitle::new(0, 2_000, &line)];
    let issues = validate(&subs, &netflix());
    assert!(issues.iter().any(|i| matches!(
      i,
      ValidationIssue::TextTooLong {
        chars: 45,
        max_chars: 42,
        ..
      }
    )));
  }

  #[test]
  fn test_cps_too_high() {
    // 30 chars / 1 s = 30 CPS > 20.
    let text = "c".repeat(30);
    let subs = vec![Subtitle::new(0, 1_000, &text)];
    let issues = validate(&subs, &netflix());
    assert!(
      issues.iter().any(
        |i| matches!(i, ValidationIssue::CpsTooHigh { cps, max_cps: 20.0, .. } if *cps > 20.0)
      )
    );
  }

  #[test]
  fn test_too_short_gap() {
    // Gap of 40 ms < 83 ms (2 frames @ 24 fps).
    let subs = vec![
      Subtitle::new(0, 1_000, "first"),
      Subtitle::new(1_040, 2_000, "second"),
    ];
    let issues = validate(&subs, &netflix());
    assert!(issues.iter().any(|i| matches!(
      i,
      ValidationIssue::TooShortGap {
        gap_ms: 40,
        min_gap_ms: 83,
        ..
      }
    )));
  }

  #[test]
  fn test_gap_check_skips_overlapping_pairs() {
    let subs = vec![
      Subtitle::new(0, 1_000, "first"),
      Subtitle::new(500, 2_000, "second"), // overlaps, not a "gap"
    ];
    let issues = validate(&subs, &netflix());
    assert!(
      !issues
        .iter()
        .any(|i| matches!(i, ValidationIssue::TooShortGap { .. })),
      "overlapping pairs are validate()'s job, not a short gap"
    );
  }

  #[test]
  fn test_gap_check_sorts_before_comparing() {
    // Unsorted input: the pair is adjacent in time order regardless.
    let subs = vec![
      Subtitle::new(1_040, 2_000, "second"),
      Subtitle::new(0, 1_000, "first"),
    ];
    let issues = validate(&subs, &netflix());
    assert!(
      issues
        .iter()
        .any(|i| matches!(i, ValidationIssue::TooShortGap { gap_ms: 40, .. }))
    );
  }

  #[test]
  fn test_zero_disables_check() {
    let mut g = netflix();
    g.max_cps = 0.0;
    g.min_duration_ms = 0;
    g.max_lines = 0;
    let subs = vec![
      Subtitle::new(0, 200, "instant but who cares"),
      Subtitle::new(300, 400, "x\ny\nz"),
    ];
    let issues = validate(&subs, &g);
    assert!(
      !issues.iter().any(|i| matches!(
        i,
        ValidationIssue::CpsTooHigh { .. }
          | ValidationIssue::TooShortDuration { .. }
          | ValidationIssue::LineCountExceeded { .. }
      )),
      "zeroed fields must disable their checks, got: {:?}",
      issues
    );
  }

  #[test]
  fn test_guideline_serde_round_trip() {
    let g = netflix();
    let json = serde_json::to_string(&g).unwrap();
    let back: Guideline = serde_json::from_str(&json).unwrap();
    assert_eq!(g, back);

    let preset_json = serde_json::to_string(&GuidelinePreset::Ted).unwrap();
    assert_eq!(preset_json, "\"Ted\"");
    let preset: GuidelinePreset = serde_json::from_str(&preset_json).unwrap();
    assert_eq!(preset, GuidelinePreset::Ted);
  }

  #[test]
  fn test_clean_file_passes_netflix() {
    // 40-char line, 2 s duration (12.5 CPS), 1 s gaps.
    let text = "d".repeat(40);
    let subs = vec![
      Subtitle::new(0, 2_000, &text),
      Subtitle::new(3_000, 5_000, &text),
    ];
    assert!(validate(&subs, &netflix()).is_empty());
  }
}
