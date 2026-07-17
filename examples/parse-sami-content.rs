//! Parse a SAMI (.smi) subtitle file.
//!
//! SAMI (Synchronized Accessible Media Interchange) is a Microsoft-developed
//! subtitle format widely used in Asian markets, especially Korea and China.
//!
//! # Features
//! - HTML-like structure with `<Sync>` and `<P>` tags
//! - Multi-language support
//! - CSS styling
//!
//! # Run
//! ```sh
//! cargo run --example parse-sami-content
//! ```

use subtitler::model::SubtitleFile;

fn main() -> anyhow::Result<()> {
  let content = r#"<SAMI>
<Head>
  <Title>Example Subtitle</Title>
  <Style Type="text/css">
  <!--
    .ENCC {Name: English; lang: en-US;}
  -->
  </Style>
</Head>
<Body>
  <Sync Start=1000><P Class=ENCC>First subtitle line</P></Sync>
  <Sync Start=4000><P Class=ENCC>Second subtitle line</P></Sync>
  <Sync Start=7000><P Class=ENCC>Third subtitle line</P></Sync>
</Body>
</SAMI>"#;

  // Parse SAMI content
  let file = subtitler::sami::parse_content(content)?;

  match file {
    SubtitleFile::Sami(data) => {
      println!("=== SAMI File Info ===");
      println!("Subtitles: {}", data.subtitles.len());

      if let Some(ref header) = data.header {
        println!("\nHeader:\n{}", header);
      }

      if !data.styles.is_empty() {
        println!("\nStyles:");
        for (name, style) in &data.styles {
          println!("  .{}: {}", name, style);
        }
      }

      println!("\n=== Subtitles ===");
      for (i, sub) in data.subtitles.iter().enumerate() {
        println!(
          "{}. [{:>5}ms - {:>5}ms] {}",
          i + 1,
          sub.start,
          sub.end,
          sub.text
        );
      }
    }
    _ => println!("Unexpected format"),
  }

  Ok(())
}