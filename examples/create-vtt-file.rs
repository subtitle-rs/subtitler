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

use subtitler::vtt::{parse_content, generate};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
#[tokio::main]
async fn main() -> Result<()> {
  let subscriber = FmtSubscriber::builder()
    .with_max_level(Level::INFO)
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
  let content: &str = r#"
WEBVTT

1
00:00:01.000 --> 00:00:03.500
Hi there! How have you been?

2
00:00:04.000 --> 00:00:06.500
I've been good, just busy with work.

3
00:00:07.000 --> 00:00:09.500
What project are you currently working on?

4
00:00:10.000 --> 00:00:12.500
I'm developing a new feature for our app.

5
00:00:13.000 --> 00:00:15.500
That sounds exciting! What does it do?

6
00:00:16.000 --> 00:00:18.500
It's a tool for user feedback and surveys.

7
00:00:19.000 --> 00:00:21.500
Nice! When do you think it will be ready?

8
00:00:22.000 --> 00:00:24.500
I expect to finish it by next week.

9
00:00:25.000 --> 00:00:27.500
Great! I can't wait to try it out.

10
00:00:28.000 --> 00:00:30.500
Thanks! I'm looking forward to your feedback.
"#;

  let subtitle = parse_content(content).await?;
  info!("subtitle {:#?}", subtitle);
  info!("subtitle json {}", serde_json::to_string_pretty(&subtitle)?);

  generate(&subtitle, "test.vtt").await?;

  Ok(())
}
