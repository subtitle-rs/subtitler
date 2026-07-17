use smallvec::SmallVec;
use std::path::PathBuf;
use subtitler::ass;
use subtitler::model::{
  Subtitle, SubtitleFile, SubtitleFormat, ValidationIssue, frames_to_ms, ms_to_frames,
};
use subtitler::srt;
use subtitler::vtt;

fn fixture_dir() -> PathBuf {
  let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  d.push("examples");
  d
}

// --- SRT Tests ---

#[tokio::test]
async fn test_srt_parse_example_file() {
  let mut path = fixture_dir();
  path.push("example.srt");
  let subtitles = srt::parse_file(&path).await.unwrap();
  assert_eq!(subtitles.subtitles().len(), 10);
  assert_eq!(subtitles.subtitles()[0].index, Some(1));
  assert_eq!(subtitles.subtitles()[0].start, 1000);
  assert_eq!(subtitles.subtitles()[0].end, 3500);
  assert_eq!(subtitles.subtitles()[0].text, "Hello! How are you today?");
  assert_eq!(subtitles.subtitles()[9].index, Some(10));
  assert!(
    subtitles
      .subtitles()
      .iter()
      .all(|s| s.start > 0 && s.end > s.start)
  );
}

#[tokio::test]
async fn test_srt_round_trip_full() {
  let mut path = fixture_dir();
  path.push("example.srt");
  let original = srt::parse_file(&path).await.unwrap();

  let out_path = fixture_dir().join("_test_round_trip.srt");
  srt::generate(original.subtitles(), &out_path, None)
    .await
    .unwrap();
  let round_tripped = srt::parse_file(&out_path).await.unwrap();
  std::fs::remove_file(&out_path).ok();

  assert_eq!(original.subtitles().len(), round_tripped.subtitles().len());
  for (a, b) in original
    .subtitles()
    .iter()
    .zip(round_tripped.subtitles().iter())
  {
    assert_eq!(a.start, b.start);
    assert_eq!(a.end, b.end);
    assert_eq!(a.text, b.text);
  }
}

#[tokio::test]
async fn test_srt_parse_content_empty() {
  let result = srt::parse_content("").unwrap();
  assert!(result.subtitles().is_empty());
}

#[tokio::test]
async fn test_srt_parse_content_only_header_like() {
  let content = "1\n00:00:01,000 --> 00:00:03,000\nx\n\n2\n00:00:04,000 --> 00:00:06,000\ny\n\n";
  let result = srt::parse_content(content).unwrap();
  assert_eq!(result.subtitles().len(), 2);
}

#[tokio::test]
async fn test_srt_consecutive_blank_lines() {
  let content =
    "1\n00:00:01,000 --> 00:00:03,500\nHello\n\n\n2\n00:00:04,000 --> 00:00:06,500\nWorld\n\n";
  let result = srt::parse_content(content).unwrap();
  assert_eq!(result.subtitles().len(), 2);
}

#[tokio::test]
async fn test_srt_empty_text() {
  let content = "1\n00:00:01,000 --> 00:00:03,500\n\n\n";
  let result = srt::parse_content(content).unwrap();
  assert_eq!(result.subtitles().len(), 1);
  assert_eq!(result.subtitles()[0].text, "");
}

#[tokio::test]
async fn test_srt_leading_newline() {
  let content = "\n1\n00:00:01,000 --> 00:00:03,500\nHello\n\n";
  let result = srt::parse_content(content).unwrap();
  assert_eq!(result.subtitles()[0].text, "Hello");
}

#[tokio::test]
async fn test_srt_missing_index() {
  let content = "00:00:01,000 --> 00:00:03,500\nNo index here\n\n";
  let result = srt::parse_content(content).unwrap();
  assert_eq!(result.subtitles()[0].index, None);
}

#[tokio::test]
async fn test_srt_generate_empty() {
  let path = fixture_dir().join("_test_empty.srt");
  srt::generate(&[], &path, None).await.unwrap();
  let content = std::fs::read_to_string(&path).unwrap();
  std::fs::remove_file(&path).ok();
  assert_eq!(content, "");
}

