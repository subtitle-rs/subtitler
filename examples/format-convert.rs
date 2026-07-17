#[macro_use]
extern crate tracing;

use subtitler::SubtitleFormat;
use subtitler::srt;
use subtitler::types::AnyResult;
use subtitler::vtt;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main(flavor = "current_thread")]
async fn main() -> AnyResult<()> {
  let subscriber = FmtSubscriber::builder()
    .with_max_level(Level::INFO)
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

  let srt_content = r#"
1
00:00:01,000 --> 00:00:03,500
Hello! How are you today?

2
00:00:04,000 --> 00:00:06,500
I'm doing well, thank you!

3
00:00:07,000 --> 00:00:09,500
What are your plans for the weekend?
"#;

  let subtitles = srt::parse_content(srt_content)?;
  info!("Parsed {} SRT cues", subtitles.subtitles().len());

  // Convert SRT -> VTT
  let vtt_output = vtt::to_string(subtitles.subtitles(), None);
  info!("VTT output:\n{}", vtt_output);

  // Convert SRT -> ASS
  let ass_output = subtitler::ass::to_string(
    &Default::default(),
    &[subtitler::model::AssStyle::default_style()],
    subtitles.subtitles(),
  );
  info!("ASS output:\n{}", ass_output);

  // Round-trip: SRT -> VTT -> parse -> SRT
  let reparsed = vtt::parse_content(&vtt_output)?;
  let srt_roundtrip = srt::to_string(&reparsed);
  info!("Round-trip SRT:\n{}", srt_roundtrip);

  Ok(())
}
