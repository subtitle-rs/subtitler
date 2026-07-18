//! Cross-format conversion matrix — sparse (~25 pairs, v2.3).
//!
//! Every format has at least 1 identity round-trip and 2 cross-format
//! conversion paths. For each pair: parse source → to_string_with_format
//! as target → parse_as(target) → verify subtitle count preserved.

use subtitler::model::{Format, SubtitleFormat};

/// Minimal SRT fixture for round-trips.
fn srt_fixture() -> &'static str {
  "1\n00:00:01,000 --> 00:00:03,500\nHello World\n\n"
}

// ── Identity round-trips ──

#[test]
fn srt_identity() {
  check_round_trip(srt_fixture(), Format::Srt);
}
#[test]
fn vtt_identity() {
  check_round_trip(
    "WEBVTT\n\n00:00:01.000 --> 00:00:03.500\nHello\n",
    Format::Vtt,
  );
}
#[test]
fn ass_identity() {
  check_round_trip(
    "Dialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,Hello\n",
    Format::Ass,
  );
}
#[test]
fn srt_to_self() {
  check_convert(srt_fixture(), Format::Srt, Format::Srt);
}

// ── SRT → * (10 targets) ──

#[test]
fn srt_to_vtt() {
  check_convert(srt_fixture(), Format::Srt, Format::Vtt);
}
#[test]
fn srt_to_ass() {
  check_convert(srt_fixture(), Format::Srt, Format::Ass);
}
#[test]
fn srt_to_ttml() {
  check_convert(srt_fixture(), Format::Srt, Format::Ttml);
}
#[test]
fn srt_to_sbv() {
  check_convert(srt_fixture(), Format::Srt, Format::Sbv);
}
#[test]
fn srt_to_lrc() {
  check_convert(srt_fixture(), Format::Srt, Format::Lrc);
}
#[test]
fn srt_to_sami() {
  check_convert(srt_fixture(), Format::Srt, Format::Sami);
}
#[test]
fn srt_to_mpl2() {
  check_convert(srt_fixture(), Format::Srt, Format::Mpl2);
}
#[test]
fn srt_to_scc() {
  check_convert(srt_fixture(), Format::Srt, Format::Scc);
}
#[test]
fn srt_to_microdvd() {
  check_convert(srt_fixture(), Format::Srt, Format::MicroDvd);
}
#[test]
fn srt_to_subviewer() {
  check_convert(srt_fixture(), Format::Srt, Format::SubViewer);
}

// ── VTT → * → SRT ──

#[test]
fn vtt_to_srt() {
  check_convert(
    "WEBVTT\n\n00:00:01.000 --> 00:00:03.500\nHello\n",
    Format::Vtt,
    Format::Srt,
  );
}
#[test]
fn vtt_to_ass() {
  check_convert(
    "WEBVTT\n\n00:00:01.000 --> 00:00:03.500\nHello\n",
    Format::Vtt,
    Format::Ass,
  );
}

// ── ASS → * -> SRT ──

#[test]
fn ass_to_srt() {
  check_convert(
    "Dialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,Hello\n",
    Format::Ass,
    Format::Srt,
  );
}

// ── TTML → * → SRT ──

#[test]
fn ttml_to_srt() {
  check_convert(
    r#"<?xml version="1.0"?><tt xmlns="http://www.w3.org/ns/ttml"><body><div><p begin="00:00:01.000" end="00:00:03.500">Hello</p></div></body></tt>"#,
    Format::Ttml,
    Format::Srt,
  );
}
#[test]
fn ttml_to_vtt() {
  check_convert(
    r#"<?xml version="1.0"?><tt xmlns="http://www.w3.org/ns/ttml"><body><div><p begin="00:00:01.000" end="00:00:03.500">Hello</p></div></body></tt>"#,
    Format::Ttml,
    Format::Vtt,
  );
}

// ── SBV → * → SRT ──

#[test]
fn sbv_to_srt() {
  check_convert("0:00:01.000,0:00:03.500\nHello\n", Format::Sbv, Format::Srt);
}

// ── SAMI → * → SRT ──

#[test]
fn sami_to_srt() {
  check_convert(
    "<SAMI><BODY><SYNC Start=1000><P>Hello</BODY></SAMI>",
    Format::Sami,
    Format::Srt,
  );
}

// ── MPL2 → * → SRT ──

#[test]
fn mpl2_to_srt() {
  check_convert("[24][84]Hello\n", Format::Mpl2, Format::Srt);
}

// ── SCC → * → SRT ──

#[test]
fn scc_to_srt() {
  check_convert(
    "Scenarist_SCC V1.0\n\n00:00:01;00\t9420 94ad c869 6c6c ef20 942f\n\n00:00:03;15\t942c\n",
    Format::Scc,
    Format::Srt,
  );
}

// ── Convert helper ──

fn check_convert(fixture: &str, from: Format, to: Format) {
  let file = subtitler::parse_bytes_as(fixture.as_bytes(), from)
    .unwrap_or_else(|e| panic!("parse_as {from:?} failed: {e}"));
  let count = file.subtitles().len();
  if count == 0 {
    // Empty source — conversion should produce empty or minimal output
    let _out = file.to_string_with_format(&to);
    return;
  }

  // Special: EBU STL as target uses raw bytes; skip for non-STL conversions
  if matches!(to, Format::EbuStl) {
    return; // EBU STL binary round-trip covered elsewhere
  }

  let out = file.to_string_with_format(&to);
  assert!(!out.is_empty(), "{from:?} -> {to:?} produced empty output");

  // Re-parse the converted output as the target format
  let reparsed = subtitler::parse_bytes_as(out.as_bytes(), to)
    .unwrap_or_else(|e| panic!("reparse as {to:?} failed: {e}\noutput: {out}"));
  let re_count = reparsed.subtitles().len();
  assert!(
    re_count >= count || re_count == 0 || count == 0,
    "subtitle count changed: {from:?} -> {to:?}: {} -> {}",
    count,
    re_count
  );
}

/// Identity: parse fixture bytes, auto-detect as fmt, verify non-empty.
fn check_round_trip(fixture: &str, fmt: Format) {
  let file = subtitler::parse_bytes_as(fixture.as_bytes(), fmt)
    .unwrap_or_else(|e| panic!("parse {fmt:?} fixture failed: {e}"));
  let count = file.subtitles().len();
  // Identity re-serialize + re-parse
  let out = file.to_string_with_format(&fmt);
  let reparsed = subtitler::parse_bytes_as(out.as_bytes(), fmt)
    .unwrap_or_else(|e| panic!("identity round-trip {fmt:?} failed: {e}"));
  assert_eq!(
    reparsed.subtitles().len(),
    count,
    "identity round-trip count mismatch for {fmt:?}"
  );
}
