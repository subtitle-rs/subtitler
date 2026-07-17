//! EBU STL (Standard Transmission Format) parser and generator.
//!
//! EBU STL is a binary subtitle format standardized by the European Broadcasting Union
//! for professional broadcast applications. It supports precise timing, multi-language
//! subtitles, and rich formatting.
//!
//! # Structure
//! - GSI (General Subtitle Information) block: File header with metadata
//! - TTI (Text and Timing Information) blocks: Subtitle entries
//!
//! # Timecode Format
//! - SMPTE timecode: HH:MM:SS:FF (hours:minutes:seconds:frames)
//! - Frame rates: 25 fps (PAL) or 29.97 fps (NTSC)

use crate::error::SubtitleError;
use crate::model::{Format, Subtitle, SubtitleFile};
use crate::types::AnyResult;
use serde::{Deserialize, Serialize};

/// EBU STL file structure
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EbuStlData {
  /// GSI block: General Subtitle Information
  pub gsi: GsiBlock,
  /// Subtitles (converted from TTI blocks)
  pub subtitles: Vec<Subtitle>,
  /// Raw TTI blocks (for advanced use)
  pub tti_blocks: Vec<TtiBlock>,
}

/// GSI (General Subtitle Information) Block
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GsiBlock {
  /// Character code table number (0-15)
  pub code_page: u8,
  /// Disk format code
  pub disk_format_code: String,
  /// Display standard code
  pub display_standard: String,
  /// Character code table number
  pub character_code_table: u8,
  /// Language code (ISO 639-2)
  pub language_code: String,
  /// Original program title
  pub original_program_title: String,
  /// Original episode title
  pub original_episode_title: String,
  /// Translated program title
  pub translated_program_title: String,
  /// Translated episode title
  pub translated_episode_title: String,
  /// Transmitter's name
  pub transmitter_name: String,
  /// Translator's contact information
  pub translator_contact: String,
  /// Subtitle list reference code
  pub subtitle_list_reference: String,
  /// Creation date
  pub creation_date: String,
  /// Revision date
  pub revision_date: String,
  /// Revision number
  pub revision_number: String,
  /// Total number of TTI blocks
  pub total_tti_blocks: u32,
  /// Total number of subtitles
  pub total_subtitles: u16,
  /// Timecode: Start of program
  pub timecode_start: String,
  /// Timecode: First in-cue
  pub timecode_first_incue: String,
  /// Total duration
  pub total_duration: String,
  /// Publisher
  pub publisher: String,
  /// Editor's name
  pub editor_name: String,
  /// Editor's contact
  pub editor_contact: String,
  /// Spare bytes
  pub spare_bytes: Vec<u8>,
  /// User defined area
  pub user_defined_area: Vec<u8>,
}

/// TTI (Text and Timing Information) Block
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TtiBlock {
  /// Subtitle group number
  pub subtitle_group: u8,
  /// Subtitle number
  pub subtitle_number: u16,
  /// Extension block number
  pub extension_block: u8,
  /// Cumulative status
  pub cumulative_status: u8,
  /// Time code: Start
  pub timecode_start: u32,
  /// Time code: End
  pub timecode_end: u32,
  /// Vertical position
  pub vertical_position: u8,
  /// Justification code
  pub justification: u8,
  /// Comment flag
  pub comment_flag: bool,
  /// Text field
  pub text: String,
}

impl EbuStlData {
  /// Create a new EbuStlData with default GSI block
  pub fn new() -> Self {
    EbuStlData {
      gsi: GsiBlock::default(),
      subtitles: Vec::new(),
      tti_blocks: Vec::new(),
    }
  }

  /// Parse EBU STL binary data
  pub fn parse(data: &[u8]) -> Result<Self, SubtitleError> {
    if data.len() < 1024 {
      return Err(SubtitleError::InvalidFormat {
        format: Format::EbuStl,
        reason: "File too short for EBU STL format".to_string(),
      });
    }

    // Parse GSI block (first 1024 bytes)
    let gsi = GsiBlock::parse(&data[0..1024])?;

    // Parse TTI blocks
    let tti_count = ((data.len() - 1024) / 128).min(1000); // STL max 1000 subtitles
    let mut tti_blocks = Vec::with_capacity(tti_count);
    let mut subtitles: Vec<Subtitle> = Vec::with_capacity(tti_count);
    let mut offset = 1024;

    while offset + 128 <= data.len() {
      let tti_data = &data[offset..offset + 128];
      if let Some(tti) = TtiBlock::parse(tti_data)? {
        tti_blocks.push(tti.clone());

        // Convert to Subtitle
        if !tti.text.is_empty() {
          let start_ms = tti.timecode_start as u64;
          let end_ms = tti.timecode_end as u64;
          subtitles.push(Subtitle::new(start_ms, end_ms, &tti.text));
        }
      }
      offset += 128;
    }

    Ok(EbuStlData {
      gsi,
      subtitles,
      tti_blocks,
    })
  }

