use std::path::PathBuf;
use subtitler::model::Subtitle;
use subtitler::srt;
use subtitler::vtt;

fn fixture_dir() -> PathBuf {
  let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  d.push("examples");
  d
}

#[tokio::test]
async fn test_srt_parse_example_file() {
  let mut path = fixture_dir();
  path.push("example.srt");
  let subtitles = srt::parse_file(&path).await.unwrap();
  assert_eq!(subtitles.len(), 10);
  assert_eq!(subtitles[0].index, Some(1));
  assert_eq!(subtitles[0].start, 1000);
  assert_eq!(subtitles[0].end, 3500);
  assert_eq!(subtitles[0].text, "Hello! How are you today?");
  assert_eq!(subtitles[9].index, Some(10));
  assert!(subtitles.iter().all(|s| s.start > 0 && s.end > s.start));
}

#[tokio::test]
async fn test_vtt_parse_example_file() {
  let mut path = fixture_dir();
  path.push("example.vtt");
  let subtitles = vtt::parse_file(&path).await.unwrap();
  assert_eq!(subtitles.len(), 10);
  assert_eq!(subtitles[0].start, 1000);
  assert_eq!(subtitles[0].end, 3500);
  assert_eq!(subtitles[0].text, "Hi there! How have you been?");
  assert!(subtitles.iter().all(|s| s.end >= s.start));
}

#[tokio::test]
async fn test_srt_round_trip_full() {
  let mut path = fixture_dir();
  path.push("example.srt");
  let original = srt::parse_file(&path).await.unwrap();

  let out_path = fixture_dir().join("_test_round_trip.srt");
  srt::generate(&original, &out_path).await.unwrap();
  let round_tripped = srt::parse_file(&out_path).await.unwrap();
  std::fs::remove_file(&out_path).ok();

  assert_eq!(original.len(), round_tripped.len());
  for (a, b) in original.iter().zip(round_tripped.iter()) {
    assert_eq!(a.start, b.start);
    assert_eq!(a.end, b.end);
    assert_eq!(a.text, b.text);
  }
}

#[tokio::test]
async fn test_vtt_round_trip_full() {
  let mut path = fixture_dir();
  path.push("example.vtt");
  let original = vtt::parse_file(&path).await.unwrap();

  let out_path = fixture_dir().join("_test_round_trip.vtt");
  vtt::generate(&original, &out_path).await.unwrap();
  let round_tripped = vtt::parse_file(&out_path).await.unwrap();
  std::fs::remove_file(&out_path).ok();

  assert_eq!(original.len(), round_tripped.len());
  for (a, b) in original.iter().zip(round_tripped.iter()) {
    assert_eq!(a.start, b.start);
    assert_eq!(a.end, b.end);
    assert_eq!(a.text, b.text);
  }
}

#[tokio::test]
async fn test_srt_parse_content_empty() {
  let content = "";
  let result = srt::parse_content(content).await.unwrap();
  assert!(result.is_empty());
}

#[tokio::test]
async fn test_vtt_parse_content_empty() {
  let content = "WEBVTT\n\n";
  let result = vtt::parse_content(content).await.unwrap();
  assert!(result.is_empty());
}

#[tokio::test]
async fn test_srt_parse_content_only_header_like() {
  let content = "1\n00:00:01,000 --> 00:00:03,000\nx\n\n2\n00:00:04,000 --> 00:00:06,000\ny\n\n";
  let result = srt::parse_content(content).await.unwrap();
  assert_eq!(result.len(), 2);
  assert_eq!(result[0].index, Some(1));
  assert_eq!(result[1].index, Some(2));
}

#[tokio::test]
async fn test_srt_consecutive_blank_lines() {
  let content =
    "1\n00:00:01,000 --> 00:00:03,500\nHello\n\n\n2\n00:00:04,000 --> 00:00:06,500\nWorld\n\n";
  let result = srt::parse_content(content).await.unwrap();
  assert_eq!(result.len(), 2);
}

