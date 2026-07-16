//! Parse TTML and LRC formats — streaming and lyrics.
//!
//! Run: cargo run --example parse-ttml-lrc

use subtitler::SubtitleFormat;

fn main() -> subtitler::types::AnyResult<()> {
  // === TTML (Netflix / streaming) ===
  let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling" xml:lang="en">
  <body><div>
    <p begin="00:00:01.000" end="00:00:03.500">Hello from TTML</p>
    <p begin="00:00:04.000" end="00:00:06.500"><span tts:color="yellow">Colored</span> text</p>
    <p begin="00:00:07.000" dur="2.5s">Using duration</p>
    <p begin="00:00:10.000" end="00:00:12.000">Line one<br/>Line two</p>
  </div></body>
</tt>"#;

  println!("=== TTML ===");
  let subs = subtitler::ttml::parse_content(ttml)?;
  for (i, s) in subs.iter().enumerate() {
    println!("  [{}] {}-{}ms '{}'", i, s.start, s.end, s.text);
    if !s.text_parts.is_empty() {
      for p in &s.text_parts {
        println!("       part: '{}' color={:?}", p.text, p.color);
      }
    }
  }

  // === LRC (lyrics) ===
  let lrc = "[00:01.50]Imagine there's no heaven\n\
             [00:03.20]It's easy if you try\n\
             [00:05.00]No hell below us\n";

  println!("\n=== LRC ===");
  let lrc_data = subtitler::lrc::LrcData::parse(lrc)?; let subs = lrc_data.to_subtitles();
  for (i, s) in subs.iter().enumerate() {
    println!("  [{}] {}-{}ms '{}'", i, s.start, s.end, s.text);
  }

  // Convert LRC → SRT
  let srt_file = subtitler::model::SubtitleFile::Srt(subs);
  println!("\n=== LRC → SRT conversion ===");
  println!("{}", srt_file.to_string());

  // === SBV (YouTube) ===
  let sbv = "0:00:01.000,0:00:03.500,Hello from SBV\n\
             0:00:04.000,0:00:06.500,Second line\n";

  println!("=== SBV ===");
  let subs = subtitler::sbv::parse_content(sbv)?;
  for (i, s) in subs.iter().enumerate() {
    println!("  [{}] {}-{}ms '{}'", i, s.start, s.end, s.text);
  }

  Ok(())
}
