//! Error type regression tests — verify that structured errors are used
//! everywhere and contain the expected variant data.

use subtitler::error::SubtitleError;
use subtitler::model::Format;

// ── Error construction ──

#[test]
fn error_invalid_timestamp_contains_format_and_value() {
  let e = SubtitleError::InvalidTimestamp {
    format: Format::Srt,
    value: "bad_time".into(),
  };
  let msg = e.to_string();
  assert!(msg.contains("Srt"));
  assert!(msg.contains("bad_time"));
}

#[test]
fn error_unexpected_line_contains_context() {
  let e = SubtitleError::UnexpectedLine {
    format: Format::Srt,
    row: 42,
    expected: "timestamp",
    got: "hello".into(),
  };
  let msg = e.to_string();
  assert!(msg.contains("42"));
  assert!(msg.contains("timestamp"));
  assert!(msg.contains("hello"));
}

#[test]
fn error_invalid_line_contains_format_and_line() {
  let e = SubtitleError::InvalidLine {
    format: Format::Sbv,
    line: "corrupted line".into(),
  };
  let msg = e.to_string();
  assert!(msg.contains("Sbv"));
  assert!(msg.contains("corrupted line"));
}

#[test]
fn error_invalid_format_stores_reason() {
  let e = SubtitleError::InvalidFormat {
    format: Format::EbuStl,
    reason: "GSI block size mismatch".into(),
  };
  let msg = e.to_string();
  assert!(msg.contains("EbuStl"));
  assert!(msg.contains("GSI block size mismatch"));
}

#[test]
fn error_unsupported_encoding_gives_helpful_hint() {
  let e = SubtitleError::UnsupportedEncoding {
    encoding: "EUC-KR".into(),
  };
  let msg = e.to_string();
  assert!(msg.contains("EUC-KR"));
  assert!(msg.contains("UTF-8"));
}

#[test]
fn error_from_utf8_error_conversion() {
  let bad = vec![0xFF, 0xFE, 0xFD];
  let utf8_err = String::from_utf8(bad).unwrap_err();
  let e = SubtitleError::from(utf8_err);
  assert!(matches!(e, SubtitleError::InvalidUtf8(_)));
}

#[test]
fn error_from_io_error_conversion() {
  let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
  let e = SubtitleError::from(io_err);
  assert!(matches!(e, SubtitleError::Io(_)));
  assert!(e.to_string().contains("file missing"));
}

// ── Error produced from actual parsing ──

#[test]
fn srt_bad_timestamp_returns_invalid_timestamp_error() {
  let err = subtitler::srt::parse_content("1\nnot a time\n").unwrap_err();
  let msg = format!("{}", err);
  // Either directly contains "expected timestamp" or "Invalid SRT"
  assert!(msg.contains("expected timestamp") || msg.contains("Invalid SRT"));
}

#[test]
fn vtt_bad_timestamp_returns_error() {
  let err = subtitler::vtt::parse_content("WEBVTT\n\n1\nnot a time\n").unwrap_err();
  let msg = format!("{}", err);
  assert!(msg.contains("expected timestamp") || msg.contains("Invalid"));
}

#[test]
fn srt_empty_is_ok() {
  let result = subtitler::srt::parse_content("");
  assert!(result.is_ok());
}

#[test]
fn vtt_empty_webvtt_is_ok() {
  let result = subtitler::vtt::parse_content("WEBVTT\n\n");
  assert!(result.is_ok());
}

// ── Error display format ──

#[test]
fn all_error_variants_implement_display() {
  let errors: [SubtitleError; 8] = [
    SubtitleError::InvalidTimestamp {
      format: Format::Srt,
      value: "test".into(),
    },
    SubtitleError::UnexpectedLine {
      format: Format::Srt,
      row: 1,
      expected: "ts",
      got: "x".into(),
    },
    SubtitleError::InvalidLine {
      format: Format::Sbv,
      line: "x".into(),
    },
    SubtitleError::InvalidFormat {
      format: Format::EbuStl,
      reason: "bad".into(),
    },
    SubtitleError::UnsupportedEncoding {
      encoding: "EUC-JP".into(),
    },
    SubtitleError::InvalidEncoding {
      encoding: "UTF-16".into(),
      error: "malformed".into(),
    },
    SubtitleError::FileExists {
      path: std::path::PathBuf::from("/tmp/x.srt"),
    },
    SubtitleError::InvalidUtf8(String::from_utf8(vec![0xC0, 0x80]).unwrap_err()),
  ];
  for e in &errors {
    let s = e.to_string();
    assert!(!s.is_empty(), "empty Display for {:?}", e);
  }
}
