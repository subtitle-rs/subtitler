#[macro_use]
extern crate tracing;

use std::env;
use std::path::PathBuf;
use subtitler::srt::parse_file;
use subtitler::types::AnyResult;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> AnyResult<()> {
  let subscriber = FmtSubscriber::builder()
    .with_max_level(Level::INFO)
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
  let mut file_path: PathBuf = env::current_dir().expect("Failed to get current_dir");
  file_path.push("examples/example.srt");

  let subtitle = parse_file(&file_path).await?;
  info!("subtitle {:#?}", subtitle);
  info!("subtitle json {}", serde_json::to_string_pretty(&subtitle)?);
  Ok(())
}
