//! Parse an MPL2 (.mpl) subtitle file.
//!
//! MPL2 is a frame-based subtitle format popular in Eastern Europe.
//!
//! # Features
//! - Frame-accurate timing
//! - Simple format: [start_frame][end_frame]text
//! - Default frame rate: 23.976 fps
//!
//! # Run
//! ```sh
//! cargo run --example parse-mpl2-content
//! ```

use subtitler::mpl2::{Mpl2Data, DEFAULT_FPS};

fn main() -> anyhow::Result<()> {
  let content = r#"[100][200]First subtitle line
[300][450]Second subtitle line
[500][650]Third subtitle line
"#;

  println!("=== MPL2 File Info ===");
  println!("Default FPS: {}", DEFAULT_FPS);

  // Parse MPL2 content
  let data = Mpl2Data::parse(content, None)?;

  println!("Subtitles: {}", data.subtitles.len());
  println!("Frame rate: {} fps", data.fps);

  println!("\n=== Subtitles ===");
  for (i, sub) in data.subtitles.iter().enumerate() {
    let start_frame = (sub.start as f64 / 1000.0 * data.fps).round() as u64;
    let end_frame = (sub.end as f64 / 1000.0 * data.fps).round() as u64;

    println!(
      "{}. Frame [{:>4}-{:<4}] Time [{:>5}ms - {:>5}ms] {}",
      i + 1,
      start_frame,
      end_frame,
      sub.start,
      sub.end,
      sub.text
    );
  }

  // Example with custom frame rate
  println!("\n=== Custom Frame Rate (25 fps) ===");
  let custom_data = Mpl2Data::parse(content, Some(25.0))?;
  println!("Frame rate: {} fps", custom_data.fps);

  for (i, sub) in custom_data.subtitles.iter().enumerate() {
    println!(
      "{}. [{:>5}ms - {:>5}ms] {}",
      i + 1,
      sub.start,
      sub.end,
      sub.text
    );
  }

  Ok(())
}