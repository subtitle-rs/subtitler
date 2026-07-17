//! Create a SAMI (.smi) subtitle file.
//!
//! Demonstrates how to programmatically create SAMI files
//! with multi-language support and styling.
//!
//! # Run
//! ```sh
//! cargo run --example create-sami-file
//! ```

use std::collections::HashMap;
use subtitler::model::Subtitle;
use subtitler::sami::SamiData;

fn main() -> anyhow::Result<()> {
  println!("=== Creating SAMI File ===\n");

  // Create subtitles
  let subtitles = vec![
    Subtitle::new(1000, 3000, "Welcome to our tutorial"),
    Subtitle::new(3500, 5500, "This is a SAMI subtitle file"),
    Subtitle::new(6000, 8000, "Widely used in Asia"),
    Subtitle::new(8500, 10500, "Supports multiple languages"),
    Subtitle::new(11000, 13000, "Thank you for watching!"),
  ];

  // Create SAMI data with header
  let mut styles = HashMap::new();
  styles.insert(
    "ENCC".to_string(),
    "{Name: English; lang: en-US; SAMIType: CC;}".to_string(),
  );
  styles.insert(
    "KRCC".to_string(),
    "{Name: Korean; lang: ko-KR; SAMIType: CC;}".to_string(),
  );

  let header = Some(
    r#"<Head>
<Title>Multi-language Subtitle</Title>
<Style Type="text/css">
<!--
  .ENCC {Name: English; lang: en-US; SAMIType: CC;}
  .KRCC {Name: Korean; lang: ko-KR; SAMIType: CC;}
-->
</Style>
</Head>"#
      .to_string(),
  );

  let sami_data = SamiData {
    header,
    styles,
    subtitles,
  };

  // Generate SAMI output
  let output = sami_data.to_string();

  println!("Generated SAMI content:\n");
  println!("{}", output);

  // Save to file
  std::fs::write("output.smi", &output)?;
  println!("\n✓ Saved to output.smi");

  // Verify by re-parsing
  println!("\n=== Verification ===");
  let parsed = subtitler::sami::parse_content(&output)?;
  if let subtitler::model::SubtitleFile::Sami(data) = parsed {
    println!("✓ Successfully parsed {} subtitles", data.subtitles.len());
    println!("✓ Styles found: {}", data.styles.len());
  }

  Ok(())
}
