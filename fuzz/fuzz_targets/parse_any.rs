#![no_main]
//! Auto-detect + parse: exercises the encoding detection, format
//! signature matching and every format parser behind one entry point.
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
  let _ = subtitler::parse_bytes(data);
});
