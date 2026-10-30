#![cfg(not(target_arch = "wasm32"))]
//! SSA v4.00 round-trip golden tests.
//!
//! Real SSA files use `[V4 Styles]` (18-field style lines) and a
//! `Marked`-first dialogue format with `Text` in the middle — field
//! orders that differ from ASS V4+. Regression for the §14.5 blind spot
//! where well-formed SSA v4.00 files parsed to zero subtitles.

#![cfg(feature = "ass")]
#![allow(irrefutable_let_patterns)]
#![allow(unreachable_patterns)]

use subtitler::SubtitleFormat;
use subtitler::model::SubtitleFile;

const SAMPLE_SSA: &str = "[Script Info]\n\
Title: SSA Sample\n\
ScriptType: v4.00\n\
PlayResX: 384\n\
PlayResY: 288\n\
\n\
[V4 Styles]\n\
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, TertiaryColour, BackColour, Bold, Italic, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, AlphaLevel, Encoding\n\
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,1,0,1,2,1,2,10,10,10,0,1\n\
\n\
[Events]\n\
Format: Marked, Start, End, Text, Name, MarginL, MarginR, MarginV, Effect\n\
Dialogue: Marked=0,0:00:01.00,0:00:03.00,Hello from SSA,,0,0,0,\n\
Dialogue: Marked=0,0:00:04.00,0:00:06.00,Second line, Speaker,0,0,0,\n";

fn parse_to_ass() -> subtitler::model::AssData {
  let file = subtitler::ass::parse_content(SAMPLE_SSA).unwrap();
  match file {
    SubtitleFile::Ass(data) => data,
    other => panic!("expected Ass representation, got {:?}", other.format()),
  }
}

#[test]
fn ssa_v4_parses_events() {
  let data = parse_to_ass();
  assert_eq!(data.subtitles.len(), 2, "SSA events must not be dropped");
  assert_eq!(data.subtitles[0].start, 1000);
  assert_eq!(data.subtitles[0].end, 3000);
  assert_eq!(data.subtitles[0].text, "Hello from SSA");
  assert_eq!(data.subtitles[1].text, "Second line");
  // SSA has no Style column — the fifth field is Name (actor).
  assert_eq!(data.subtitles[1].actor.as_deref(), Some("Speaker"));
  assert_eq!(data.subtitles[0].style, None);
}

#[test]
fn ssa_v4_parses_styles_with_v4_field_map() {
  let data = parse_to_ass();
  assert_eq!(data.styles.len(), 1, "V4 style line must not be dropped");
  let style = &data.styles[0];
  assert_eq!(style.name, "Default");
  assert_eq!(style.fontname, "Arial");
  assert_eq!(style.fontsize, 20);
  assert_eq!(style.outline, 2.0);
  assert_eq!(style.shadow, 1.0);
  assert!(style.bold, "SSA uses 1 for bold, not -1");
  // TertiaryColour maps to the outline colour slot.
  assert_eq!(style.outline_color, "&H00000000");
}

#[test]
fn ssa_round_trips_through_ass_output() {
  let data = parse_to_ass();
  let out = {
    let file = SubtitleFile::Ass(data.clone());
    file.to_string()
  };
  let reparsed = subtitler::ass::parse_content(&out).unwrap();
  match reparsed {
    SubtitleFile::Ass(d) => {
      assert_eq!(d.subtitles.len(), 2);
      assert_eq!(d.subtitles[0].text, "Hello from SSA");
      assert_eq!(d.subtitles[0].start, 1000);
      assert_eq!(d.styles.len(), 1);
      assert_eq!(d.styles[0].name, "Default");
    }
    other => panic!("expected Ass representation, got {:?}", other.format()),
  }
}

#[cfg(feature = "ssa")]
#[test]
fn ssa_detects_as_ssa_and_parses_via_bytes() {
  assert_eq!(
    subtitler::detect_format(SAMPLE_SSA.as_bytes()),
    Some(subtitler::Format::Ssa)
  );
  let file = subtitler::parse_bytes(SAMPLE_SSA.as_bytes()).unwrap();
  assert_eq!(file.format(), subtitler::Format::Ssa);
  assert_eq!(file.subtitles().len(), 2);
}
