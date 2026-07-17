use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum ValidationIssue {
  Overlap {
    index_a: usize,
    index_b: usize,
    end_a: u64,
    start_b: u64,
  },
  NegativeDuration {
    index: usize,
    start: u64,
    end: u64,
  },
  ZeroDuration {
    index: usize,
    time: u64,
  },
  DecreasingStartTime {
    index: usize,
    prev_start: u64,
    curr_start: u64,
  },
  TooLongGap {
    index: usize,
    prev_end: u64,
    curr_start: u64,
    gap_ms: u64,
  },
  TextTooLong {
    index: usize,
    chars: usize,
    max_chars: usize,
  },
  CpsTooHigh {
    index: usize,
    cps: f64,
    max_cps: f64,
  },
}

impl ValidationIssue {
  pub fn description(&self) -> String {
    match self {
      ValidationIssue::Overlap {
        index_a,
        index_b,
        end_a,
        start_b,
      } => {
        format!(
          "subtitle {index_a} (ends at {end_a}ms) overlaps with subtitle {index_b} (starts at {start_b}ms)"
        )
      }
      ValidationIssue::NegativeDuration { index, start, end } => {
        format!("subtitle {index} has negative duration: {start}ms -> {end}ms")
      }
      ValidationIssue::ZeroDuration { index, time } => {
        format!("subtitle {index} has zero duration at {time}ms")
      }
      ValidationIssue::DecreasingStartTime {
        index,
        prev_start,
        curr_start,
      } => {
        format!(
          "subtitle {index} starts at {curr_start}ms before previous subtitle's start time {prev_start}ms"
        )
      }
      ValidationIssue::TooLongGap {
        index,
        prev_end,
        curr_start,
        gap_ms,
      } => {
        format!("subtitle {index}: {gap_ms}ms gap between {prev_end}ms and {curr_start}ms")
      }
      ValidationIssue::TextTooLong {
        index,
        chars,
        max_chars,
      } => {
        format!("subtitle {index} has {chars} characters (max recommended: {max_chars})")
      }
      ValidationIssue::CpsTooHigh {
        index,
        cps,
        max_cps,
      } => {
        format!("subtitle {index} has {cps:.1} chars/second (max recommended: {max_cps:.1})")
      }
    }
  }
}
