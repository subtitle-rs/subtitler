//! Cross-format conversion tests — verify data survives SRT→VTT→ASS→SRT etc.
#![cfg(not(target_arch = "wasm32"))]

use subtitler::model::{Format, Subtitle, SubtitleFile, SubtitleFormat};

fn sample_srt_file() -> SubtitleFile {
  SubtitleFile::Srt(vec![
    Subtitle::new(1000, 3000, "Hello World"),
    Subtitle::new(4000, 6000, "Second line"),
    Subtitle::new(7000, 9000, "Third"),
  ])
}

#[test]
fn srt_to_vtt_round_trip() {
  let original = sample_srt_file();
  let vtt_str = original.to_string_with_format(&Format::Vtt);
  // Re-parse the VTT output as VTT
  let reparsed = subtitler::parse_bytes(vtt_str.as_bytes()).unwrap();
  assert_eq!(reparsed.format(), Format::Vtt);
  assert_eq!(reparsed.subtitles().len(), 3);
  assert_eq!(reparsed.subtitles()[0].start, 1000);
  assert_eq!(reparsed.subtitles()[0].text, "Hello World");
}

#[test]
fn srt_to_ass_round_trip() {
  let original = sample_srt_file();
  let ass_str = original.to_string_with_format(&Format::Ass);
  let reparsed = subtitler::parse_bytes(ass_str.as_bytes()).unwrap();
  assert_eq!(reparsed.format(), Format::Ass);
  assert_eq!(reparsed.subtitles().len(), 3);
  assert_eq!(reparsed.subtitles()[0].start, 1000);
}

#[test]
fn srt_to_srt_idempotent() {
  let original = sample_srt_file();
  let srt_str = original.to_string_with_format(&Format::Srt);
  let reparsed = subtitler::parse_bytes(srt_str.as_bytes()).unwrap();
  // Compare start/end/text (index is positional, not round-tripped)
  let orig = original.subtitles();
  let rep = reparsed.subtitles();
  assert_eq!(orig.len(), rep.len());
  for (a, b) in orig.iter().zip(rep.iter()) {
    assert_eq!(a.start, b.start);
    assert_eq!(a.end, b.end);
    assert_eq!(a.text, b.text);
  }
}

#[test]
fn vtt_to_srt_preserves_text() {
  let vtt = SubtitleFile::Vtt {
    header: None,
    subtitles: vec![
      Subtitle::new(1500, 3500, "VTT content"),
      Subtitle::new(5000, 7000, "More text"),
    ],
  };
  let srt_str = vtt.to_string_with_format(&Format::Srt);
  assert!(srt_str.contains("VTT content"));
  assert!(srt_str.contains("More text"));
  // SRT uses comma separator
  assert!(srt_str.contains(','));
}

#[test]
fn srt_to_microdvd_round_trip() {
  let original = sample_srt_file();
  let md_str = original.to_string_with_format(&Format::MicroDvd);
  let reparsed = subtitler::parse_bytes(md_str.as_bytes()).unwrap();
  assert_eq!(reparsed.subtitles().len(), 3);
}

#[test]
fn all_formats_produce_valid_output() {
  let original = sample_srt_file();
  for fmt in [
    Format::Srt,
    Format::Vtt,
    Format::Ass,
    Format::MicroDvd,
    Format::SubViewer,
  ] {
    let output = original.to_string_with_format(&fmt);
    assert!(!output.is_empty(), "Format {:?} produced empty output", fmt);
  }
}
