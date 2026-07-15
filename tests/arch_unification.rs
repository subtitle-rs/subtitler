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
  let (header, subs) = subtitler::subviewer::parse_content(content).unwrap();
  assert!(
    header.as_deref().unwrap().contains("My Film"),
    "header lost: {header:?}"
  );
  assert_eq!(subs.len(), 1);
  assert_eq!(subs[0].text, "Hello");
}