  /// Serialize to EBU STL binary format
  pub fn to_bytes(&self) -> Vec<u8> {
    let mut data = Vec::new();

    // Write GSI block
    data.extend_from_slice(&self.gsi.to_bytes());

    // Write TTI blocks
    for tti in &self.tti_blocks {
      data.extend_from_slice(&tti.to_bytes());
    }

    data
  }
}

impl GsiBlock {
  /// Parse GSI block from binary data
  fn parse(data: &[u8]) -> Result<Self, SubtitleError> {
    // Simplified GSI parsing - in production, would parse all 1024 bytes
    let language_code = String::from_utf8_lossy(&data[14..17]).to_string();

    Ok(GsiBlock {
      code_page: data[0],
      disk_format_code: String::new(),
      display_standard: String::new(),
      character_code_table: 0,
      language_code,
      original_program_title: String::new(),
      original_episode_title: String::new(),
      translated_program_title: String::new(),
      translated_episode_title: String::new(),
      transmitter_name: String::new(),
      translator_contact: String::new(),
      subtitle_list_reference: String::new(),
      creation_date: String::new(),
      revision_date: String::new(),
      revision_number: String::new(),
      total_tti_blocks: 0,
      total_subtitles: 0,
      timecode_start: String::new(),
      timecode_first_incue: String::new(),
      total_duration: String::new(),
      publisher: String::new(),
      editor_name: String::new(),
      editor_contact: String::new(),
      spare_bytes: vec![0; 75],
      user_defined_area: vec![0; 376],
    })
  }

  /// Convert GSI block to binary
  fn to_bytes(&self) -> Vec<u8> {
    // Simplified GSI serialization - would write all 1024 bytes in production
    vec![0; 1024]
  }
}

impl TtiBlock {
  /// Parse TTI block from binary data
  fn parse(data: &[u8]) -> Result<Option<Self>, SubtitleError> {
    if data.len() < 128 {
      return Ok(None);
    }

    let subtitle_number = u16::from_be_bytes([data[1], data[2]]);

    // Skip invalid blocks
    if subtitle_number == 0xFFFF {
      return Ok(None);
    }

    // Parse timecode (SMPTE format)
    let timecode_start = parse_smpte_timecode(&data[3..7])?;
    let timecode_end = parse_smpte_timecode(&data[7..11])?;

    // Extract text field (112 bytes starting at offset 16)
    let text_bytes = &data[16..128];
    let text = decode_stl_text(text_bytes);

    Ok(Some(TtiBlock {
      subtitle_group: data[0],
      subtitle_number,
      extension_block: data[12],
      cumulative_status: data[13],
      timecode_start,
      timecode_end,
      vertical_position: data[14],
      justification: data[15],
      comment_flag: false,
      text,
    }))
  }

  /// Convert TTI block to binary
  fn to_bytes(&self) -> Vec<u8> {
    let mut data = vec![0; 128];

    data[0] = self.subtitle_group;
    data[1..3].copy_from_slice(&self.subtitle_number.to_be_bytes());

    // Write timecode
    encode_smpte_timecode(self.timecode_start, &mut data[3..7]);
    encode_smpte_timecode(self.timecode_end, &mut data[7..11]);

    data[12] = self.extension_block;
    data[13] = self.cumulative_status;
    data[14] = self.vertical_position;
    data[15] = self.justification;

    // Write text (simplified)
    let text_bytes = self.text.as_bytes();
    let len = text_bytes.len().min(112);
    data[16..16 + len].copy_from_slice(&text_bytes[..len]);

    data
  }
}

/// Parse SMPTE timecode from binary
fn parse_smpte_timecode(data: &[u8]) -> Result<u32, SubtitleError> {
  if data.len() < 4 {
    return Ok(0);
  }

  // SMPTE timecode: HH:MM:SS:FF
  let hours = data[0] as u32;
  let minutes = data[1] as u32;
  let seconds = data[2] as u32;
  let frames = data[3] as u32;

  // Convert to milliseconds (assuming 25 fps PAL)
  let total_frames = hours * 3600 * 25 + minutes * 60 * 25 + seconds * 25 + frames;
  let seconds_total = total_frames as f64 / 25.0;

  Ok((seconds_total * 1000.0).round() as u32)
}

