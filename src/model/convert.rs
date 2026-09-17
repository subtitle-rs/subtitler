/// Milliseconds per hour (3,600,000).
pub const MS_PER_HOUR: u64 = 3_600_000;
/// Milliseconds per minute (60,000).
pub const MS_PER_MINUTE: u64 = 60_000;
/// Milliseconds per second (1,000).
pub const MS_PER_SECOND: u64 = 1_000;
/// Default tail duration for formats without explicit end times (3 seconds).
pub const DEFAULT_TAIL_MS: u64 = 3_000;

pub fn parse_ass_color(color: &str) -> (u8, u8, u8, u8) {
  let hex = color.trim_start_matches("&H").trim_start_matches("&h");
  let parsed = u32::from_str_radix(hex, 16).unwrap_or(0x00FFFFFF);
  let b = (parsed >> 16 & 0xFF) as u8;
  let g = (parsed >> 8 & 0xFF) as u8;
  let r = (parsed & 0xFF) as u8;
  let a = (parsed >> 24 & 0xFF) as u8;
  (r, g, b, a)
}

pub fn format_ass_color(r: u8, g: u8, b: u8, a: u8) -> String {
  let value = ((a as u32) << 24) | ((b as u32) << 16) | ((g as u32) << 8) | (r as u32);
  format!("&H{:08X}", value)
}

pub fn ms_to_frames(ms: u64, fps: f64) -> u64 {
  ((ms as f64) * fps / 1000.0).round() as u64
}

pub fn frames_to_ms(frames: u64, fps: f64) -> u64 {
  ((frames as f64) * 1000.0 / fps).round() as u64
}

/// A timecode timebase: wall-clock rate plus drop-frame numbering.
///
/// The DF variants follow SMPTE 12M-1-2014 §3.3: frame numbers are counted
/// at the nominal rate (30 / 60) and skip 2 (30) or 4 (60) frames at every
/// minute except minutes divisible by ten, so that one displayed minute
/// matches one wall-clock minute on average. Millisecond conversions use
/// the quoted rates (29.97 / 59.94), matching `scc.rs` and the rest of the
/// crate.
///
/// Note: at drop minutes the displays `;00` / `;01` (or `;00`–`;03` at 60)
/// do not exist — e.g. `00:00:59;29` is followed by `00:01:00;02`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Timebase {
  /// Non-drop-frame at an arbitrary rate (23.976, 24, 25, 29.97, 50, …).
  Ndf(f64),
  /// SMPTE drop-frame at nominal 30, wall rate 29.97 fps.
  Df2997,
  /// SMPTE drop-frame at nominal 60, wall rate 59.94 fps.
  Df5994,
}

impl Timebase {
  /// Nominal (display) rate the H:M:S:F counter runs at: 30 / 60 for the
  /// DF timebases, and `fps.round()` for NDF — 29.97 timecode is displayed
  /// in a 30-frame counter (which is exactly why NDF drifts ~3.6 s/hour
  /// against wall clock).
  fn nominal(self) -> u64 {
    match self {
      Timebase::Ndf(fps) => (fps.round() as u64).max(1),
      Timebase::Df2997 => 30,
      Timebase::Df5994 => 60,
    }
  }

  /// Wall-clock frames per second (the quoted rate).
  fn fps(self) -> f64 {
    match self {
      Timebase::Ndf(fps) => fps,
      Timebase::Df2997 => 29.97,
      Timebase::Df5994 => 59.94,
    }
  }

  fn drop_per_min(self) -> u64 {
    match self {
      Timebase::Ndf(_) => 0,
      Timebase::Df2997 => 2,
      Timebase::Df5994 => 4,
    }
  }

  fn is_df(self) -> bool {
    self.drop_per_min() > 0
  }

  /// Frames dropped before displayed minute `tm` begins.
  fn drops_before(self, tm: u64) -> u64 {
    self.drop_per_min() * (tm - tm / 10)
  }

  /// Wall-clock milliseconds for a displayed timecode value.
  ///
  /// For DF timebases the components must form a display that exists
  /// (frames `;00`/`;01` at 30, `;00`–`;03` at 60 do not exist at drop
  /// minutes). Non-existent components are still converted — they map to
  /// the frame number a counter would have reached without the skip.
  pub fn display_to_ms(self, h: u64, m: u64, s: u64, f: u64) -> u64 {
    let tm = h * 60 + m;
    let frame_number = (h * 3600 + m * 60 + s) * self.nominal() + f - self.drops_before(tm);
    frames_to_ms(frame_number, self.fps())
  }

  /// Decompose wall-clock milliseconds into this timebase's display.
  pub fn ms_to_display(self, ms: u64) -> (u64, u64, u64, u64) {
    let frame_number = ms_to_frames(ms, self.fps());
    self.frame_number_to_display(frame_number)
  }

  /// Display components for a wall-clock frame number.
  fn frame_number_to_display(self, frame_number: u64) -> (u64, u64, u64, u64) {
    if !self.is_df() {
      let nominal = (self.fps().round() as u64).max(1);
      return decompose_nominal(frame_number, nominal);
    }
    let nominal = self.nominal();
    // The drop count depends on the displayed minute, which is what we are
    // solving for; search around the naive estimate until consistent.
    let approx_tm = frame_number / (60 * nominal);
    for tm in approx_tm.saturating_sub(2)..=approx_tm + 2 {
      let dropped = self.drops_before(tm);
      let display_total = frame_number + dropped;
      if display_total >= tm * 60 * nominal && display_total < (tm + 1) * 60 * nominal {
        return decompose_nominal(display_total, nominal);
      }
    }
    decompose_nominal(frame_number, nominal)
  }
}

