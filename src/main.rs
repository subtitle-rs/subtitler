mod cli;
mod types;
use crate::types::AnyResult;
use clap::Parser;
use cli::{Cli, Commands};
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

  let cli = Cli::parse();

  match cli.command {
    Some(Commands::File { path }) => {
      if path.ends_with(".vtt") {
        let subtitles = vtt::parse_file(&path).await?;
        println!("{}", serde_json::to_string_pretty(&subtitles)?);
      } else if path.ends_with(".srt") {
        let subtitles = srt::parse_file(&path).await?;
        println!("{}", serde_json::to_string_pretty(&subtitles)?);
      } else {
        eprintln!(
          "Unknown file format. Expected .srt or .vtt extension: {}",
          path
        );
      }
    }
    #[cfg(feature = "http")]
    Some(Commands::Url { url }) => {
      if url.contains(".vtt") {
        let subtitles = vtt::parse_url(&url).await?;
        println!("{}", serde_json::to_string_pretty(&subtitles)?);
      } else if url.contains(".srt") {
        let subtitles = srt::parse_url(&url).await?;
        println!("{}", serde_json::to_string_pretty(&subtitles)?);
      } else {
        eprintln!("Unknown URL format. Expected .srt or .vtt in URL: {}", url);
      }
    }
    #[cfg(not(feature = "http"))]
    Some(Commands::Url { .. }) => {
      eprintln!("`url` command requires the `http` feature. Rebuild with default features.");
    }

    None => {
      println!("No command provided. Use --help for more information.");
    }
  }
  Ok(())
}
