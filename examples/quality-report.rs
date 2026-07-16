//! Generate a JSON quality report for a subtitle file.
//!
//! Run: cargo run --example quality-report

use subtitler::SubtitleFormat;
use subtitler::quality::generate_report;

#[tokio::main(flavor = "current_thread")]
async fn main() -> subtitler::types::AnyResult<()> {
  let data = tokio::fs::read("examples/example.srt").await?;
  let file = subtitler::parse_bytes(&data)?;
  let subs = file.subtitles();

  let report = generate_report(subs, 42, 5000, 25.0);

  println!("=== Quality Report ===");
  println!("Total subtitles:  {}", report.total_subtitles);
  println!("Total issues:     {}", report.total_issues);
  println!("Avg duration:     {}ms", report.avg_duration_ms);
  println!("Avg CPS:          {:.1}", report.avg_cps);
  println!("Avg WPM:          {:.1}", report.avg_wpm);

  // Find worst subtitle
  if let Some(worst) = report.subtitles.iter().max_by_key(|s| s.issues.len()) {
    if !worst.issues.is_empty() {
      println!(
        "\nWorst subtitle: #{} ({} issues, CPS {:.1})",
        worst.index + 1,
        worst.issues.len(),
        worst.chars_per_second
      );
    }
  }

  // Export as JSON
  let json = serde_json::to_string_pretty(&report)?;
  println!("\n=== JSON Output (first 500 chars) ===");
  println!("{}", &json[..json.len().min(500)]);

  Ok(())
}
