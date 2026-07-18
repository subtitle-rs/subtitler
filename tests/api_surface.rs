//! Compile-time API surface verification for all formats.
#![cfg(not(target_arch = "wasm32"))]
//!
//! Tests that every format module exposes the expected set of functions.
//! If any format regresses, this file will **fail to compile**.
//!
//! Each reference is gated behind the corresponding feature flag so
//! that cargo test --no-default-features --features srt --lib passes.

// ── SRT / VTT (full reference spec) ──

#[test]
#[cfg(feature = "srt")]
fn api_srt() {
  let _ = subtitler::srt::parse_content;
  let _ = subtitler::srt::parse_bytes;
  let _ = subtitler::srt::parse_stream;
  let _ = subtitler::srt::to_string;
  let _ = subtitler::srt::detect_format;
}

#[test]
#[cfg(feature = "vtt")]
fn api_vtt() {
  let _ = subtitler::vtt::parse_content;
  let _ = subtitler::vtt::parse_bytes;
  let _ = subtitler::vtt::parse_stream;
  let _ = subtitler::vtt::to_string;
  let _ = subtitler::vtt::detect_format;
}

// ── Streaming formats: parse_stream factory ──

#[test]
fn api_parse_stream_for_all_streaming_formats() {
  #[cfg(feature = "srt")]
  let _ = subtitler::srt::parse_stream;
  #[cfg(feature = "vtt")]
  let _ = subtitler::vtt::parse_stream;
  #[cfg(feature = "microdvd")]
  let _ = subtitler::microdvd::parse_stream;
  #[cfg(feature = "subviewer")]
  let _ = subtitler::subviewer::parse_stream;
  #[cfg(feature = "sbv")]
  let _ = subtitler::sbv::parse_stream;
  #[cfg(feature = "lrc")]
  let _ = subtitler::lrc::parse_stream;
  #[cfg(feature = "sami")]
  let _ = subtitler::sami::parse_stream;
  #[cfg(feature = "mpl2")]
  let _ = subtitler::mpl2::parse_stream;
  #[cfg(feature = "scc")]
  let _ = subtitler::scc::parse_stream;
}

// ── detect_format ──

#[test]
fn api_detect_format_for_all_formats() {
  #[cfg(feature = "srt")]
  let _ = subtitler::srt::detect_format;
  #[cfg(feature = "vtt")]
  let _ = subtitler::vtt::detect_format;
  #[cfg(feature = "ass")]
  let _ = subtitler::ass::detect_format;
  #[cfg(feature = "microdvd")]
  let _ = subtitler::microdvd::detect_format;
  #[cfg(feature = "subviewer")]
  let _ = subtitler::subviewer::detect_format;
  #[cfg(feature = "ttml")]
  let _ = subtitler::ttml::detect_format;
  #[cfg(feature = "sbv")]
  let _ = subtitler::sbv::detect_format;
  #[cfg(feature = "lrc")]
  let _ = subtitler::lrc::detect_format;
  #[cfg(feature = "sami")]
  let _ = subtitler::sami::detect_format;
  #[cfg(feature = "mpl2")]
  let _ = subtitler::mpl2::detect_format;
  #[cfg(feature = "scc")]
  let _ = subtitler::scc::detect_format;
  #[cfg(feature = "ebu_stl")]
  let _ = subtitler::ebu_stl::detect_format;
}

// ── parse_content ──

#[test]
fn api_parse_content_for_all_formats() {
  #[cfg(feature = "ass")]
  let _ = subtitler::ass::parse_content;
  #[cfg(feature = "microdvd")]
  let _ = subtitler::microdvd::parse_content;
  #[cfg(feature = "subviewer")]
  let _ = subtitler::subviewer::parse_content;
  #[cfg(feature = "ttml")]
  let _ = subtitler::ttml::parse_content;
  #[cfg(feature = "sbv")]
  let _ = subtitler::sbv::parse_content;
  #[cfg(feature = "lrc")]
  let _ = subtitler::lrc::parse_content;
  #[cfg(feature = "sami")]
  let _ = subtitler::sami::parse_content;
  #[cfg(feature = "mpl2")]
  let _ = subtitler::mpl2::parse_content;
  #[cfg(feature = "scc")]
  let _ = subtitler::scc::parse_content;
}
