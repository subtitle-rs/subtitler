#[macro_use]
extern crate tracing;

mod cli;
mod types;
use crate::types::AnyResult;
use clap::Parser;
use cli::{Commands, CLI};
use subtitler::srt;
use subtitler::vtt;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> AnyResult<()> {
  let subscriber = FmtSubscriber::builder()
    .with_max_level(Level::INFO)
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

  let cli = CLI::parse();

  match cli.command {
    Some(Commands::File { path }) => {
      if path.ends_with(".vtt") {
        let subtitles = vtt::parse_file(&path).await?;
        info!("{}", serde_json::to_string_pretty(&subtitles)?);
      }
      if path.ends_with(".srt") {
        let subtitles = srt::parse_file(&path).await?;
        info!("{}", serde_json::to_string_pretty(&subtitles)?);
      }
    }
    Some(Commands::Url { url }) => {
      if url.contains(".vtt") {
        let subtitles = vtt::parse_url(&url).await?;
        info!("{}", serde_json::to_string_pretty(&subtitles)?);
      }
      if url.contains(".srt") {
        let subtitles = srt::parse_url(&url).await?;
        info!("{}", serde_json::to_string_pretty(&subtitles)?);
      }
    }

    None => {
      info!("No command provided. Use --help for more information.");
    }
  }
  Ok(())
}
