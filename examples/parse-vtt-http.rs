#[macro_use]
extern crate tracing;

use std::env;
use subtitler::types::AnyResult;
use subtitler::vtt::parse_url;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main(flavor = "current_thread")]
async fn main() -> AnyResult<()> {
  let subscriber = FmtSubscriber::builder()
    .with_max_level(Level::INFO)
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

  let args: Vec<String> = env::args().collect();
  info!("args {:?}", args);
  if let Some(url) = args.get(1) {
    let subtitle = parse_url(url).await?;
    info!("subtitle {:#?}", subtitle);
    info!("subtitle json {}", serde_json::to_string_pretty(&subtitle)?);
  }
  Ok(())
}
