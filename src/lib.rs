#[cfg(feature = "ass")]
pub mod ass;
pub mod config;
pub mod encoding;
pub mod error;
#[cfg(feature = "lrc")]
pub mod lrc;
#[cfg(feature = "microdvd")]
pub mod microdvd;
pub mod model;
pub mod normalize;
pub mod quality;
#[cfg(feature = "sbv")]
pub mod sbv;
#[cfg(feature = "srt")]
pub mod srt;
#[cfg(feature = "subviewer")]
pub mod subviewer;
#[cfg(feature = "ttml")]
pub mod ttml;
pub mod types;
pub mod utils;
#[cfg(feature = "vtt")]
pub mod vtt;

pub use model::SubtitleFormat;

// Re-export commonly used types for convenience
pub use model::{
  Format, ParseConfig, Subtitle, SubtitleFile, SubtitleFileBuilder, TextPart, WritePolicy,
};

pub fn detect_format(data: &[u8]) -> Option<Format> {
  #[cfg(feature = "srt")]
  let f = srt::detect_format(data);
  #[cfg(not(feature = "srt"))]
  let f: Option<Format> = None;

  #[cfg(feature = "vtt")]
  let f = f.or_else(|| vtt::detect_format(data));
  #[cfg(feature = "ass")]
  let f = f.or_else(|| ass::detect_format(data));
  #[cfg(feature = "microdvd")]
  let f = f.or_else(|| microdvd::detect_format(data));
  #[cfg(feature = "subviewer")]
  let f = f.or_else(|| subviewer::detect_format(data));
  #[cfg(feature = "ttml")]
  let f = f.or_else(|| ttml::detect_format(data));
  #[cfg(feature = "sbv")]
  let f = f.or_else(|| sbv::detect_format(data));
  #[cfg(feature = "lrc")]
  let f = f.or_else(|| lrc::detect_format(data));
  f
}

/// Parse bytes into a `SubtitleFile`, auto-detecting the format.
pub fn parse_bytes(data: &[u8]) -> Result<model::SubtitleFile, error::ParseError> {
  let fmt = detect_format(data).ok_or(error::ParseError::UnknownFormat)?;
  parse_bytes_as(data, fmt)
}

/// Parse bytes as a specific format.
pub fn parse_bytes_as(data: &[u8], fmt: Format) -> Result<model::SubtitleFile, error::ParseError> {
  match fmt {
    #[cfg(feature = "srt")]
    Format::Srt => Ok(model::SubtitleFile::Srt(srt::parse_bytes(data)?)),
    #[cfg(feature = "vtt")]
    Format::Vtt => {
      let (header, subs) = vtt::parse_bytes_full(data)?;
      Ok(model::SubtitleFile::Vtt {
        header,
        subtitles: subs,
      })
    }
    #[cfg(feature = "ass")]
    Format::Ass => Ok(ass::parse_bytes(data)?),
    #[cfg(feature = "ssa")]
    Format::Ssa => match ass::parse_bytes(data)? {
      model::SubtitleFile::Ass(data) => Ok(model::SubtitleFile::Ssa(data)),
      other => Ok(other),
    },
    #[cfg(feature = "microdvd")]
    Format::MicroDvd => {
      let file = microdvd::parse_bytes(data, None)?;
      Ok(file)
    }
    #[cfg(feature = "subviewer")]
    Format::SubViewer => Ok(subviewer::parse_bytes(data)?),
    #[cfg(feature = "ttml")]
    Format::Ttml => {
      let subs = ttml::parse_bytes(data)?;
      Ok(model::SubtitleFile::Ttml {
        header: None,
        subtitles: subs,
      })
    }
    #[cfg(feature = "sbv")]
    Format::Sbv => Ok(model::SubtitleFile::Sbv(sbv::parse_bytes(data)?)),
    #[cfg(feature = "lrc")]
    Format::Lrc => Ok(model::SubtitleFile::Lrc(lrc::parse_bytes(data)?)),
    #[allow(unreachable_patterns)]
    _ => Err(error::ParseError::Unsupported(fmt)),
  }
}

/// Parse a file into a `SubtitleFile`, auto-detecting the format.
pub async fn parse_file(
  path: impl AsRef<std::path::Path>,
) -> Result<model::SubtitleFile, error::ParseError> {
  let data = tokio::fs::read(path).await?;
  parse_bytes(&data)
}

/// Parse a URL into a `SubtitleFile`, auto-detecting the format (requires
/// the `http` feature).
#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> Result<model::SubtitleFile, error::ParseError> {
  let client = reqwest::Client::new();
  parse_url_with(url, &client).await
}

/// Parse a URL with a custom `reqwest::Client`, allowing configuration of
/// timeouts, redirect policy, TLS options, etc.
#[cfg(feature = "http")]
pub async fn parse_url_with(
  url: &str,
  client: &reqwest::Client,
) -> Result<model::SubtitleFile, error::ParseError> {
  let response = client.get(url).send().await?;
  let bytes = response.bytes().await?;
  parse_bytes(&bytes)
}