/// Encode SMPTE timecode to binary
fn encode_smpte_timecode(timecode: u32, data: &mut [u8]) {
  // Convert milliseconds to timecode (25 fps PAL)
  let total_frames = (timecode as f64 / 1000.0 * 25.0).round() as u32;

  let frames = total_frames % 25;
  let seconds = (total_frames / 25) % 60;
  let minutes = (total_frames / (25 * 60)) % 60;
  let hours = total_frames / (25 * 3600);

  data[0] = hours as u8;
  data[1] = minutes as u8;
  data[2] = seconds as u8;
  data[3] = frames as u8;
}

/// Decode STL text field
fn decode_stl_text(data: &[u8]) -> String {
  // Simplified text decoding - remove control codes
  let mut text = String::new();

  for byte in data {
    if *byte >= 0x20 && *byte < 0x7F {
      text.push(*byte as char);
    } else if *byte == 0x8F {
      // Italic start
      text.push('<');
      text.push('i');
      text.push('>');
    } else if *byte == 0x90 {
      // Italic end
      text.push('<');
      text.push('/');
      text.push('i');
      text.push('>');
    }
  }

  text.trim().to_string()
}

/// Parse EBU STL from file content
pub fn parse_content(data: &[u8]) -> AnyResult<SubtitleFile> {
  let stl_data = EbuStlData::parse(data)?;
  Ok(SubtitleFile::EbuStl(Box::new(stl_data)))
}

/// Parse EBU STL from bytes
pub fn parse_bytes(data: &[u8]) -> AnyResult<SubtitleFile> {
  parse_content(data)
}

/// Parse EBU STL file asynchronously
#[cfg(not(target_arch = "wasm32"))]
pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<SubtitleFile> {
  let data = tokio::fs::read(path).await?;
  parse_content(&data)
}

/// Parse EBU STL from URL (requires `http` feature)
#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<SubtitleFile> {
  let response = reqwest::get(url).await?;
  let data = response.bytes().await?;
  parse_content(&data)
}

/// Detect if data looks like EBU STL.
///
/// EBU STL files are 1024-byte GSI header + N×128-byte TTI blocks.
/// The first byte is the code page number (0–31); the second byte
/// is the disk format code (typically STL25.01 / STL30.01);
/// character bytes at offset 3+ carry subtitle count metadata.
pub fn detect_format(data: &[u8]) -> Option<crate::model::Format> {
  if data.len() >= 1024 && (data.len() - 1024) % 128 == 0 {
    let tti_count = u16::from_be_bytes([data[1], data[2]]) as usize;
    let expected_tti = (data.len() - 1024) / 128;
    let code_page = data[0];
    if code_page < 32 && tti_count > 0 && tti_count == expected_tti {
      return Some(crate::model::Format::EbuStl);
    }
  }
  None
}

/// Serialize subtitles to EBU STL format
pub fn to_string(subtitles: &[Subtitle]) -> Vec<u8> {
  let gsi = GsiBlock::default();

  let tti_blocks: Vec<TtiBlock> = subtitles
    .iter()
    .enumerate()
    .map(|(i, sub)| {
      let start_timecode = (sub.start / 40) as u32; // Simplified conversion
      let end_timecode = (sub.end / 40) as u32;

      TtiBlock {
        subtitle_group: 0,
        subtitle_number: (i + 1) as u16,
        extension_block: 0,
        cumulative_status: 0,
        timecode_start: start_timecode,
        timecode_end: end_timecode,
        vertical_position: 20,
        justification: 0,
        comment_flag: false,
        text: sub.text.clone(),
      }
    })
    .collect();

  let stl_data = EbuStlData {
    gsi,
    subtitles: subtitles.to_vec(),
    tti_blocks,
  };

  stl_data.to_bytes()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_timecode_conversion() {
    let ms = 10000u64; // 10 seconds
    let frames = (ms as f64 / 1000.0 * 25.0).round() as u32;
    assert_eq!(frames, 250);

    // Test parse and encode round-trip
    let mut data = [0u8; 4];
    encode_smpte_timecode(10000, &mut data);
    let parsed = parse_smpte_timecode(&data).unwrap();
    assert!(parsed > 0);
  }

  #[test]
  fn test_detect() {
    let mut data = vec![0u8; 1152];
    data[0] = 5;
    data[2] = 1;
    assert!(detect_format(&data).is_some());

    // Invalid structure
    let data = vec![0u8; 100];
    assert!(detect_format(&data).is_none());
  }
}
