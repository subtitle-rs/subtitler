#![no_main]
//! ASS/SSA parser: field-map driven style/dialogue parsing, override
//! tag scanning and the [Fonts] uuencode decoder.
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
  let text = String::from_utf8_lossy(data);
  let _ = subtitler::ass::parse_content(&text);
});
