#[cfg(feature = "ass")]
pub mod ass;
pub mod config;
pub mod encoding;
pub mod error;
#[cfg(feature = "microdvd")]
pub mod microdvd;
pub mod model;
pub mod normalize;
#[cfg(feature = "srt")]
pub mod srt;
#[cfg(feature = "subviewer")]
pub mod subviewer;
pub mod types;
pub mod utils;
#[cfg(feature = "vtt")]
pub mod vtt;

pub use model::SubtitleFormat;

use model::Format;

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
  f
}
