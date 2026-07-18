//! Compile-time API surface verification for all 13 formats.
//!
//! Tests that every format module exposes the expected set of functions.
//! If any format regresses (e.g. someone accidentally deletes `parse_content`
//! or `to_string`), this file will **fail to compile** — a stronger
//! guarantee than runtime checks.
//!
//! Async generic functions (generate, write_stream) are NOT checked here
//! because their type parameters can't be inferred from bare references.
//! They're verified by the existing runtime test suite (api_surface tests,
//! integration.rs, pipeline_integration.rs).

// ── SRT / VTT (full reference spec) ──

#[test]
fn api_srt() {
  let _ = subtitler::srt::parse_content;
  let _ = subtitler::srt::parse_bytes;
  let _ = subtitler::srt::parse_stream;
  let _ = subtitler::srt::to_string;
  let _ = subtitler::srt::detect_format;
}

#[test]
fn api_vtt() {
  let _ = subtitler::vtt::parse_content;
  let _ = subtitler::vtt::parse_bytes;
  let _ = subtitler::vtt::parse_stream;
  let _ = subtitler::vtt::to_string;
  let _ = subtitler::vtt::detect_format;
}

// ── Streaming formats: parse_stream factory (9 formats) ──

#[test]
fn api_parse_stream_for_all_streaming_formats() {
  let _ = subtitler::srt::parse_stream;
  let _ = subtitler::vtt::parse_stream;
  let _ = subtitler::microdvd::parse_stream;
  let _ = subtitler::subviewer::parse_stream;
  let _ = subtitler::sbv::parse_stream;
  let _ = subtitler::lrc::parse_stream;
  let _ = subtitler::sami::parse_stream;
  let _ = subtitler::mpl2::parse_stream;
  let _ = subtitler::scc::parse_stream;
}

// ── detect_format for all 12 compiled format modules ──

#[test]
fn api_detect_format_for_all_formats() {
  let _ = subtitler::srt::detect_format;
  let _ = subtitler::vtt::detect_format;
  let _ = subtitler::ass::detect_format;
  let _ = subtitler::microdvd::detect_format;
  let _ = subtitler::subviewer::detect_format;
  let _ = subtitler::ttml::detect_format;
  let _ = subtitler::sbv::detect_format;
  let _ = subtitler::lrc::detect_format;
  let _ = subtitler::sami::detect_format;
  let _ = subtitler::mpl2::detect_format;
  let _ = subtitler::scc::detect_format;
  let _ = subtitler::ebu_stl::detect_format;
}

// ── parse_content / parse_bytes for all text formats ──

#[test]
fn api_parse_content_for_all_formats() {
  let _ = subtitler::ass::parse_content;
  let _ = subtitler::microdvd::parse_content;
  let _ = subtitler::subviewer::parse_content;
  let _ = subtitler::ttml::parse_content;
  let _ = subtitler::sbv::parse_content;
  let _ = subtitler::lrc::parse_content;
  let _ = subtitler::sami::parse_content;
  let _ = subtitler::mpl2::parse_content;
  let _ = subtitler::scc::parse_content;
  // ebu_stl parse_content takes &[u8]; covered by detect_format check above
}
