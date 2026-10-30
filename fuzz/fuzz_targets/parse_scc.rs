#![no_main]
//! SCC (CEA-608) parser: drop-frame timecode arithmetic and the
//! hex-pair caption decoder.
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
  let text = String::from_utf8_lossy(data);
  let _ = subtitler::scc::parse_content(&text);
});
