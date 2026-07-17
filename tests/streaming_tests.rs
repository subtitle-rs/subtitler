//! Streaming parser tests — verify SrtStream and VttStream yield correct
//! Subtitles one-at-a-time without full Vec allocation.

use subtitler::model::{StreamingParser, SubtitleFormat};
use subtitler::srt;
use subtitler::vtt;

// ── SRT Streaming ──

#[test]
fn srt_stream_basic_parse() {
  let content =
    "1\n00:00:01,000 --> 00:00:03,500\nHello\n\n2\n00:00:04,000 --> 00:00:06,500\nWorld\n\n";
  let results: Vec<_> = srt::parse_stream(content)
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
  assert_eq!(results.len(), 2);
  assert_eq!(results[0].text, "Hello");
  assert_eq!(results[0].start, 1000);
  assert_eq!(results[0].end, 3500);
  assert_eq!(results[1].text, "World");
  assert_eq!(results[1].start, 4000);
  assert_eq!(results[1].end, 6500);
}

#[test]
fn srt_stream_empty_content() {
  let results: Vec<_> = srt::parse_stream("")
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
  assert!(results.is_empty());
}

#[test]
fn srt_stream_gather_same_as_batch() {
  let content =
    "1\n00:00:01,000 --> 00:00:03,500\nHello\n\n2\n00:00:04,000 --> 00:00:06,500\nWorld\n\n";
  let batch = srt::parse_content(content).unwrap();
  let stream: Vec<_> = srt::parse_stream(content)
    .collect::<Result<Vec<_>, _>>()
    .unwrap();

  assert_eq!(batch.subtitles().len(), stream.len());
  for (a, b) in batch.subtitles().iter().zip(stream.iter()) {
    assert_eq!(a.start, b.start);
    assert_eq!(a.end, b.end);
    assert_eq!(a.text, b.text);
    assert_eq!(a.index, b.index);
  }
}

#[test]
fn srt_stream_missing_index() {
  let content = "00:00:01,000 --> 00:00:03,500\nNo index\n\n";
  let results: Vec<_> = srt::parse_stream(content)
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
  assert_eq!(results.len(), 1);
  assert_eq!(results[0].index, None);
  assert_eq!(results[0].text, "No index");
}

#[test]
fn srt_stream_with_bom() {
  let content = "\u{FEFF}1\n00:00:01,000 --> 00:00:03,500\nHello\n\n";
  let results: Vec<_> = srt::parse_stream(content)
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
  assert_eq!(results.len(), 1);
  assert_eq!(results[0].text, "Hello");
}

#[test]
fn srt_stream_error_on_bad_timestamp() {
  let content = "00:00:01,000 --> not_a_time\nBad\n\n";
  let results: Vec<_> = srt::parse_stream(content).collect();
  assert!(results[0].is_err());
}

#[test]
fn srt_stream_consecutive_blank_lines() {
  let content =
    "1\n00:00:01,000 --> 00:00:03,500\nHello\n\n\n\n2\n00:00:04,000 --> 00:00:06,500\nWorld\n\n";
  let results: Vec<_> = srt::parse_stream(content)
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
  assert_eq!(results.len(), 2);
}

// ── VTT Streaming ──

#[test]
fn vtt_stream_basic_parse() {
  let content = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n2\n00:00:04.000 --> 00:00:06.500\nWorld\n\n";
  let results: Vec<_> = vtt::parse_stream(content)
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
  assert_eq!(results.len(), 2);
  assert_eq!(results[0].text, "Hello");
  assert_eq!(results[0].start, 1000);
  assert_eq!(results[1].text, "World");
}

#[test]
fn vtt_stream_empty() {
  let results: Vec<_> = vtt::parse_stream("WEBVTT\n\n")
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
  assert!(results.is_empty());
}

#[test]
fn vtt_stream_gather_same_as_batch() {
  let content = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n2\n00:00:04.000 --> 00:00:06.500\nWorld\n\n";
  let batch = vtt::parse_content(content).unwrap();
  let stream: Vec<_> = vtt::parse_stream(content)
    .collect::<Result<Vec<_>, _>>()
    .unwrap();

  assert_eq!(batch.subtitles().len(), stream.len());
  for (a, b) in batch.subtitles().iter().zip(stream.iter()) {
    assert_eq!(a.start, b.start);
    assert_eq!(a.end, b.end);
    assert_eq!(a.text, b.text);
  }
}

#[test]
fn vtt_stream_header_access() {
  let content =
    "WEBVTT\nKind: captions\nLanguage: en\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n";
  let mut stream = vtt::parse_stream(content);
  // consume first to trigger header collection
  let _first = stream.next();
  let header = stream.header();
  assert!(header.is_some());
  assert!(header.unwrap().contains("Kind: captions"));
}

#[test]
fn vtt_stream_with_note_skipped() {
  let content = "WEBVTT\n\nNOTE This is a comment\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n";
  let results: Vec<_> = vtt::parse_stream(content)
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
  assert_eq!(results.len(), 1);
  assert_eq!(results[0].text, "Hello");
}

#[test]
fn vtt_stream_with_bom() {
  let content = "\u{FEFF}WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n";
  let results: Vec<_> = vtt::parse_stream(content)
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
  assert_eq!(results.len(), 1);
}

#[test]
fn vtt_stream_error_on_bad_timestamp() {
  let content = "WEBVTT\n\n1\n00:00:01.000 --> not_a_time\nBad\n\n";
  let results: Vec<_> = vtt::parse_stream(content).collect();
  assert!(results[0].is_err());
}

// ── StreamingParser trait ──

#[test]
fn streaming_parser_collect_all() {
  let content =
    "1\n00:00:01,000 --> 00:00:03,500\nHello\n\n2\n00:00:04,000 --> 00:00:06,500\nWorld\n\n";
  let mut parser = srt::parse_stream(content);
  let results = parser.collect_all().unwrap();
  assert_eq!(results.len(), 2);
}

#[test]
fn streaming_parser_count_remaining() {
  let content = "1\n00:00:01,000 --> 00:00:03,500\nA\n\n2\n00:00:04,000 --> 00:00:06,500\nB\n\n3\n00:00:07,000 --> 00:00:09,500\nC\n\n";
  let mut parser = srt::parse_stream(content);
  let _first = parser.next().unwrap().unwrap();
  let remaining = parser.count_remaining();
  assert_eq!(remaining, 2);
}
