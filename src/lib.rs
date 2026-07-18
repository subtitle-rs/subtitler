#[cfg(feature = "ass")]
pub mod ass;
pub mod config;
#[cfg(feature = "dfxp")]
pub mod dfxp;
#[cfg(feature = "ebu_stl")]
pub mod ebu_stl;
pub mod encoding;
pub mod error;
#[cfg(not(target_arch = "wasm32"))]
pub mod io;
#[cfg(feature = "lrc")]
pub mod lrc;
#[cfg(feature = "microdvd")]
pub mod microdvd;
pub mod model;
#[cfg(feature = "mpl2")]
pub mod mpl2;
pub mod normalize;
pub mod pipeline;
pub mod quality;
#[cfg(feature = "sami")]
pub mod sami;
#[cfg(feature = "sbv")]
pub mod sbv;
#[cfg(feature = "scc")]
pub mod scc;
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
  Format, ParseConfig, StreamingParser, Subtitle, SubtitleFile, SubtitleFileBuilder, TextPart,
  WritePolicy,
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
  #[cfg(feature = "dfxp")]
  let f = f.or_else(|| dfxp::detect_format(data));
  #[cfg(feature = "sbv")]
  let f = f.or_else(|| sbv::detect_format(data));
  #[cfg(feature = "lrc")]
  let f = f.or_else(|| lrc::detect_format(data));
  #[cfg(feature = "sami")]
  let f = f.or_else(|| sami::detect_format(data));
  #[cfg(feature = "mpl2")]
  let f = f.or_else(|| mpl2::detect_format(data));
  #[cfg(feature = "scc")]
  let f = f.or_else(|| scc::detect_format(data));
  #[cfg(feature = "ebu_stl")]
  let f = f.or_else(|| ebu_stl::detect_format(data));
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
    Format::Srt => Ok(srt::parse_bytes(data)?),
    #[cfg(feature = "vtt")]
    Format::Vtt => Ok(vtt::parse_bytes(data)?),
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
    Format::Ttml => Ok(ttml::parse_bytes(data)?),
    #[cfg(feature = "sbv")]
    Format::Sbv => Ok(sbv::parse_bytes(data)?),
    #[cfg(feature = "lrc")]
    Format::Lrc => Ok(lrc::parse_bytes(data)?),
    #[cfg(feature = "sami")]
    Format::Sami => Ok(sami::parse_bytes(data)?),
    #[cfg(feature = "mpl2")]
    Format::Mpl2 => Ok(mpl2::parse_bytes(data)?),
    #[cfg(feature = "scc")]
    Format::Scc => Ok(scc::parse_bytes(data)?),
    #[cfg(feature = "ebu_stl")]
    Format::EbuStl => Ok(ebu_stl::parse_bytes(data)?),
    #[cfg(feature = "dfxp")]
    Format::Dfxp => Ok(dfxp::parse_bytes(data)?),
    #[allow(unreachable_patterns)]
    _ => Err(error::ParseError::Unsupported(fmt)),
  }
}

/// Parse a file into a `SubtitleFile`, auto-detecting the format.
#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(target_arch = "wasm32")]
pub mod wasm;