#[tokio::test]
async fn test_srt_timestamp_error_message() {
  let err = srt::parse_content("1\nnot a time\n").unwrap_err();
  let msg = format!("{}", err);
  assert!(msg.contains("expected timestamp") || msg.contains("Invalid SRT"));
}

// --- VTT Tests ---

#[tokio::test]
async fn test_vtt_parse_example_file() {
  let mut path = fixture_dir();
  path.push("example.vtt");
  let subtitles = vtt::parse_file(&path).await.unwrap();
  assert_eq!(subtitles.subtitles().len(), 10);
  assert_eq!(subtitles.subtitles()[0].start, 1000);
  assert_eq!(subtitles.subtitles()[0].end, 3500);
  assert_eq!(
    subtitles.subtitles()[0].text,
    "Hi there! How have you been?"
  );
  assert!(subtitles.subtitles().iter().all(|s| s.end >= s.start));
}

#[tokio::test]
async fn test_vtt_round_trip_full() {
  let mut path = fixture_dir();
  path.push("example.vtt");
  let original = vtt::parse_file(&path).await.unwrap();

  let out_path = fixture_dir().join("_test_round_trip.vtt");
  vtt::generate(original.subtitles(), &out_path, None)
    .await
    .unwrap();
  let round_tripped = vtt::parse_file(&out_path).await.unwrap();
  std::fs::remove_file(&out_path).ok();

  assert_eq!(original.subtitles().len(), round_tripped.subtitles().len());
  for (a, b) in original
    .subtitles()
    .iter()
    .zip(round_tripped.subtitles().iter())
  {
    assert_eq!(a.start, b.start);
    assert_eq!(a.end, b.end);
    assert_eq!(a.text, b.text);
  }
}

#[tokio::test]
async fn test_vtt_parse_content_empty() {
  let result = vtt::parse_content("WEBVTT\n\n").unwrap();
  assert!(result.subtitles().is_empty());
}

#[tokio::test]
async fn test_vtt_consecutive_blank_lines() {
  let content = "WEBVTT\n\n\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n\n\n2\n00:00:04.000 --> 00:00:06.500\nWorld\n\n";
  let result = vtt::parse_content(content).unwrap();
  assert_eq!(result.subtitles().len(), 2);
}

#[tokio::test]
async fn test_vtt_leading_newline() {
  let content = "\nWEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n";
  let result = vtt::parse_content(content).unwrap();
  assert_eq!(result.subtitles()[0].text, "Hello");
}

#[tokio::test]
async fn test_vtt_cue_id_is_string() {
  let content = "WEBVTT\n\nchapter1\n00:00:01.000 --> 00:00:03.500\nHello\n\n";
  let result = vtt::parse_content(content).unwrap();
  assert_eq!(result.subtitles()[0].index, None);
}

#[tokio::test]
async fn test_vtt_generate_empty() {
  let path = fixture_dir().join("_test_empty.vtt");
  vtt::generate(&[], &path, None).await.unwrap();
  let content = std::fs::read_to_string(&path).unwrap();
  std::fs::remove_file(&path).ok();
  assert_eq!(content, "WEBVTT\n\n");
}

#[tokio::test]
async fn test_vtt_timestamp_error_message() {
  let err = vtt::parse_content("WEBVTT\n\n1\nnot a time\n").unwrap_err();
  let msg = format!("{}", err);
  assert!(msg.contains("expected timestamp") || msg.contains("Invalid"));
}

#[tokio::test]
async fn test_vtt_parse_with_bom() {
  let content = "\u{FEFF}WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n";
  let result = vtt::parse_content(content).unwrap();
  assert_eq!(result.subtitles().len(), 1);
  assert_eq!(result.subtitles()[0].text, "Hello");
}

#[tokio::test]
async fn test_vtt_parse_with_note() {
  let content = "WEBVTT\n\nNOTE This is a comment\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n";
  let result = vtt::parse_content(content).unwrap();
  assert_eq!(result.subtitles().len(), 1);
  assert_eq!(result.subtitles()[0].text, "Hello");
}

#[tokio::test]
async fn test_vtt_parse_header_preserved() {
  let content =
    "WEBVTT\nKind: captions\nLanguage: en\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n";
  let (header, subtitles) = vtt::parse_content_full(content).unwrap();
  assert_eq!(subtitles.len(), 1);
  assert!(header.is_some());
  assert!(header.unwrap().contains("Kind: captions"));
}

