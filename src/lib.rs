pub mod ass;
pub mod config;
pub mod model;
pub mod srt;
pub mod types;
pub mod utils;
pub mod vtt;

use model::SubtitleFormat;

pub fn detect_format(data: &[u8]) -> Option<SubtitleFormat> {
  srt::detect_format(data)
    .or_else(|| vtt::detect_format(data))
    .or_else(|| ass::detect_format(data))
}
