//! Typed errors for subtitle parsing.
//!
//! Existing public functions still return `AnyResult<T>` (alias for
//! `Result<T, anyhow::Error>`). This module provides structured error variants
//! for new code and gradual migration. `SubtitleError` converts into
//! `anyhow::Error` automatically via `thiserror`, so a function returning
//! `Result<_, SubtitleError>` interops with `AnyResult` callers.

use crate::model::Format;
use thiserror::Error;

/// Unified parse error for the high-level `subtitler::parse_*` entry points.
#[derive(Debug, Error)]
pub enum ParseError {
  #[error("could not detect subtitle format")]
  UnknownFormat,
  #[error("format {0:?} is not enabled (enable its cargo feature)")]
  Unsupported(Format),
  #[error("internal error: {0}")]
  Anyhow(#[from] anyhow::Error),
  #[error("parse error: {0}")]
  Decode(#[from] SubtitleError),
  #[error("I/O error: {0}")]
  Io(#[from] std::io::Error),
  #[cfg(feature = "http")]
  #[error("HTTP error: {0}")]
  Http(#[from] reqwest::Error),
}

#[derive(Debug, Error)]
pub enum SubtitleError {
  #[error("invalid timestamp format: {0}")]
  InvalidTimestamp(String),

  #[error("expected {expected} at row {row}, got: {got}")]
  UnexpectedLine {
    row: usize,
    expected: &'static str,
    got: String,
  },

  #[error("invalid UTF-8: {0}")]
  InvalidUtf8(#[from] std::string::FromUtf8Error),

  #[error("I/O error: {0}")]
  Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn invalid_timestamp_display() {
    let e = SubtitleError::InvalidTimestamp("nope".into());
    assert_eq!(e.to_string(), "invalid timestamp format: nope");
  }

  #[test]
  fn unexpected_line_display() {
    let e = SubtitleError::UnexpectedLine {
      row: 7,
      expected: "timestamp",
      got: "hello".into(),
    };
    assert_eq!(e.to_string(), "expected timestamp at row 7, got: hello");
  }

  #[test]
  fn from_utf8_error_wraps() {
    let bad = vec![0xFF, 0xFE, 0xFD];
    let utf8_err = String::from_utf8(bad).unwrap_err();
    let e = SubtitleError::from(utf8_err);
    assert!(matches!(e, SubtitleError::InvalidUtf8(_)));
    assert!(e.to_string().starts_with("invalid UTF-8:"));
  }
}