// --- ASS Tests ---

#[test]
fn test_ass_parse_content() {
  let content = "[Script Info]\nTitle: Test\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,Hello!\nDialogue: 0,0:00:04.00,0:00:06.50,Default,,0,0,0,,World!\n";
  let file = ass::parse_content(content).unwrap();
  let subs = file.subtitles();
  assert_eq!(subs.len(), 2);
  assert_eq!(subs[0].start, 1000);
  assert_eq!(subs[0].end, 3500);
  assert_eq!(subs[0].text, "Hello!");
  assert_eq!(subs[0].style.as_deref(), Some("Default"));
}

#[test]
fn test_ass_parse_empty_events() {
  let content = "[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: ...\nStyle: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n";
  let file = ass::parse_content(content).unwrap();
  assert!(file.subtitles().is_empty());
}

#[test]
fn test_ass_to_string_round_trip() {
  let content = "[Script Info]\nTitle: RT\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,Hello\nDialogue: 0,0:00:04.00,0:00:06.50,Default,,0,0,0,,World\n";
  let parsed = ass::parse_content(content).unwrap();
  let regenerated = parsed.to_string();
  let reparsed = ass::parse_content(&regenerated).unwrap();
  assert_eq!(reparsed.subtitles().len(), 2);
  assert_eq!(reparsed.subtitles()[0].start, 1000);
  assert_eq!(reparsed.subtitles()[0].text, "Hello");
  assert_eq!(reparsed.subtitles()[1].start, 4000);
  assert_eq!(reparsed.subtitles()[1].text, "World");
}

#[test]
fn test_ass_detect_format() {
  let data = b"[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nStyle: Default,...\n";
  assert_eq!(
    ass::detect_format(data),
    Some(subtitler::model::Format::Ass)
  );
}

#[test]
fn test_ass_detect_format_negative() {
  let data = b"WEBVTT\n\n";
  assert_eq!(ass::detect_format(data), None);
}

#[test]
fn test_ass_ssa_format() {
  let content = "[Script Info]\nScriptType: v4.00\n\n[V4 Styles]\nFormat: ...\nStyle: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,Hello\n";
  let file = ass::parse_content(content).unwrap();
  assert_eq!(file.subtitles().len(), 1);
}

// --- SRT Stringify Tests ---

#[tokio::test]
async fn test_srt_to_string() {
  let mut sub1 = Subtitle::new(1000, 3500, "Hello");
  sub1.index = Some(1);
  let mut sub2 = Subtitle::new(4000, 6500, "World");
  sub2.index = Some(2);
  let subtitles = vec![sub1, sub2];
  let output = srt::to_string(&subtitles);
  assert!(output.contains("00:00:01,000 --> 00:00:03,500"));
  assert!(output.contains("00:00:04,000 --> 00:00:06,500"));
  assert!(output.contains("Hello\n\n2\n"));
  assert!(output.contains("World\n"));
}

// --- VTT Stringify Tests ---

#[tokio::test]
async fn test_vtt_to_string() {
  let subtitles = vec![
    Subtitle::new(1000, 3500, "Hello"),
    Subtitle::new(4000, 6500, "World"),
  ];
  let output = vtt::to_string(&subtitles, None);
  assert!(output.starts_with("WEBVTT\n\n"));
  assert!(output.contains("00:00:01.000 --> 00:00:03.500"));
  assert!(output.contains("00:00:04.000 --> 00:00:06.500"));
}

// --- Utility Method Tests ---

#[test]
fn test_subtitle_file_sort() {
  let mut file = SubtitleFile::Srt(vec![
    Subtitle::new(5000, 7000, "c"),
    Subtitle::new(1000, 3000, "a"),
    Subtitle::new(3000, 5000, "b"),
  ]);
  file.sort();
  assert_eq!(file.subtitles()[0].start, 1000);
  assert_eq!(file.subtitles()[1].start, 3000);
  assert_eq!(file.subtitles()[2].start, 5000);
}

#[test]
fn test_subtitle_file_validate_clean() {
  let file = SubtitleFile::Srt(vec![
    Subtitle::new(1000, 3000, "a"),
    Subtitle::new(4000, 6000, "b"),
  ]);
  assert!(file.validate().is_empty());
}