#[tokio::test]
async fn test_vtt_consecutive_blank_lines() {
  let content = "WEBVTT\n\n\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n\n\n2\n00:00:04.000 --> 00:00:06.500\nWorld\n\n";
  let result = vtt::parse_content(content).await.unwrap();
  assert_eq!(result.len(), 2);
}

#[tokio::test]
async fn test_srt_empty_text() {
  let content = "1\n00:00:01,000 --> 00:00:03,500\n\n\n";
  let result = srt::parse_content(content).await.unwrap();
  assert_eq!(result.len(), 1);
  assert_eq!(result[0].text, "");
}

#[tokio::test]
async fn test_srt_leading_newline() {
  let content = "\n1\n00:00:01,000 --> 00:00:03,500\nHello\n\n";
  let result = srt::parse_content(content).await.unwrap();
  assert_eq!(result.len(), 1);
  assert_eq!(result[0].text, "Hello");
}

#[tokio::test]
async fn test_vtt_leading_newline() {
  let content = "\nWEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n";
  let result = vtt::parse_content(content).await.unwrap();
  assert_eq!(result.len(), 1);
  assert_eq!(result[0].text, "Hello");
}

#[tokio::test]
async fn test_srt_missing_index() {
  let content = "00:00:01,000 --> 00:00:03,500\nNo index here\n\n";
  let result = srt::parse_content(content).await.unwrap();
  assert_eq!(result.len(), 1);
  assert_eq!(result[0].index, None);
}

#[tokio::test]
async fn test_vtt_cue_id_is_string() {
  let content = "WEBVTT\n\nchapter1\n00:00:01.000 --> 00:00:03.500\nHello\n\n";
  let result = vtt::parse_content(content).await.unwrap();
  assert_eq!(result.len(), 1);
  assert_eq!(result[0].index, None);
}

#[tokio::test]
async fn test_srt_generate_empty() {
  let path = fixture_dir().join("_test_empty.srt");
  srt::generate(&[], &path).await.unwrap();
  let content = std::fs::read_to_string(&path).unwrap();
  std::fs::remove_file(&path).ok();
  assert_eq!(content, "");
}

#[tokio::test]
async fn test_vtt_generate_empty() {
  let path = fixture_dir().join("_test_empty.vtt");
  vtt::generate(&[], &path).await.unwrap();
  let content = std::fs::read_to_string(&path).unwrap();
  std::fs::remove_file(&path).ok();
  assert_eq!(content, "WEBVTT\n\n");
}

#[tokio::test]
async fn test_srt_timestamp_error_message() {
  let err = srt::parse_content("1\nnot a time\n").await.unwrap_err();
  let msg = format!("{}", err);
  assert!(msg.contains("Expected timestamp") || msg.contains("Invalid SRT"));
}

#[tokio::test]
async fn test_vtt_timestamp_error_message() {
  let err = vtt::parse_content("WEBVTT\n\n1\nnot a time\n")
    .await
    .unwrap_err();
  let msg = format!("{}", err);
  assert!(msg.contains("Expected timestamp") || msg.contains("Invalid"));
}

#[test]
fn test_subtitle_serde_round_trip() {
  let sub = Subtitle {
    index: Some(1),
    start: 1000,
    end: 3500,
    text: "Hello".to_string(),
    settings: None,
    text_parts: Vec::new(),
  };
  let json = serde_json::to_string(&sub).unwrap();
  let parsed: Subtitle = serde_json::from_str(&json).unwrap();
  assert_eq!(sub, parsed);

  let sub_with_settings = Subtitle {
    index: None,
    start: 0,
    end: 0,
    text: String::new(),
    settings: Some("align:start".to_string()),
    text_parts: Vec::new(),
  };
  let json2 = serde_json::to_string_pretty(&sub_with_settings).unwrap();
  let parsed2: Subtitle = serde_json::from_str(&json2).unwrap();
  assert_eq!(sub_with_settings, parsed2);
}
