#[macro_use]
extern crate tracing;

use std::env;
use std::path::PathBuf;
use subtitler::srt::parse_file;
mod error;
mod types;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
#[tokio::main]
async fn main() -> types::Result<()> {
  let subscriber = FmtSubscriber::builder()
    .with_max_level(Level::INFO)
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
  let mut file_path: PathBuf = env::current_dir().expect("Failed to get current_dir");
  file_path.push("example.srt");

  let subtitle = parse_file(file_path.to_str().unwrap()).await?;
  info!("subtitle {:?}", subtitle);
  info!("subtitle json {}", serde_json::to_string(&subtitle)?);
  Ok(())
}