#[test]
fn test_subtitle_file_validate_overlap() {
  let file = SubtitleFile::Srt(vec![
    Subtitle::new(1000, 3000, "a"),
    Subtitle::new(2000, 4000, "b"),
  ]);
  let issues = file.validate();
  assert_eq!(issues.len(), 1);
  assert!(matches!(issues[0], ValidationIssue::Overlap { .. }));
}

#[test]
fn test_subtitle_file_validate_negative_duration() {
  let file = SubtitleFile::Srt(vec![Subtitle::new(3000, 1000, "bad")]);
  let issues = file.validate();
  assert_eq!(issues.len(), 1);
  assert!(matches!(
    issues[0],
    ValidationIssue::NegativeDuration { .. }
  ));
}

#[test]
fn test_subtitle_file_validate_zero_duration() {
  let file = SubtitleFile::Srt(vec![Subtitle::new(1000, 1000, "bad")]);
  let issues = file.validate();
  assert_eq!(issues.len(), 1);
  assert!(matches!(issues[0], ValidationIssue::ZeroDuration { .. }));
}

#[test]
fn test_subtitle_file_merge_adjacent() {
  let mut file = SubtitleFile::Srt(vec![
    Subtitle::new(1000, 3000, "first"),
    Subtitle::new(3100, 5000, "second"),
    Subtitle::new(7000, 9000, "third"),
  ]);
  file.merge_adjacent(500);
  let subs = file.subtitles();
  assert_eq!(subs.len(), 2);
  assert_eq!(subs[0].text, "first\nsecond");
  assert_eq!(subs[0].end, 5000);
}

#[test]
fn test_subtitle_file_merge_noop() {
  let mut file = SubtitleFile::Srt(vec![
    Subtitle::new(1000, 3000, "a"),
    Subtitle::new(5000, 7000, "b"),
  ]);
  file.merge_adjacent(100);
  assert_eq!(file.subtitles().len(), 2);
}

#[test]
fn test_subtitle_file_split_long() {
  let mut file = SubtitleFile::Srt(vec![Subtitle::new(
    1000,
    5000,
    "this is a very long subtitle that must be split into smaller pieces",
  )]);
  file.split_long(20);
  assert!(file.subtitles().len() >= 2);
  for sub in file.subtitles() {
    assert!(sub.text.chars().count() <= 20 || sub.text.len() <= 20);
  }
}

#[test]
fn test_subtitle_file_split_long_noop() {
  let mut file = SubtitleFile::Srt(vec![Subtitle::new(1000, 3000, "short")]);
  file.split_long(20);
  assert_eq!(file.subtitles().len(), 1);
}

#[test]
fn test_subtitle_file_transform_framerate() {
  let mut file = SubtitleFile::Srt(vec![Subtitle::new(1000, 3000, "test")]);
  file.transform_framerate(23.976, 25.0);
  let sub = &file.subtitles()[0];
  assert!(sub.start >= 1040 && sub.start <= 1045);
}

#[test]
fn test_subtitle_file_shift_all() {
  let mut file = SubtitleFile::Srt(vec![Subtitle::new(1000, 3000, "test")]);
  file.shift_all(500);
  assert_eq!(file.subtitles()[0].start, 1500);
  assert_eq!(file.subtitles()[0].end, 3500);
}

#[test]
fn test_subtitle_file_shift_all_negative_clamp() {
  let mut file = SubtitleFile::Srt(vec![Subtitle::new(1000, 3000, "test")]);
  file.shift_all(-2000);
  assert_eq!(file.subtitles()[0].start, 0);
  assert_eq!(file.subtitles()[0].end, 1000);
}

#[test]
fn test_subtitle_file_map() {
  let file = SubtitleFile::Srt(vec![
    Subtitle::new(1000, 3000, "hello"),
    Subtitle::new(4000, 6000, "world"),
  ]);
  let file = file.map(|sub| {
    sub.text = sub.text.to_uppercase();
  });
  assert_eq!(file.subtitles()[0].text, "HELLO");
  assert_eq!(file.subtitles()[1].text, "WORLD");
}

