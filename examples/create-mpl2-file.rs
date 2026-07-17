//! Create an MPL2 (.mpl) subtitle file.
//!
//! Demonstrates how to create MPL2 files with frame-based timing.
//!
//! # Run
//! ```sh
//! cargo run --example create-mpl2-file
//! ```

use subtitler::model::Subtitle;
use subtitler::mpl2::{DEFAULT_FPS, Mpl2Data};

fn main() -> anyhow::Result<()> {
  println!("=== Creating MPL2 File ===\n");

  // Create subtitles (timestamps in milliseconds)
  let subtitles = vec![
    Subtitle::new(4000, 8000, "MPL2 frame-based subtitles"),
    Subtitle::new(9000, 13000, "Popular in Eastern Europe"),
    Subtitle::new(14000, 18000, "Frame-accurate timing"),
    Subtitle::new(19000, 23000, "Default 23.976 fps"),
    Subtitle::new(24000, 28000, "Simple and efficient"),
  ];

  // Create MPL2 data with default frame rate
  let mpl2_data = Mpl2Data {
    fps: DEFAULT_FPS,
    subtitles: subtitles.clone(),
  };

  println!("Frame rate: {} fps\n", mpl2_data.fps);
  println!("=== Frame Conversion ===");

  for (i, sub) in mpl2_data.subtitles.iter().enumerate() {
    let start_frame = (sub.start as f64 / 1000.0 * mpl2_data.fps).round() as u64;
    let end_frame = (sub.end as f64 / 1000.0 * mpl2_data.fps).round() as u64;

    println!(
      "{}. {:>5}ms-{:<5}ms → Frame [{:>4}-{:<4}] {}",
      i + 1,
      sub.start,
      sub.end,
      start_frame,
      end_frame,
      sub.text
    );
  }

  // Generate MPL2 output
  let output = mpl2_data.render();

  println!("\n=== Generated MPL2 Content ===\n");
  println!("{}", output);

  // Save to file
  std::fs::write("output.mpl", &output)?;
  println!("✓ Saved to output.mpl");

  // Example with different frame rate (25 fps)
  println!("\n=== Alternative Frame Rate (25 fps) ===");
  let mpl2_25fps = Mpl2Data {
    fps: 25.0,
    subtitles,
  };

  let output_25fps = mpl2_25fps.render();
  println!("{}", output_25fps);

  std::fs::write("output_25fps.mpl", &output_25fps)?;
  println!("✓ Saved to output_25fps.mpl");

  // Verify by re-parsing
  println!("\n=== Verification ===");
  let parsed = Mpl2Data::parse(&output, None)?;
  println!("✓ Successfully parsed {} subtitles", parsed.subtitles.len());
  println!("✓ Frame rate: {} fps", parsed.fps);

  Ok(())
}