fn decompose_nominal(total: u64, nominal: u64) -> (u64, u64, u64, u64) {
  let h = total / (3600 * nominal);
  let m = (total % (3600 * nominal)) / (60 * nominal);
  let s = (total % (60 * nominal)) / nominal;
  let f = total % nominal;
  (h, m, s, f)
}

pub fn split_text_chunks(text: &str, max_chars: usize) -> Vec<String> {
  let mut chunks = Vec::new();
  let words: Vec<&str> = text.split_whitespace().collect();
  let mut current = String::with_capacity(max_chars);

  for word in words {
    let needed = if current.is_empty() {
      word.len()
    } else {
      current.len() + 1 + word.len()
    };

    if needed > max_chars && !current.is_empty() {
      chunks.push(std::mem::take(&mut current));
      current.push_str(word);
    } else {
      if !current.is_empty() {
        current.push(' ');
      }
      current.push_str(word);
    }
  }

  if !current.is_empty() {
    chunks.push(current);
  }

  chunks
}

#[cfg(test)]
mod tests {
  use super::*;

  // Values verified with the Python model in the repo's SMPTE 12M notes
  // (AGENTS §6.4): wall ms = frame_number * 1000 / 29.97 (or 59.94), where
  // frame_number subtracts the dropped frames of prior minutes.
  #[test]
  fn test_df2997_known_values() {
    assert_eq!(Timebase::Df2997.display_to_ms(1, 0, 0, 0), 3_600_000);
    assert_eq!(Timebase::Df2997.display_to_ms(0, 10, 0, 0), 600_000);
    // 00:01:00;00: 2 frames dropped before it → 1798 frames elapsed.
    assert_eq!(Timebase::Df2997.display_to_ms(0, 1, 0, 0), 59_993);
    assert_eq!(Timebase::Df2997.display_to_ms(0, 0, 59, 29), 60_027);
  }

  #[test]
  fn test_df5994_known_values() {
    assert_eq!(Timebase::Df5994.display_to_ms(1, 0, 0, 0), 3_600_000);
    assert_eq!(Timebase::Df5994.display_to_ms(0, 10, 0, 0), 600_000);
    // 4 frames dropped per minute → 3596 frames elapsed.
    assert_eq!(Timebase::Df5994.display_to_ms(0, 1, 0, 0), 59_993);
  }

  #[test]
  fn test_ndf_known_values() {
    // 00:10:00:00 in NDF 29.97 is 18000 frames — 600.6 s of wall clock.
    assert_eq!(Timebase::Ndf(29.97).display_to_ms(0, 10, 0, 0), 600_601);
    assert_eq!(Timebase::Ndf(25.0).display_to_ms(0, 0, 1, 0), 1_000);
  }

  #[test]
  fn test_df_ms_to_display_round_trip() {
    // Valid displays round-trip. At drop minutes ;00/;01 (30) and
    // ;00..;03 (60) do not exist and are skipped here.
    let nominal = Timebase::Df2997.nominal();
    for tm in [0u64, 1, 9, 10, 11, 59, 60, 61, 100, 599, 600, 601, 1439] {
      for f in 0..nominal {
        if tm % 10 != 0 && f < 2 {
          continue; // skipped frames
        }
        let display_total = tm * 60 * nominal + f;
        let (h, m, s, ff) = decompose_nominal(display_total, nominal);
        let ms = Timebase::Df2997.display_to_ms(h, m, s, ff);
        assert_eq!(
          Timebase::Df2997.ms_to_display(ms),
          (h, m, s, ff),
          "29.97DF round trip failed for {h}:{m}:{s};{ff}"
        );
      }
    }
    for tm in [0u64, 1, 10, 61, 600, 1439] {
      for f in 0..Timebase::Df5994.nominal() {
        if tm % 10 != 0 && f < 4 {
          continue;
        }
        let display_total = tm * 60 * Timebase::Df5994.nominal() + f;
        let (h, m, s, ff) = decompose_nominal(display_total, Timebase::Df5994.nominal());
        let ms = Timebase::Df5994.display_to_ms(h, m, s, ff);
        assert_eq!(
          Timebase::Df5994.ms_to_display(ms),
          (h, m, s, ff),
          "59.94DF round trip failed for {h}:{m}:{s};{ff}"
        );
      }
    }
  }

  #[test]
  fn test_reinterpret_ndf_parsed_df_timecode() {
    // A 29.97 DF file misread as NDF: its "00:10:00:00" became 600601 ms.
    // Reinterpreting NDF→DF recovers the true 600000 ms wall clock.
    let misread = Timebase::Ndf(29.97).display_to_ms(0, 10, 0, 0);
    assert_eq!(misread, 600_601);
    let (h, m, s, f) = Timebase::Ndf(29.97).ms_to_display(misread);
    assert_eq!(Timebase::Df2997.display_to_ms(h, m, s, f), 600_000);
  }

  #[test]
  fn test_timebase_serde_round_trip() {
    for tb in [
      Timebase::Ndf(23.976),
      Timebase::Ndf(25.0),
      Timebase::Df2997,
      Timebase::Df5994,
    ] {
      let json = serde_json::to_string(&tb).unwrap();
      let back: Timebase = serde_json::from_str(&json).unwrap();
      assert_eq!(tb, back);
    }
  }
}