#[test]
fn test_subtitle_file_filter() {
  let file = SubtitleFile::Srt(vec![
    Subtitle::new(1000, 3000, "keep"),
    Subtitle::new(4000, 6000, "drop"),
  ]);
  let file = file.filter(|sub| sub.text == "keep");
  assert_eq!(file.subtitles().len(), 1);
  assert_eq!(file.subtitles()[0].text, "keep");
}

#[test]
fn test_subtitle_file_to_string() {
  let file = SubtitleFile::Srt(vec![Subtitle::new(1000, 3500, "Hello")]);
  let s = file.to_string();
  assert!(s.contains("00:00:01,000 --> 00:00:03,500"));
  assert!(s.contains("Hello"));
}

#[test]
fn test_frame_conversion_round_trip() {
  for ms in [0, 1000, 3600000] {
    for fps in [23.976, 24.0, 25.0, 29.97, 30.0] {
      let frames = ms_to_frames(ms, fps);
      let back = frames_to_ms(frames, fps);
      let diff = (back as i64 - ms as i64).abs();
      let tolerance = (1000.0 / fps).ceil() as i64;
      assert!(
        diff <= tolerance,
        "round-trip failed: {}ms -> {}f @ {}fps -> {}ms (diff={}, tolerance={})",
        ms,
        frames,
        fps,
        back,
        diff,
        tolerance
      );
    }
  }
}

#[test]
fn test_subtitle_chars_per_second() {
  let sub = Subtitle::new(0, 2000, "Hello World");
  let cps = sub.chars_per_second();
  assert!((cps - 5.5).abs() < 0.01);
}

#[test]
fn test_subtitle_duration_ms() {
  let sub = Subtitle::new(1000, 5000, "test");
  assert_eq!(sub.duration_ms(), 4000);
}

// --- Serde Tests ---

#[test]
fn test_subtitle_serde_round_trip() {
  let sub = Subtitle {
    index: Some(1),
    start: 1000,
    end: 3500,
    text: "Hello".to_string(),
    settings: None,
    text_parts: SmallVec::new(),
    style: None,
    actor: None,
    is_comment: false,
  };
  let json = serde_json::to_string(&sub).unwrap();
  let parsed: Subtitle = serde_json::from_str(&json).unwrap();
  assert_eq!(sub, parsed);

  let sub_with_style = Subtitle {
    style: Some("Custom".into()),
    actor: Some("Alice".into()),
    ..Subtitle::new(1000, 2000, "styled")
  };
  let json2 = serde_json::to_string_pretty(&sub_with_style).unwrap();
  let parsed2: Subtitle = serde_json::from_str(&json2).unwrap();
  assert_eq!(sub_with_style, parsed2);
}

// --- Format Detection Tests ---

#[test]
fn test_detect_format_srt() {
  assert_eq!(
    subtitler::detect_format(b"1\n00:00:01,000 --> 00:00:03,500\nHello\n\n"),
    Some(subtitler::model::Format::Srt)
  );
}

#[test]
fn test_detect_format_vtt() {
  assert_eq!(
    subtitler::detect_format(b"WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n"),
    Some(subtitler::model::Format::Vtt)
  );
}

#[test]
fn test_detect_format_ass() {
  assert_eq!(
    subtitler::detect_format(b"[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nStyle: ...\n"),
    Some(subtitler::model::Format::Ass)
  );
}

#[test]
fn test_detect_format_unknown() {
  assert_eq!(subtitler::detect_format(b"not a subtitle"), None);
  assert_eq!(subtitler::detect_format(b""), None);
}

// ── Duration enforcement tests ──

#[test]
fn test_enforce_min_duration() {
  let mut file = SubtitleFile::Srt(vec![
    Subtitle::new(1000, 1200, "short"), // 200ms → should extend to 1000ms
    Subtitle::new(2000, 5000, "ok"),
  ]);
  file.enforce_min_duration(1000);
  assert_eq!(file.subtitles()[0].end, 2000);
  assert_eq!(file.subtitles()[1].end, 5000);
}

#[test]
fn test_enforce_max_duration() {
  let mut file = SubtitleFile::Srt(vec![
    Subtitle::new(1000, 8000, "very long"), // 7000ms → trim to 3000ms
    Subtitle::new(9000, 10000, "short"),
  ]);
  file.enforce_max_duration(3000);
  assert_eq!(file.subtitles()[0].end, 4000);
  assert_eq!(file.subtitles()[1].end, 10000);
}

