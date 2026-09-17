//! iTT (iTunes Timed Text) parser and generator.
//!
//! iTT is Apple's subtitle delivery profile for the iTunes Store and Apple
//! TV, built on IMSC1 (a TTML profile). Files carry the TTML namespace
//! plus the IMSC1 profile designator:
//! `ttp:profile="http://www.w3.org/ns/ttml/profile/imsc1"`.
//!
//! Parsing and serialization delegate to the TTML module — the IMSC1
//! structure is a superset of what the TTML parser extracts (timings and
//! text). Note that `to_string` emits a base TTML document without the
//! IMSC1 profile attribute; strict Apple delivery validation is future
//! work.

use crate::model::{Format, Subtitle, SubtitleFile};
use crate::types::AnyResult;

/// Detect iTT by the IMSC1 profile designator.
///
/// Must run BEFORE `ttml::detect_format`: iTT files carry the plain TTML
/// namespace too, so the TTML detector would otherwise claim them.
pub fn detect_format(data: &[u8]) -> Option<Format> {
  let text = crate::encoding::try_decode_for_detection(data)?;
  if text.contains("ttml/profile/imsc1") {
    return Some(Format::Itt);
  }
  None
}

/// Parse iTT content — delegates to the TTML parser.
pub fn parse_content(content: &str) -> AnyResult<SubtitleFile> {
  let file = crate::ttml::parse_content(content)?;
  match file {
    SubtitleFile::Ttml { header, subtitles } => Ok(SubtitleFile::Itt { header, subtitles }),
    _ => Ok(file),
  }
}

/// Parse iTT bytes — auto-detect encoding then parse.
pub fn parse_bytes(data: &[u8]) -> AnyResult<SubtitleFile> {
  let content = crate::encoding::decode_to_string(data)?;
  parse_content(&content)
}

/// Parse iTT from file.
#[cfg(not(target_arch = "wasm32"))]
pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<SubtitleFile> {
  let text = tokio::fs::read_to_string(path).await?;
  parse_content(&text)
}

/// Parse iTT from URL (requires `http` feature).
#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<SubtitleFile> {
  let response = reqwest::get(url).await?;
  let text = response.text().await?;
  parse_content(&text)
}

/// Serialize subtitles to iTT format — delegates to the TTML serializer.
pub fn to_string(subtitles: &[Subtitle], header: Option<&str>) -> String {
  crate::ttml::to_string(subtitles, header)
}

/// Write subtitles to a file in iTT format.
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
  fn test_detect_itt_imsc1_profile() {
    let data = br#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml"
    xmlns:ttp="http://www.w3.org/ns/ttml#parameter"
    ttp:profile="http://www.w3.org/ns/ttml/profile/imsc1">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.500">Hello iTT</p>
    </div>
  </body>
</tt>"#;
    assert_eq!(detect_format(data), Some(Format::Itt));
  }

  #[test]
  fn test_detect_plain_ttml_not_itt() {
    let data = br#"<?xml version="1.0"?>
<tt xmlns="http://www.w3.org/ns/ttml">
  <body><div><p begin="00:00:01.000" end="00:00:02.000">Hello</p></div></body>
</tt>"#;
    assert_eq!(detect_format(data), None);
  }

  #[test]
  fn test_detect_precedes_ttml_in_chain() {
    // With both features on, the IMSC1 profile must route to Itt before
    // the TTML detector (which matches the shared namespace) claims it.
    let data = br#"<tt xmlns="http://www.w3.org/ns/ttml" ttp:profile="http://www.w3.org/ns/ttml/profile/imsc1"><body><div><p begin="00:00:01.000" end="00:00:02.000">Hi</p></div></body></tt>"#;
    assert_eq!(crate::detect_format(data), Some(Format::Itt));
  }

  #[test]
  fn test_parse_and_round_trip() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" ttp:profile="http://www.w3.org/ns/ttml/profile/imsc1">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.500">Hello iTT</p>
    </div>
  </body>
</tt>"#;
    let file = parse_content(xml).unwrap();
    assert_eq!(file.format(), Format::Itt);
    let subs = file.subtitles();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].text, "Hello iTT");
    assert_eq!(subs[0].start, 1_000);
    assert_eq!(subs[0].end, 3_500);
    let out = to_string(subs, None);
    assert!(out.contains("Hello iTT"));
  }
}
