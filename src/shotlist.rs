//! Shot-change list parsing from CMX3600 EDL files.
//!
//! A "shot list" EDL (e.g. DaVinci Resolve timeline cut markers, AI scene
//! cut detectors, hand-authored cut lists) describes one shot per event.
//! [`parse_edl_cuts`] extracts the record-in/out timecodes of every event
//! and returns the sorted, deduplicated cut points in milliseconds — the
//! input for [`SubtitleFormat::apply_shot_changes`] (Netflix-style rules:
//! a subtitle must end at least N frames before a cut and start at least
//! M frames after one).
//!
//! Only EDL reading is supported by design: general NLE round-trip stays
//! out of scope (roadmap YAGNI boundary), and timecodes need the
//! project's frame rate passed in — EDL files do not declare it.
//!
//! ```text
//! TITLE: SCENE CUTS
//! FCM: NON-DROP FRAME
//!
//! 001  AX       V     C        00:00:01:04 00:00:05:20 00:00:01:04 00:00:05:20
//! * FROM CLIP NAME: shot1.mov
//!
//! 002  AX       V     C        00:00:05:20 00:00:09:12 00:00:05:20 00:00:09:12
//! ```

use crate::model::convert::frames_to_ms;

/// Parse a CMX3600 EDL into sorted, deduplicated cut points (ms).
///
/// Per event line the record (timeline) in/out timecodes are used — the
/// 3rd/4th timecode pair when four are present, otherwise the 1st/2nd.
/// Comment (`*`), header (`TITLE:`/`FCM:`), and blank lines are skipped.
fn parse_edl_cuts_str(edl: &str, fps: f64) -> Vec<u64> {
  let mut cuts: Vec<u64> = Vec::new();
  for line in edl.lines() {
    let fields: Vec<&str> = line.split_whitespace().collect();
    // Event line: num reel chan trans [src_in src_out] rec_in rec_out.
    // Header/comment lines ("TITLE:", "FCM:", "*", "BL") don't match.
    if fields.len() < 6 || !fields[0].chars().all(|c| c.is_ascii_digit()) {
      continue;
    }
    let tcs: Vec<&str> = fields.iter().filter(|f| is_timecode(f)).copied().collect();
    let (rec_in, rec_out) = match tcs.len() {
      4 => (tcs[2], tcs[3]),
      2 => (tcs[0], tcs[1]),
      _ => continue,
    };
    if let (Some(a), Some(b)) = (parse_timecode(rec_in, fps), parse_timecode(rec_out, fps)) {
      cuts.push(a);
      cuts.push(b);
    }
  }
  cuts.sort_unstable();
  cuts.dedup();
  cuts
}

/// Parse EDL bytes — auto-detect encoding then parse.
pub fn parse_edl_cuts(data: &[u8], fps: f64) -> crate::types::AnyResult<Vec<u64>> {
  let text = crate::encoding::decode_to_string(data)?;
  Ok(parse_edl_cuts_str(&text, fps))
}

fn is_timecode(s: &str) -> bool {
  let b = s.as_bytes();
  b.len() == 11
    && (b[2] == b':' && b[5] == b':' && (b[8] == b':' || b[8] == b';'))
    && b
      .iter()
      .enumerate()
      .all(|(i, &c)| [2usize, 5, 8].contains(&i) || c.is_ascii_digit())
}

fn parse_timecode(tc: &str, fps: f64) -> Option<u64> {
  let mut parts = tc.trim().split([':', ';']);
  let h: u64 = parts.next()?.parse().ok()?;
  let m: u64 = parts.next()?.parse().ok()?;
  let s: u64 = parts.next()?.parse().ok()?;
  let f: u64 = parts.next()?.parse().ok()?;
  if parts.next().is_some() {
    return None;
  }
  let total_frames = ((h * 3600 + m * 60 + s) as f64 * fps).round() as u64 + f;
  Some(frames_to_ms(total_frames, fps))
}

#[cfg(test)]
mod tests {
  use super::*;

  const SAMPLE: &str = "TITLE: SCENE CUTS
FCM: NON-DROP FRAME

001  AX       V     C        00:00:01:04 00:00:05:00 00:00:01:04 00:00:05:00
* FROM CLIP NAME: shot1.mov

002  AX       V     C        00:00:05:00 00:00:09:00 00:00:05:00 00:00:09:00
* FROM CLIP NAME: shot2.mov
";

  #[test]
  fn test_parse_edl_cuts_dedupes_and_sorts() {
    // 25 fps: 00:00:01:04 = 29 frames = 1160 ms; 00:00:05:00 = 125 frames
    // = 5000 ms; 00:00:09:00 = 225 frames = 9000 ms.
    let cuts = parse_edl_cuts_str(SAMPLE, 25.0);
    assert_eq!(cuts, vec![1_160, 5_000, 9_000]);
    // Shared boundary (00:00:05:00 appears as out and in) is deduplicated.
  }

  #[test]
  fn test_parse_edl_cuts_six_field_events() {
    // Some simplified cut lists carry only the record pair.
    let edl = "001  AX  V  C  00:00:00:00 00:00:04:00\n002  AX  V  C  00:00:04:00 00:00:08:00\n";
    let cuts = parse_edl_cuts_str(edl, 25.0);
    assert_eq!(cuts, vec![0, 4_000, 8_000]);
  }

  #[test]
  fn test_parse_edl_ignores_comments_and_headers() {
    let edl = "TITLE: X\nFCM: NON-DROP FRAME\n* FROM CLIP NAME: 00:00:01:04\n";
    assert!(parse_edl_cuts_str(edl, 25.0).is_empty());
  }

  #[test]
  fn test_parse_edl_bytes() {
    let cuts = parse_edl_cuts(SAMPLE.as_bytes(), 25.0).unwrap();
    assert_eq!(cuts.len(), 3);
  }
}
