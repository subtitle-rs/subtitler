//! Regression test for EBU STL binary input handling.
#![cfg(not(target_arch = "wasm32"))]
//!
//! Background: `parse_to_file` and `cmd_parse` (in `src/main.rs`) used
//! to run `encoding::decode_to_string(data)?` before dispatching to
//! format parsers. For EBU STL (binary), this could either fail with
//! InvalidEncoding OR silently produce garbage text (chardetng is
//! permissive). The fix hoists the EbuStl branch above the decode call.
//!
//! `parse_to_file`/`cmd_parse` are private to the binary crate, so we
//! can't unit-test them directly. Instead we verify the contract they
//! rely on: `ebu_stl::parse_content` accepts raw bytes WITHOUT going
//! through text decoding. If this stops being true, the CLI fix would
//! silently break.

use subtitler::ebu_stl;
use subtitler::model::SubtitleFormat;

/// A minimal valid EBU STL file: 1024-byte GSI + 1×128-byte TTI.
/// Bytes are 0xFF-heavy to mimic real binary STL content (GSI block
/// contains packed binary fields, not text).
fn build_minimal_stl() -> Vec<u8> {
  let mut data = vec![0u8; 1024 + 128]; // GSI + 1 TTI
  // GSI byte 0 = code page (<32 for detect_format to match)
  data[0] = 4;
  // Bytes 1-2 (big-endian u16) = expected TTI count = 1
  data[1] = 0;
  data[2] = 1;
  // Fill the rest of the GSI with high bytes (packed binary content)
  data[3..1024].fill(0xFF);
  data
}

#[test]
fn test_ebu_stl_parse_content_accepts_binary_without_decoding() {
  // Contract: ebu_stl::parse_content accepts raw bytes directly.
  // The CLI's EbuStl branch must call this WITHOUT prior text decoding.
  let data = build_minimal_stl();
  let file = ebu_stl::parse_content(&data).expect(
    "ebu_stl::parse_content must accept raw binary bytes; \
     if this fails, the CLI's EbuStl branch would also fail",
  );
  // Sanity: the format detection agrees this is EbuStl.
  assert_eq!(file.format(), subtitler::model::Format::EbuStl);
}

#[test]
fn test_ebu_stl_detect_format_matches_binary_signature() {
  // Contract: detect_format identifies the binary signature without
  // needing text decoding. This is what CLI dispatch relies on.
  let data = build_minimal_stl();
  assert_eq!(
    subtitler::detect_format(&data),
    Some(subtitler::model::Format::EbuStl),
    "detect_format must identify binary EBU STL without text decoding"
  );
}
