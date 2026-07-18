use crate::error::SubtitleError;

pub fn detect_encoding(data: &[u8]) -> &'static str {
  if data.len() >= 3 && data[0] == 0xEF && data[1] == 0xBB && data[2] == 0xBF {
    return "UTF-8-BOM";
  }
  if data.len() >= 2 && data[0] == 0xFE && data[1] == 0xFF {
    return "UTF-16BE";
  }
  if data.len() >= 2 && data[0] == 0xFF && data[1] == 0xFE {
    return "UTF-16LE";
  }
  if std::str::from_utf8(data).is_ok() {
    return "UTF-8";
  }
  let mut detector = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Allow);
  detector.feed(data, true);
  let encoding = detector.guess(None, chardetng::Utf8Detection::Allow);
  encoding.name()
}

pub fn decode_to_string(data: &[u8]) -> Result<String, SubtitleError> {
  // A single byte that is a UTF-16 BOM prefix (FE or FF) is a truncated
  // BOM — there is no meaningful content to decode. Return empty rather
  // than letting chardetng fabricate a character (e.g. "ÿ" for 0xFF).
  if data.len() == 1 && (data[0] == 0xFE || data[0] == 0xFF) {
    return Ok(String::new());
  }

  let encoding = detect_encoding(data);

  match encoding {
    "UTF-8" | "UTF-8-BOM" => {
      let text = String::from_utf8(data.to_vec()).map_err(|e| SubtitleError::InvalidEncoding {
        encoding: encoding.to_string(),
        error: e.to_string(),
      })?;
      Ok(text.trim_start_matches('\u{FEFF}').to_string())
    }
    "UTF-16BE" | "UTF-16LE" => {
      // Guard against too-short inputs (BOM is 2 bytes)
      if data.len() < 2 {
        return Ok(String::new());
      }
      // Skip the 2-byte BOM (FF FE for LE, FE FF for BE) before decoding.
      // detect_encoding already confirmed the BOM bytes match.
      let body = &data[2..];
      let u16: Vec<u16> = body
        .chunks_exact(2)
        .map(|c| {
          if encoding == "UTF-16BE" {
            u16::from_be_bytes([c[0], c[1]])
          } else {
            u16::from_le_bytes([c[0], c[1]])
          }
        })
        .collect();
      // Note: any trailing odd byte after body[2..] is silently dropped
      // by chunks_exact(2). This is intentional — UTF-16 is strictly
      // 2-byte aligned; a trailing odd byte indicates a corrupt file.
      let s = String::from_utf16(&u16).map_err(|e| SubtitleError::InvalidEncoding {
        encoding: encoding.to_string(),
        error: format!("{:?}", e),
      })?;
      // Double-safety: trim a leading U+FEFF in case the BOM bytes were
      // decoded as a character (shouldn't happen after skipping, but
      // matches the UTF-8 path's behavior).
      Ok(s.trim_start_matches('\u{FEFF}').to_string())
    }
    _ => {
      let label = encoding.as_bytes();
      if let Some(enc) = encoding_rs::Encoding::for_label_no_replacement(label) {
        let (cow, _enc, had_errors) = enc.decode(data);
        if had_errors {
          tracing::warn!(encoding = %encoding, "subtitle decoding encountered byte errors");
        }
        Ok(cow.into_owned())
      } else {
        Err(SubtitleError::UnsupportedEncoding {
          encoding: encoding.to_string(),
        })
      }
    }
  }
}