#[test]
fn test_auto_extend_for_cps() {
  let mut file = SubtitleFile::Srt(vec![Subtitle::new(
    0,
    500,
    "This is a very long subtitle text that needs more time",
  )]);
  file.auto_extend_for_cps(20.0);
  // 54 chars / 20 cps = 2.7s = 2700ms
  assert!(file.subtitles()[0].end >= 2700);
}

// ── Batch operation tests ──

#[test]
fn test_extract_range() {
  let file = SubtitleFile::Srt(vec![
    Subtitle::new(1000, 3000, "first"),
    Subtitle::new(4000, 6000, "second"),
    Subtitle::new(7000, 9000, "third"),
  ]);
  let extracted = file.extract_range(2000, 7000);
  assert_eq!(extracted.len(), 2);
  assert_eq!(extracted[0].text, "first");
  assert_eq!(extracted[0].start, 2000); // clamped
  assert_eq!(extracted[1].text, "second");
}

#[test]
fn test_concatenate() {
  let mut file1 = SubtitleFile::Srt(vec![
    Subtitle::new(1000, 3000, "A"),
    Subtitle::new(4000, 6000, "B"),
  ]);
  let file2 = SubtitleFile::Srt(vec![Subtitle::new(1000, 3000, "C")]);
  file1.concatenate(&file2, 1000);
  assert_eq!(file1.subtitles().len(), 3);
  assert_eq!(file1.subtitles()[2].text, "C");
  assert_eq!(file1.subtitles()[2].start, 8000);
}

// ── Color conversion tests ──

#[test]
fn test_parse_ass_color() {
  let (r, g, b, a) = subtitler::model::parse_ass_color("&H00FFFFFF");
  assert_eq!(r, 255);
  assert_eq!(g, 255);
  assert_eq!(b, 255);
  assert_eq!(a, 0);
}

#[test]
fn test_format_ass_color() {
  let color = subtitler::model::format_ass_color(255, 128, 0, 0);
  assert_eq!(color, "&H000080FF");
}

#[test]
fn test_color_round_trip() {
  for (r, g, b, a) in [(255u8, 255, 255, 0), (0, 0, 0, 255), (128, 64, 32, 128)] {
    let formatted = subtitler::model::format_ass_color(r, g, b, a);
    let (r2, g2, b2, a2) = subtitler::model::parse_ass_color(&formatted);
    assert_eq!((r, g, b, a), (r2, g2, b2, a2));
  }
}

// ── ASS tag parsing tests ──

#[test]
fn test_ass_to_plaintext() {
  assert_eq!(
    subtitler::ass::ass_to_plaintext("{\\i1}Hello{\\i0} World"),
    "Hello World"
  );
  assert_eq!(
    subtitler::ass::ass_to_plaintext("Line 1\\NLine 2"),
    "Line 1\nLine 2"
  );
}

#[test]
fn test_parse_ass_tags_bold() {
  let parts = subtitler::ass::parse_ass_tags("{\\b1}Bold text{\\b0}");
  assert_eq!(parts.len(), 1);
  assert!(parts[0].bold());
  assert_eq!(parts[0].text, "Bold text");
}

#[test]
fn test_parse_ass_tags_italic() {
  let parts = subtitler::ass::parse_ass_tags("{\\i1}Italic{\\i0}");
  assert_eq!(parts.len(), 1);
  assert!(parts[0].italic());
}

// ── Normalize tests ──

#[test]
fn test_normalize_whitespace_integration() {
  assert_eq!(
    subtitler::normalize::normalize_whitespace("hello   world"),
    "hello world"
  );
}

#[test]
fn test_strip_hearing_impaired_integration() {
  assert_eq!(
    subtitler::normalize::strip_hearing_impaired("Hello (LAUGHS)"),
    "Hello"
  );
}

// ── Subtitle.plaintext test ──

#[test]
fn test_subtitle_plaintext() {
  let sub = Subtitle::new(0, 1000, "<b>Hello</b> {\\i1}World{\\i0}");
  assert_eq!(sub.plaintext(), "Hello World");
}
