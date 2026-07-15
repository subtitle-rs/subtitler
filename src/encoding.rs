use crate::types::AnyResult;
use anyhow::anyhow;

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

pub fn decode_to_string(data: &[u8]) -> AnyResult<String> {
  let encoding = detect_encoding(data);

  match encoding {
    "UTF-8" | "UTF-8-BOM" => {
      let text = String::from_utf8(data.to_vec()).map_err(|e| anyhow!("Invalid UTF-8: {}", e))?;
      Ok(text.trim_start_matches('\u{FEFF}').to_string())
    }
    "UTF-16BE" => {
      let u16: Vec<u16> = data
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
      String::from_utf16(&u16).map_err(|e| anyhow!("Invalid UTF-16BE: {:?}", e))
    }
    "UTF-16LE" => {
      let u16: Vec<u16> = data
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
      String::from_utf16(&u16).map_err(|e| anyhow!("Invalid UTF-16LE: {:?}", e))
    }
    _ => {
      let text = String::from_utf8(data.to_vec()).map_err(|_| {
        anyhow!(
          "Cannot decode encoding '{}'. Try converting to UTF-8 first.",
          encoding
        )
      })?;
      Ok(text)
    }
  }
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
}
