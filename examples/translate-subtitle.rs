//! Batch processing: read a directory of subtitle files, normalize text,
//! fix timing, and convert to VTT — all in one pass.
//!
//! Run: cargo run --example batch-process

use std::path::Path;
use subtitler::SubtitleFormat;
use subtitler::model::Format;

#[tokio::main]
async fn main() -> subtitler::types::AnyResult<()> {
  let dir = "examples";
  let mut processed = 0;
  let mut errors = 0;

  for entry in std::fs::read_dir(dir)? {
    let path = entry?.path();
    let ext = match path.extension().and_then(|e| e.to_str()) {
      Some(e) => e.to_lowercase(),
      None => continue,
    };

    if !["srt", "vtt", "ass"].contains(&ext.as_str()) {
      continue;
    }

    println!("Processing: {}", path.display());

    match process_file(&path).await {
      Ok(out_path) => {
        println!("  → wrote: {}", out_path);
        processed += 1;
      }
      Err(e) => {
        eprintln!("  ✗ error: {}", e);
        errors += 1;
      }
    }
  }

  println!("\nDone: {} processed, {} errors", processed, errors);
  Ok(())
}

async fn process_file(path: &Path) -> subtitler::types::AnyResult<String> {
  let data = tokio::fs::read(path).await?;
  let mut file = subtitler::parse_bytes(&data)?;

  // 1. Normalize text in each subtitle
  for sub in file.subtitles_mut() {
    sub.text = subtitler::normalize::normalize_text(&sub.text);
  }

  // 2. Fix timing
  file.sort();
  file.enforce_min_duration(1000);
  file.remove_overlaps();

  // 3. Convert to VTT
  let vtt = file.to_string_with_format(&Format::Vtt);

  // 4. Write output
  let out_path = path.with_extension("vtt");
  tokio::fs::write(&out_path, vtt).await?;
  Ok(out_path.to_string_lossy().into_owned())
}
