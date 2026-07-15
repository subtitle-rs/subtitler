#[macro_use]
extern crate tracing;

use subtitler::model::{SubtitleFile, frames_to_ms, ms_to_frames};
use subtitler::srt;
use subtitler::types::AnyResult;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> AnyResult<()> {
  let subscriber = FmtSubscriber::builder()
    .with_max_level(Level::INFO)
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

  let content = r#"
2
00:00:04,000 --> 00:00:06,500
I'm doing well! How about you?

1
00:00:01,000 --> 00:00:03,500
Hello there!

3
00:00:01,500 --> 00:00:03,000
This subtitle overlaps with the first two!

4
00:00:07,000 --> 00:00:12,500
This is a very long subtitle that contains way too much text to be readable at a normal reading speed and should be split into more manageable chunks
"#;

  let subtitles = srt::parse_content(content).await?;
  let mut file = SubtitleFile::Srt(subtitles);

  // 1. Sort by start time
  file.sort();
  info!("After sort: {} subtitles", file.subtitles().len());
  for sub in file.subtitles() {
    info!("  {}ms -> {}ms: {}", sub.start, sub.end, sub.text);
  }

  // 2. Validate
  let issues = file.validate();
  info!("Validation found {} issues:", issues.len());
  for issue in &issues {
    warn!("  {}", issue.description());
  }

  // Extended validation
  let ext_issues = file.validate_extended(42, 2000, 25.0);
  info!("Extended validation: {} issues", ext_issues.len());
  for issue in &ext_issues {
    warn!("  {}", issue.description());
  }

  // 3. Merge overlapping/adjacent subtitles
  let mut merge_file = file.clone();
  merge_file.merge_adjacent(500);
  info!(
    "After merge (gap <= 500ms): {} subtitles",
    merge_file.subtitles().len()
  );
  for sub in merge_file.subtitles() {
    info!("  {}ms -> {}ms: {}", sub.start, sub.end, sub.text);
  }

  // 4. Split long subtitles
  let mut split_file = file.clone();
  split_file.split_long(42);
  info!(
    "After split (max 42 chars): {} subtitles",
    split_file.subtitles().len()
  );
  for sub in split_file.subtitles() {
    info!(
      "  {}ms -> {}ms ({} chars): {}",
      sub.start,
      sub.end,
      sub.text.chars().count(),
      sub.text
    );
  }

  // 5. Shift all timestamps
  let mut shift_file = file.clone();
  shift_file.shift_all(2000);
  info!("After shift +2s:");
  for sub in shift_file.subtitles() {
    info!("  {}ms -> {}ms: {}", sub.start, sub.end, sub.text);
  }

  // 6. Framerate conversion
  let mut fps_file = file.clone();
  fps_file.transform_framerate(23.976, 25.0);
  info!("After framerate 23.976 -> 25 fps:");
  for sub in fps_file.subtitles() {
    info!("  {}ms -> {}ms: {}", sub.start, sub.end, sub.text);
  }

  // 7. Frame-based time conversion demo
  let ms = 1000u64;
  let fps = 23.976;
  let frames = ms_to_frames(ms, fps);
  let back = frames_to_ms(frames, fps);
  info!(
    "{}ms @ {:.3}fps = {} frames (round-trip = {}ms)",
    ms, fps, frames, back
  );

  Ok(())
}
