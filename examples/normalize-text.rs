//! Text normalization: OCR fix, hearing-impaired removal, line break optimization.
//!
//! Run: cargo run --example normalize-text

use subtitler::normalize;

fn main() {
  println!("=== OCR Error Fix ===");
  let ocr = "12O456 and 1l0 with w0rd";
  println!("  Input:  {}", ocr);
  println!("  Output: {}", normalize::fix_ocr_errors(ocr));

  println!("\n=== Hearing-Impaired Tag Removal ===");
  let hi = "(LAUGHS) Hello [APPLAUSE] world ♪ Music ♪";
  println!("  Input:  {}", hi);
  println!("  Output: {}", normalize::strip_hearing_impaired(hi));

  println!("\n=== Quote Normalization ===");
  let quotes = "\u{201C}smart quotes\u{201D} and \u{2018}single\u{2019}";
  println!("  Input:  {}", quotes);
  println!("  Output: {}", normalize::normalize_quotes(quotes));

  println!("\n=== Whitespace Normalization ===");
  let ws = "hello    world\n\n\n\nbye";
  println!("  Input:  {:?}", ws);
  println!("  Output: {:?}", normalize::normalize_whitespace(&ws));

  println!("\n=== Punctuation Fix ===");
  let punct = "hello , world ! what ???";
  println!("  Input:  {}", punct);
  println!("  Output: {}", normalize::normalize_punctuation(&punct));

  println!("\n=== Smart Line Breaking ===");
  let long = "This is a very long subtitle line that should be broken at natural word boundaries";
  println!("  Input:  {}", long);
  println!("  Output: {:?}", normalize::optimize_line_breaks(long, 42));

  println!("\n=== Full Pipeline (normalize_text) ===");
  let dirty = "Hello   \u{201C}world\u{201D} !  (LAUGHS)";
  println!("  Input:  {}", dirty);
  println!("  Output: {}", normalize::normalize_text(&dirty));
}
