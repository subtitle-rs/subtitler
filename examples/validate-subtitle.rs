//! Detect format from bytes, then validate timing and quality.
//!
//! Run: cargo run --example detect-and-validate

use subtitler::SubtitleFormat;

fn main() -> subtitler::types::AnyResult<()> {
  let samples: &[(&str, &[u8])] = &[
    ("SRT",       b"1\n00:00:01,000 --> 00:00:03,500\nHello\n\n"),
    ("VTT",       b"WEBVTT\n\n00:00:01.000 --> 00:00:03.500\nHello\n\n"),
    ("ASS",       b"[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: ...\nStyle: Default,Arial,48\n\n[Events]\nFormat: Layer,Start,End,Style,Name,MarginL,MarginR,MarginV,Effect,Text\nDialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,Hi\n"),
    ("TTML",      b"<?xml version=\"1.0\"?><tt xmlns=\"http://www.w3.org/ns/ttml\"><body><div><p begin=\"00:00:01.000\" end=\"00:00:03.500\">Hi</p></div></body></tt>"),
    ("SBV",       b"0:00:01.000,0:00:03.500,Hello\n"),
    ("LRC",       b"[00:01.50]Hello\n"),
    ("MicroDVD",  b"{25}{50}Hello\n"),
    ("Unknown",   b"not a subtitle format\n"),
  ];

  println!("=== Format Detection ===\n");
  for (label, data) in samples {
    match subtitler::detect_format(data) {
      Some(fmt) => println!("  {:12} → detected as {:?}", label, fmt),
      None => println!("  {:12} → unknown", label),
    }
  }

  // Validate a file with known issues
  println!("\n=== Validation ===\n");
  let bad = b"1\n00:00:01,000 --> 00:00:03,500\nHello\n\n\
              2\n00:00:02,000 --> 00:00:05,000\nOverlap!\n\n\
              3\n00:00:06,000 --> 00:00:06,000\nZero duration\n";
  let file = subtitler::parse_bytes(bad)?;
  let issues = file.validate();

  if issues.is_empty() {
    println!("  No issues found.");
  } else {
    println!("  Found {} issues:", issues.len());
    for issue in &issues {
      println!("    - {}", issue.description());
    }
  }

  // Extended validation with CPS
  println!("\n=== Extended Validation (CPS, text length, gaps) ===\n");
  let issues = file.validate_extended(42, 5000, 25.0);
  if issues.is_empty() {
    println!("  No issues found.");
  } else {
    for issue in &issues {
      println!("    - {}", issue.description());
    }
  }

  Ok(())
}
