//! Property-based tests for subtitle round-trips.
//!
//! Run: PROPTEST_CASES=100 cargo test --test proptest

use proptest::prelude::*;
use subtitler::model::Subtitle;
use subtitler::model::SubtitleFormat;

fn arb_subtitle() -> impl Strategy<Value = Subtitle> {
  (
    0u64..3_600_000u64,
    0u64..3_600_000u64,
    "[a-zA-Z][a-zA-Z0-9!?]{0,50}",
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

proptest! {
  #[test]
  fn srt_round_trip_preserves_text_and_times(sub in arb_subtitle()) {
    let s = subtitler::srt::to_string(std::slice::from_ref(&sub));
    let parsed = subtitler::srt::parse_content(&s).unwrap();
    prop_assert_eq!(parsed.subtitles().len(), 1, "should produce 1 cue");
    prop_assert_eq!(parsed.subtitles()[0].start, sub.start, "start mismatch");
    prop_assert_eq!(parsed.subtitles()[0].end, sub.end, "end mismatch");
    prop_assert_eq!(parsed.subtitles()[0].text.trim(), sub.text.trim(), "text mismatch");
  }

  #[test]
  fn vtt_round_trip_preserves_text_and_times(sub in arb_subtitle()) {
    let s = subtitler::vtt::to_string(std::slice::from_ref(&sub), None);
    let parsed = subtitler::vtt::parse_content(&s).unwrap();
    prop_assert_eq!(parsed.subtitles().len(), 1);
    prop_assert_eq!(parsed.subtitles()[0].start, sub.start);
    prop_assert_eq!(parsed.subtitles()[0].end, sub.end);
    prop_assert_eq!(parsed.subtitles()[0].text.trim(), sub.text.trim(), "text mismatch");
  }
}
