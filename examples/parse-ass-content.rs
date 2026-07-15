#[macro_use]
extern crate tracing;

use subtitler::ass;
use subtitler::model::SubtitleFormat;
use subtitler::types::AnyResult;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

fn main() -> AnyResult<()> {
  let subscriber = FmtSubscriber::builder()
    .with_max_level(Level::INFO)
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

  let content = r#"[Script Info]
Title: Example ASS
ScriptType: v4.00+
PlayResX: 384
PlayResY: 288
WrapStyle: 0

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1
Style: Italic,Arial,36,&H0000FFFF,&H000000FF,&H00000000,&H00000000,-1,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,Hello! How are you today?
Dialogue: 0,0:00:04.00,0:00:06.50,Default,,0,0,0,,I'm doing well, thank you!
Dialogue: 0,0:00:07.00,0:00:09.50,Default,,0,0,0,,What are your plans?
Dialogue: 0,0:00:10.00,0:00:12.50,Default,,0,0,0,,I might go hiking.
Dialogue: 0,0:00:13.00,0:00:15.50,Italic,,0,0,0,,That sounds great!
"#;

  let file = ass::parse_content(content)?;
  info!("Parsed {} subtitles", file.subtitles().len());
  info!("Styles: {:?}", file);

  let output = file.to_string();
  info!("Re-generated ASS:\n{}", output);

  Ok(())
}
