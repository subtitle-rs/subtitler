//! Stream-parse a large SRT file without loading all subtitles into memory.
//!
//! Run: cargo run --example stream-parse

use subtitler::srt;

#[tokio::main(flavor = "current_thread")]
async fn main() -> subtitler::types::AnyResult<()> {
  let content = "1\n00:00:01,000 --> 00:00:03,500\nFirst subtitle\n\n\
                 2\n00:00:04,000 --> 00:00:06,500\nSecond subtitle\n\n\
                 3\n00:00:07,000 --> 00:00:09,000\nThird subtitle\n";

  println!("Streaming parse (no Vec allocation):\n");

  let mut count = 0;
  for sub in srt::parse_stream(content) {
    let sub = sub?;
    count += 1;
    println!("  #{}: [{}-{}ms] {}", count, sub.start, sub.end, sub.text);
  }

  println!("\nProcessed {} subtitles via stream iterator.", count);
  Ok(())
}