/// Try to decode bytes for format detection (returns None on failure).
/// Unlike `decode_to_string`, this never returns an Err — useful in
/// `detect_format` functions where a failed decode just means "not this format".
pub fn try_decode_for_detection(data: &[u8]) -> Option<String> {
  if let Ok(s) = std::str::from_utf8(data) {
    return Some(s.to_string());
  }
  let mut detector = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Allow);
  detector.feed(data, true);
  let enc = detector.guess(None, chardetng::Utf8Detection::Allow);
  let enc = encoding_rs::Encoding::for_label_no_replacement(enc.name().as_bytes())?;
  let (cow, _, _) = enc.decode(data);
  Some(cow.into_owned())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_detect_utf8() {
    assert_eq!(detect_encoding(b"hello"), "UTF-8");
  }

  #[test]
  fn test_detect_utf8_bom() {
    assert_eq!(detect_encoding(b"\xEF\xBB\xBFhello"), "UTF-8-BOM");
  }

  #[test]
  fn test_detect_utf16be() {
    assert_eq!(detect_encoding(b"\xFE\xFF\x00h"), "UTF-16BE");
  }

  #[test]
  fn test_detect_utf16le() {
    assert_eq!(detect_encoding(b"\xFF\xFEh\x00"), "UTF-16LE");
  }

  #[test]
  fn test_decode_utf8_bom() {
    let result = decode_to_string(b"\xEF\xBB\xBFhello").unwrap();
    assert_eq!(result, "hello");
  }

  #[test]
  fn test_decode_utf8() {
    let result = decode_to_string(b"hello world").unwrap();
    assert_eq!(result, "hello world");
  }

  #[test]
  fn test_decode_utf16be_bom_stripped() {
    // UTF-16BE BOM (FE FF) + 'h' (00 68) + 'i' (00 69)
    let input = b"\xFE\xFF\x00\x68\x00\x69";
    let result = decode_to_string(input).unwrap();
    assert!(
      !result.starts_with('\u{FEFF}'),
      "UTF-16BE BOM not stripped: got {:?}",
      result
    );
    assert_eq!(result, "hi");
  }

  #[test]
  fn test_decode_utf16le_bom_stripped() {
    // UTF-16LE BOM (FF FE) + 'h' (68 00) + 'i' (69 00)
    let input = b"\xFF\xFE\x68\x00\x69\x00";
    let result = decode_to_string(input).unwrap();
    assert!(
      !result.starts_with('\u{FEFF}'),
      "UTF-16LE BOM not stripped: got {:?}",
      result
    );
    assert_eq!(result, "hi");
  }

  #[test]
  fn test_decode_utf16_odd_byte_ignored() {
    // UTF-16LE BOM (FF FE) + 'h' (68 00) + 'i' (69 00) + 1 trailing byte (garbage)
    let input = b"\xFF\xFE\x68\x00\x69\x00\xAB";
    let result = decode_to_string(input).unwrap();
    // Must not panic; trailing odd byte is dropped by chunks_exact(2)
    assert_eq!(result, "hi");
  }

  #[test]
  fn test_decode_utf16_empty_and_short_input() {
    // 0-byte input → empty string, no panic
    assert_eq!(decode_to_string(b"").unwrap(), "");
    // 1-byte input (just BOM first byte) → empty string, no panic
    assert_eq!(decode_to_string(b"\xFF").unwrap(), "");
  }

  #[test]
  fn test_decode_shift_jis() {
    // "こんにちは" in Shift_JIS (no BOM)
    let data: &[u8] = &[0x82, 0xB1, 0x82, 0xF1, 0x82, 0xC9, 0x82, 0xBF, 0x82, 0xCD];
    let result = decode_to_string(data).unwrap();
    assert_eq!(result, "こんにちは");
  }

  #[test]
  fn test_decode_gbk() {
    // "简体中文测试" in GBK (no BOM, 12 bytes for reliable detection)
    let data: &[u8] = &[
      0xBC, 0xF2, 0xCC, 0xE5, 0xD6, 0xD0, 0xCE, 0xC4, 0xB2, 0xE2, 0xCA, 0xD4,
    ];
    let result = decode_to_string(data).unwrap();
    // chardetng best-effort: verify non-empty, non-garbage CJK output
    assert!(!result.is_empty(), "GBK decode should produce output");
    assert!(
      result.chars().any(|c| c as u32 > 0x7F),
      "GBK decode should contain non-ASCII CJK chars, got: {}",
      result
    );
  }
}
