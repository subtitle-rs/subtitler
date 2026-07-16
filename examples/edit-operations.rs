//! Edit operations: shift, sort, merge, split, enforce duration.
//!
//! Run: cargo run --example edit-operations

use subtitler::SubtitleFormat;
use subtitler::model::{Subtitle, SubtitleFile};

fn sample() -> SubtitleFile {
  SubtitleFile::Srt(vec![
    Subtitle::new(5000, 7000, "C (out of order)"),
    Subtitle::new(1000, 3000, "A (first)"),
    Subtitle::new(3100, 5000, "B (close to A)"),
    Subtitle::new(8000, 8500, "D (too short)"),
    Subtitle::new(
      9000,
      9000 + 6000,
      "This is a very long subtitle that exceeds the recommended character limit for a single line",
    ),
  ])
}

fn show(label: &str, file: &SubtitleFile) {
  let subs = file.subtitles();
  println!("  {} ({} subs):", label, subs.len());
  for (i, s) in subs.iter().enumerate() {
    println!(
      "    [{}] {}-{}ms '{}'",
      i,
      s.start,
      s.end,
      if s.text.len() > 40 {
        &s.text[..40]
      } else {
        &s.text
      }
    );
  }
}

fn main() {
  println!("=== Sort ===");
  let mut f = sample();
  show("Before", &f);
  f.sort();
  show("After ", &f);

  println!("\n=== Shift +1000ms ===");
  let mut f = sample();
  f.sort();
  f.shift_all(1000);
  show("After", &f);

  println!("\n=== Merge adjacent (gap <= 200ms) ===");
  let mut f = sample();
  f.sort();
  f.merge_adjacent(200);
  show("After", &f);

  println!("\n=== Enforce min duration (2000ms) ===");
  let mut f = sample();
  f.sort();
  f.enforce_min_duration(2000);
  show("After", &f);

  println!("\n=== Split long (>20 chars) ===");
  let mut f = sample();
  f.sort();
  f.split_long(20);
  show("After", &f);

  println!("\n=== Remove overlaps ===");
  let mut f = SubtitleFile::Srt(vec![
    Subtitle::new(1000, 5000, "overlapping A"),
    Subtitle::new(3000, 6000, "overlapping B"),
  ]);
  show("Before", &f);
  f.remove_overlaps();
  show("After ", &f);

  println!("\n=== Auto-extend for CPS (max 15) ===");
  let mut f = SubtitleFile::Srt(vec![Subtitle::new(
    0,
    1000,
    "This text is way too long for one second",
  )]);
  show("Before", &f);
  f.auto_extend_for_cps(15.0);
  show("After ", &f);
}
