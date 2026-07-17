use crate::model::Format;
use std::path::PathBuf;
use thiserror::Error;

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
  #[error("invalid timestamp in {format:?}: {value}")]
  InvalidTimestamp { format: Format, value: String },

  #[error("expected {expected} at row {row} in {format:?}, got: {got}")]
  UnexpectedLine {
    format: Format,
    row: usize,
    expected: &'static str,
    got: String,
  },

  #[error("invalid {format:?} line: {line}")]
  InvalidLine { format: Format, line: String },

  #[error("XML error in {format:?}: {error}")]
  Xml { format: Format, error: String },

  #[error("invalid {role} frame in {format:?}: {value}")]
  InvalidFrame {
    format: Format,
    role: &'static str,
    value: String,
  },

  #[error("invalid {encoding} encoding: {error}")]
  InvalidEncoding { encoding: String, error: String },

  #[error("unsupported encoding: {encoding}. Try converting to UTF-8 first.")]
  UnsupportedEncoding { encoding: String },

  #[error("invalid {format:?} data: {reason}")]
  InvalidFormat { format: Format, reason: String },

  #[error("refusing to overwrite existing file: {path}")]
  FileExists { path: PathBuf },

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
    let e = SubtitleError::InvalidTimestamp {
      format: Format::Srt,
      value: "nope".into(),
    };
    assert_eq!(e.to_string(), "invalid timestamp in Srt: nope");
  }

  #[test]
  fn unexpected_line_display() {
    let e = SubtitleError::UnexpectedLine {
      format: Format::Srt,
      row: 7,
      expected: "timestamp",
      got: "hello".into(),
    };
    assert_eq!(
      e.to_string(),
      "expected timestamp at row 7 in Srt, got: hello"
    );
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
