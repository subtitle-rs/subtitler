use thiserror::Error;
#[derive(Debug, Error)]
pub enum Error {
  #[error("json error {0:?}")]
  Json(#[from] serde_json::Error),
  #[error("IO error {0:?}")]
  IO(#[from] std::io::Error),
  #[error("any error {0:?}")]
  Any(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[macro_use]
extern crate tracing;

use std::env;
use subtitler::vtt::parse_url;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
#[tokio::main]
async fn main() -> Result<()> {
  let subscriber = FmtSubscriber::builder()
    .with_max_level(Level::INFO)
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

  let args: Vec<String> = env::args().collect();
  info!("args {:?}", args);
  if let Some(url) = args.get(1) {
    let subtitle = parse_url(url).await?;
    info!("subtitle {:?}", subtitle);
    info!("subtitle json {}", serde_json::to_string(&subtitle)?);
  }
  Ok(())
}
