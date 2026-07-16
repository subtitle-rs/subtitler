use subtitler::model::{AssData, Format, Subtitle, SubtitleFile, SubtitleFormat};

fn sub(start: u64, end: u64, text: &str) -> Subtitle {
  Subtitle::new(start, end, text)
}

fn sample_of_each_variant() -> Vec<SubtitleFile> {
  vec![
    SubtitleFile::Srt(vec![sub(0, 2000, "a"), sub(3000, 5000, "b")]),
    SubtitleFile::Vtt {
      header: None,
      subtitles: vec![sub(0, 2000, "a"), sub(3000, 5000, "b")],
    },
    SubtitleFile::Ass(AssData {
      info: Default::default(),
      styles: vec![],
      subtitles: vec![sub(0, 2000, "a"), sub(3000, 5000, "b")],
    }),
    SubtitleFile::Ssa(AssData {
      info: Default::default(),
      styles: vec![],
      subtitles: vec![sub(0, 2000, "a"), sub(3000, 5000, "b")],
    }),
    SubtitleFile::MicroDvd {
      fps: 25.0,
      subtitles: vec![sub(0, 2000, "a"), sub(3000, 5000, "b")],
    },
    SubtitleFile::SubViewer {
      header: None,
      subtitles: vec![sub(0, 2000, "a"), sub(3000, 5000, "b")],
    },
  ]
}

#[test]
fn shift_all_works_for_every_variant() {
  for (i, mut file) in sample_of_each_variant().into_iter().enumerate() {
    file.shift_all(1000);
    let first_start = file.subtitles()[0].start;
    assert_eq!(first_start, 1000, "variant {i} shift_all failed");
  }
}

#[test]
fn format_reports_correctly_for_every_variant() {
  let expected = [
    Format::Srt,
    Format::Vtt,
    Format::Ass,
    Format::Ssa,
    Format::MicroDvd,
    Format::SubViewer,
  ];
  for (i, file) in sample_of_each_variant().into_iter().enumerate() {
    assert_eq!(file.format(), expected[i], "variant {i} format wrong");
  }
}

#[test]
fn validate_clean_for_every_variant() {
  for (i, file) in sample_of_each_variant().into_iter().enumerate() {
    let issues = file.validate();
    assert!(issues.is_empty(), "variant {i} reported {issues:?}");
  }
}

#[test]
fn microdvd_roundtrips_fps() {
  // {1}{1}30.000 declares fps=30; frames {30}{60} at 30fps = 1000-2000ms.
  // After round-trip the fps must be preserved (not fall back to 23.976).
  let content = "{1}{1}30.000\n{30}{60}Hello\n";
  let file = subtitler::microdvd::parse_content(content, None).unwrap();
  let out = file.to_string();
  assert!(
    out.contains("30.000"),
    "fps header lost in round-trip; got:\n{out}"
  );
}

#[test]
fn microdvd_variant_preserves_fps_field() {
  let content = "{1}{1}30.000\n{30}{60}Hello\n";
  let file = subtitler::microdvd::parse_content(content, None).unwrap();
  match file {
    SubtitleFile::MicroDvd { fps, .. } => assert!((fps - 30.0).abs() < 0.001, "fps={fps}"),
    other => panic!("expected MicroDvd variant, got {other:?}"),
  }
}

#[test]
fn subviewer_variant_preserves_header() {
  let content = "[INFORMATION]\n[TITLE]My Film\n[AUTHOR]Me\n[END INFORMATION]\n[SUBTITLE]\n[COLF]&HFFFFFF,[STYLE]bd,[SIZE]18,[FONT]Arial\n\n00:00:01.00,00:00:03.50\nHello\n";
  let file = subtitler::subviewer::parse_content(content).unwrap();
  match file {
    SubtitleFile::SubViewer { header, subtitles } => {
      assert!(
        header.as_deref().unwrap().contains("My Film"),
        "header lost: {header:?}"
      );
      assert_eq!(subtitles.len(), 1);
      assert_eq!(subtitles[0].text, "Hello");
    }
    _ => panic!("expected SubViewer variant"),
  }
}

#[test]
fn unified_parse_bytes_detects_srt() {
  let data = b"1\n00:00:01,000 --> 00:00:03,500\nHello\n\n";
  let file = subtitler::parse_bytes(data).unwrap();
  assert!(matches!(file, SubtitleFile::Srt(_)));
}

#[test]
fn unified_parse_bytes_detects_vtt() {
  let data = b"WEBVTT\n\n00:00:01.000 --> 00:00:03.500\nHello\n\n";
  let file = subtitler::parse_bytes(data).unwrap();
  assert!(matches!(file, SubtitleFile::Vtt { .. }));
}

#[test]
fn unified_parse_bytes_unknown_format_errors() {
  let result = subtitler::parse_bytes(b"not a subtitle at all\nnope\n");
  assert!(matches!(
    result,
    Err(subtitler::error::ParseError::UnknownFormat)
  ));
}

#[test]
fn sbv_multiline_text() {
  let content = "0:00:01.000,0:00:03.500,Line one|Line two\n0:00:04.000,0:00:06.500,Single\n";
  let subs = subtitler::sbv::parse_content(content).unwrap();
  assert_eq!(subs.len(), 2);
  // SBV uses | for line breaks internally; the parser replaces | with \n
  assert_eq!(subs[0].text, "Line one|Line two");
}

#[test]
fn lrc_multi_timestamp_round_trip() {
  let content = "[00:10.00][00:30.00]Repeated line\n";
  let data = subtitler::lrc::LrcData::parse(content).unwrap();
  assert_eq!(data.lines.len(), 1, "one LRC line");
  assert_eq!(data.lines[0].times_ms.len(), 2, "two timestamps");
  assert_eq!(data.lines[0].text, "Repeated line");

  // Convert to subtitles for compatibility
  let subs = data.to_subtitles();
  assert_eq!(subs.len(), 2, "two timestamps should produce two cues");
  assert_eq!(subs[0].text, "Repeated line");
  assert_eq!(subs[0].start, 10000);
  assert_eq!(subs[0].end, 15000);
  assert_eq!(subs[1].start, 30000);
  assert_eq!(subs[1].end, 35000);

  // Round-trip: serialize back, each cue becomes its own LRC line
  let output = data.to_string();
  assert!(output.contains("[00:10.00]"));
  assert!(output.contains("[00:30.00]"));
}

#[test]
fn microdvd_fps_round_trip_precision() {
  // {1}{1}30.000 declares fps=30; {30}{60} at 30fps = 1000-2000ms.
  // Re-encode and verify frame numbers survive.
  let content = "{1}{1}30.000\n{30}{60}Hello\n";
  let file = subtitler::microdvd::parse_content(content, None).unwrap();
  let output = file.to_string_with_format(&Format::MicroDvd);
  assert!(output.contains("30.000"), "fps header lost");
  assert!(
    output.contains("{30}{60}"),
    "frame numbers changed: {}",
    output
  );
}
