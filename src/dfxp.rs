//! DFXP (Distribution Format Exchange Profile) parser and generator.
//!
//! DFXP is the W3C predecessor to TTML. It shares the same XML structure
//! but uses a different namespace: `xmlns="http://www.w3.org/2006/04/ttaf1"`.
//! This module delegates all parsing and serialization to the TTML module.

use crate::model::{Format, Subtitle, SubtitleFile};
use crate::types::AnyResult;

/// Detect DFXP by the TT AF 1.0 namespace (`xmlns="http://www.w3.org/2006/04/ttaf1"`).
pub fn detect_format(data: &[u8]) -> Option<Format> {
  let text = crate::encoding::try_decode_for_detection(data)?;
  if text.contains("http://www.w3.org/2006/04/ttaf1") {
    return Some(Format::Dfxp);
  }
  None
}

/// Parse DFXP content — delegates to TTML parser.
pub fn parse_content(content: &str) -> AnyResult<SubtitleFile> {
  let file = crate::ttml::parse_content(content)?;
  // Wrap as Dfxp variant, preserving subtitle content.
  match file {
    SubtitleFile::Ttml { header, subtitles } => Ok(SubtitleFile::Dfxp { header, subtitles }),
    _ => Ok(file),
  }
}

/// Parse DFXP bytes — auto-detect encoding then parse.
pub fn parse_bytes(data: &[u8]) -> AnyResult<SubtitleFile> {
  let content = crate::encoding::decode_to_string(data)?;
  parse_content(&content)
}

/// Parse DFXP from file.
#[cfg(not(target_arch = "wasm32"))]
pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<SubtitleFile> {
  let text = tokio::fs::read_to_string(path).await?;
  parse_content(&text)
}

/// Parse DFXP from URL (requires `http` feature).
#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<SubtitleFile> {
  let response = reqwest::get(url).await?;
  let text = response.text().await?;
  parse_content(&text)
}

/// Serialize subtitles to DFXP format — delegates to TTML serializer.
pub fn to_string(subtitles: &[Subtitle], header: Option<&str>) -> String {
  crate::ttml::to_string(subtitles, header)
}

/// Write subtitles to a file in DFXP format.
#[cfg(not(target_arch = "wasm32"))]
pub async fn generate(
  subtitles: &[Subtitle],
  file_path: impl AsRef<std::path::Path>,
  policy: Option<crate::model::WritePolicy>,
) -> AnyResult<String> {
  let content = to_string(subtitles, None);
  let path = file_path.as_ref();
  crate::io::write_with_policy(path, content.as_bytes(), policy).await?;
  Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::SubtitleFormat;

  #[test]
  fn test_detect_dfxp() {
    let data = br#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/2006/04/ttaf1"
    xmlns:tts="http://www.w3.org/2006/04/ttaf1#styling">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.500">Hello DFXP</p>
    </div>
  </body>
</tt>"#;
    assert_eq!(detect_format(data), Some(Format::Dfxp));
  }

  #[test]
  fn test_detect_dfxp_not_ttml() {
    let data = br#"<?xml version="1.0"?>
<tt xmlns="http://www.w3.org/ns/ttml">
  <body><div><p begin="00:00:01.000" end="00:00:03.500">Hello</p></div></body>
</tt>"#;
    assert_eq!(detect_format(data), None); // TTML namespace, not DFXP
  }

  #[test]
  fn test_parse_round_trip() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/2006/04/ttaf1"
    xmlns:tts="http://www.w3.org/2006/04/ttaf1#styling">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.500">Hello DFXP</p>
    </div>
  </body>
</tt>"#;
    let file = parse_content(xml).unwrap();
    assert_eq!(file.subtitles().len(), 1);
    assert_eq!(file.subtitles()[0].text, "Hello DFXP");
    let out = to_string(file.subtitles(), None);
    assert!(!out.is_empty());
  }
}
