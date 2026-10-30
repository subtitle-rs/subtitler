#![allow(unused_imports)]
//! Property-based tests for subtitle round-trips and invariants.
//!
//! Run: PROPTEST_CASES=100 cargo test --test proptest
//!
//! Gated behind cfg(not(wasm32)) because proptest's transitive
//! dependencies (wait-timeout) do not support wasm32-unknown-unknown.

#![cfg(not(target_arch = "wasm32"))]

use proptest::prelude::*;
use subtitler::model::Subtitle;
use subtitler::model::SubtitleFile;
use subtitler::model::SubtitleFormat;

#[cfg(any(feature = "srt", feature = "vtt", feature = "ass"))]
fn arb_subtitle() -> impl Strategy<Value = Subtitle> {
  (
    0u64..3_600_000u64,
    0u64..3_600_000u64,
    // Text: expanded over successive versions — ASCII printable minus
    // & < > (tag markers: SRT/VTT parse normalizes them into text_parts,
    // so exact text round-trip only holds for tag-free input), Latin-1+,
    // CJK, Hiragana/Katakana, Hangul, combining diacritics, Hebrew (RTL
    // stress) and emoji. \n \r are outside \x20.. already.
    "[\x20-\x25\x27-\x3B\x3D\x3F-\x7E\u{80}-\u{FF}\u{300}-\u{36F}\u{4E00}-\u{9FFF}\u{3040}-\u{30FF}\u{AC00}-\u{D7A3}\u{5D0}-\u{5EA}\u{1F300}-\u{1F64F}]{0,60}",
  )
    .prop_map(|(start, end, text)| {
      let (s, e) = if start <= end {
        (start, end)
      } else {
        (end, start)
      };
      Subtitle::new(s, e, &text)
    })
}
#[cfg(feature = "srt")]
#[allow(dead_code)]
fn arb_subtitle_file(n: usize) -> impl Strategy<Value = SubtitleFile> {
  proptest::collection::vec(arb_subtitle(), n..=n).prop_map(SubtitleFile::Srt)
}

proptest! {
  #[cfg(feature = "srt")]
  #[test]
  fn srt_round_trip_preserves_text_and_times(sub in arb_subtitle()) {
    let s = subtitler::srt::to_string(std::slice::from_ref(&sub));
    let parsed = subtitler::srt::parse_content(&s).unwrap();
    prop_assert_eq!(parsed.subtitles().len(), 1);
    prop_assert_eq!(parsed.subtitles()[0].start, sub.start);
    prop_assert_eq!(parsed.subtitles()[0].end, sub.end);
    prop_assert_eq!(parsed.subtitles()[0].text.trim(), sub.text.trim());
  }

  #[cfg(feature = "vtt")]
  #[test]
  fn vtt_round_trip_preserves_text_and_times(sub in arb_subtitle()) {
    let s = subtitler::vtt::to_string(std::slice::from_ref(&sub), None);
    let parsed = subtitler::vtt::parse_content(&s).unwrap();
    prop_assert_eq!(parsed.subtitles().len(), 1);
    prop_assert_eq!(parsed.subtitles()[0].start, sub.start);
    prop_assert_eq!(parsed.subtitles()[0].end, sub.end);
    prop_assert_eq!(parsed.subtitles()[0].text.trim(), sub.text.trim());
  }

  #[cfg(feature = "ass")]
  #[test]
  fn ass_round_trip_preserves_count(sub in arb_subtitle()) {
    let s = subtitler::ass::to_string(
      &std::collections::HashMap::new(),
      &[subtitler::model::AssStyle::default_style()],
      std::slice::from_ref(&sub),
      &[],
    );
    let parsed = subtitler::ass::parse_content(&s).unwrap();
    prop_assert_eq!(parsed.subtitles().len(), 1);
  }

  // ── Idempotency properties ──

  #[cfg(feature = "srt")]
  #[test]
  fn sort_is_idempotent(file in arb_subtitle_file(10)) {
    let mut a = file.clone();
    a.sort();
    let mut b = a.clone();
    b.sort();
    prop_assert_eq!(a.subtitles().len(), b.subtitles().len());
    for (x, y) in a.subtitles().iter().zip(b.subtitles().iter()) {
      prop_assert_eq!(x.start, y.start);
      prop_assert_eq!(x.end, y.end);
    }
  }

  #[cfg(any(feature = "srt", feature = "vtt", feature = "ass"))]
  #[test]
  fn shift_all_is_reversible(sub in arb_subtitle()) {
    let mut a = sub.clone();
    a.shift(3000);
    a.shift(-3000);
    prop_assert_eq!(a.start, sub.start);
    prop_assert_eq!(a.end, sub.end);
  }

  #[cfg(feature = "srt")]
  #[test]
  fn merge_adjacent_never_increases_count(file in arb_subtitle_file(8)) {
    let mut m = file.clone();
    m.sort();
    m.merge_adjacent(1000);
    prop_assert!(m.subtitles().len() <= file.subtitles().len(),
      "merge_adjacent should not increase cue count: {} -> {}",
      file.subtitles().len(), m.subtitles().len()
    );
  }
}
