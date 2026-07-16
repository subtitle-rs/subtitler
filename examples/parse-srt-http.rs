#[macro_use]
extern crate tracing;

use subtitler::srt::parse_url;
use subtitler::types::AnyResult;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main(flavor = "current_thread")]
async fn main() -> AnyResult<()> {
  let subscriber = FmtSubscriber::builder()
    .with_max_level(Level::INFO)
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
  let url = "http://acadsoc-info.oss-accelerate.aliyuncs.com/common/b6a05931-637a-4d9b-a3f3-bdd034354464.srt";

  let subtitle = parse_url(url).await?;
  info!("subtitle {:#?}", subtitle);
  info!("subtitle json {}", serde_json::to_string_pretty(&subtitle)?);
  Ok(())
}
