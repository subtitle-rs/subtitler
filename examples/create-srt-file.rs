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

use subtitler::srt::parse_content;
use subtitler::srt::generate;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
#[tokio::main]
async fn main() -> Result<()> {
  let subscriber = FmtSubscriber::builder()
    .with_max_level(Level::INFO)
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
  let content: &str = r#"
1
00:00:01,000 --> 00:00:03,500
Hello! How are you today?

2
00:00:04,000 --> 00:00:06,500
I'm doing well, thank you!

3
00:00:07,000 --> 00:00:09,500
What are your plans for the weekend?

4
00:00:10,000 --> 00:00:12,500
I might go hiking if the weather is nice.

5
00:00:13,000 --> 00:00:15,500
That sounds like a great idea!

6
00:00:16,000 --> 00:00:18,500
Would you like to join me?

7
00:00:19,000 --> 00:00:21,500
I would love to! What time do you want to go?

8
00:00:22,000 --> 00:00:24,500
How about 8 AM? 

9
00:00:25,000 --> 00:00:27,500
Perfect! I'll see you then.

10
00:00:28,000 --> 00:00:30,500
Great! Looking forward to it.
"#;

  let subtitle = parse_content(content).await?;
  info!("subtitle {:#?}", subtitle);
  info!("subtitle json {}", serde_json::to_string_pretty(&subtitle)?);

  generate(&subtitle, "test.srt").await?;
  Ok(())
}
