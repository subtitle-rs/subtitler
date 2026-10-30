#![no_main]
//! TTML/DFXP/iTT XML parser: the quick-xml event loop, style inheritance
//! resolution and layout region handling.
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
  let text = String::from_utf8_lossy(data);
  let _ = subtitler::ttml::parse_content(&text);
});
