#[macro_use]
extern crate tracing;

use subtitler::model::{frames_to_ms, ms_to_frames};
use subtitler::types::AnyResult;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

fn main() -> AnyResult<()> {
  let subscriber = FmtSubscriber::builder()
    .with_max_level(Level::INFO)
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

  let fps = 23.976;

  info!("Frame-based timecode conversion @ {:.3} fps:", fps);
  info!("{:>8} {:>10} {:>10}", "ms", "frames", "roundtrip");
  info!("{}", "-".repeat(32));

  for ms in [0, 100, 500, 1000, 1500, 2000, 3600000] {
    let frames = ms_to_frames(ms, fps);
    let roundtrip = frames_to_ms(frames, fps);
    info!("{:>8} {:>10} {:>10}", ms, frames, roundtrip);
  }

  // Common framerate values
  info!("\nCommon framerate conversions (1000ms):");
  for fps in [23.976, 24.0, 25.0, 29.97, 30.0, 50.0, 59.94, 60.0] {
    let frames = ms_to_frames(1000, fps);
    info!("  {:.3} fps: {} frames", fps, frames);
  }

  Ok(())
}
