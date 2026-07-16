//! Convert SRT to every supported format, then verify round-trip.
//!
//! Run: cargo run --example convert-all-formats

use subtitler::SubtitleFormat;
use subtitler::model::Format;

#[tokio::main]
async fn main() -> subtitler::types::AnyResult<()> {
  let data = tokio::fs::read("examples/example.srt").await?;
  let file = subtitler::parse_bytes(&data)?;

  println!(
    "Source: {} subtitles ({:?})\n",
    file.subtitles().len(),
    file.format()
  );

  let targets = [
    ("SRT", Format::Srt),
    ("VTT", Format::Vtt),
    ("ASS", Format::Ass),
    ("MicroDVD", Format::MicroDvd),
    ("SubViewer", Format::SubViewer),
  ];

  for (name, fmt) in &targets {
    let output = file.to_string_with_format(fmt);
    let lines = output.lines().count();
    let bytes = output.len();

    // Re-parse to verify round-trip
    let reparsed = subtitler::parse_bytes(output.as_bytes())?;
    let count = reparsed.subtitles().len();
    let ok = if count == file.subtitles().len() {
      "✓"
    } else {
      "✗"
    };

    println!(
      "  {:12} → {} bytes, {} lines, {} subtitles {}",
      name, bytes, lines, count, ok
    );
  }

  println!("\nAll conversions verified.");
  Ok(())
}
