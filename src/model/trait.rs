use super::convert::{Timebase, frames_to_ms, ms_to_frames, split_text_chunks};
use super::format::Format;
use super::subtitle::Subtitle;
use super::validation::ValidationIssue;

/// Trait unifying all subtitle format operations. The four required methods
/// (`subtitles`, `subtitles_mut`, `format`, `to_string_with_format`) are
/// per-variant; the editing methods below have default implementations that
/// work through `subtitles()`/`subtitles_mut()`, so every format gets them for
/// free.
pub trait SubtitleFormat: std::fmt::Debug + Clone + Send + Sync {
  fn subtitles(&self) -> &[Subtitle];
  fn subtitles_mut(&mut self) -> &mut Vec<Subtitle>;
  fn format(&self) -> Format;
  fn to_string_with_format(&self, format: &Format) -> String;

  fn to_string(&self) -> String {
    self.to_string_with_format(&self.format())
  }

  fn shift_all(&mut self, offset_ms: i64) {
    for sub in self.subtitles_mut().iter_mut() {
      sub.shift(offset_ms);
    }
  }

  fn map<F: FnMut(&mut Subtitle)>(mut self, mut f: F) -> Self {
    for sub in self.subtitles_mut().iter_mut() {
      f(sub);
    }
    self
  }

  fn filter<F: FnMut(&Subtitle) -> bool>(mut self, mut f: F) -> Self {
    self.subtitles_mut().retain(|s| f(s));
    self
  }

  fn sort(&mut self) {
    self.subtitles_mut().sort_by_key(|s| (s.start, s.end));
  }

  fn validate(&self) -> Vec<ValidationIssue> {
    let subs = SubtitleFormat::subtitles(self);
    let mut issues = Vec::new();

    for (i, sub) in subs.iter().enumerate() {
      if sub.end < sub.start {
        issues.push(ValidationIssue::NegativeDuration {
          index: i,
          start: sub.start,
          end: sub.end,
        });
      }
      if sub.start == sub.end {
        issues.push(ValidationIssue::ZeroDuration {
          index: i,
          time: sub.start,
        });
      }
    }

    let mut order: Vec<usize> = (0..subs.len()).collect();
    order.sort_by_key(|&i| (subs[i].start, subs[i].end));
    for w in order.windows(2) {
      let (a, b) = (w[0], w[1]);
      if subs[b].start < subs[a].end {
        issues.push(ValidationIssue::Overlap {
          index_a: a,
          index_b: b,
          end_a: subs[a].end,
          start_b: subs[b].start,
        });
      }
    }

    for i in 1..subs.len() {
      if subs[i].start < subs[i - 1].start {
        issues.push(ValidationIssue::DecreasingStartTime {
          index: i,
          prev_start: subs[i - 1].start,
          curr_start: subs[i].start,
        });
      }
    }

    issues
  }

  fn validate_extended(
    &self,
    max_chars: usize,
    max_gap_ms: u64,
    max_cps: f64,
  ) -> Vec<ValidationIssue> {
    let mut issues = self.validate();
    let subs = SubtitleFormat::subtitles(self);

    for (i, sub) in subs.iter().enumerate() {
      let char_count = sub.text.chars().count();
      if char_count > max_chars {
        issues.push(ValidationIssue::TextTooLong {
          index: i,
          chars: char_count,
          max_chars,
        });
      }

      let cps = sub.chars_per_second();
      if cps > max_cps {
        issues.push(ValidationIssue::CpsTooHigh {
          index: i,
          cps,
          max_cps,
        });
      }
    }

    for i in 1..subs.len() {
      let gap = subs[i].start.saturating_sub(subs[i - 1].end);
      if gap > max_gap_ms {
        issues.push(ValidationIssue::TooLongGap {
          index: i,
          prev_end: subs[i - 1].end,
          curr_start: subs[i].start,
          gap_ms: gap,
        });
      }
    }

    issues
  }

  /// One-shot guideline check: structural timing issues from `validate()`
  /// plus the broadcaster rule set (per-line length, line count, duration
  /// bounds, minimum gap, reading speed).
  fn validate_guideline(&self, guideline: &crate::guidelines::Guideline) -> Vec<ValidationIssue> {
    let mut issues = self.validate();
    issues.extend(crate::guidelines::validate(
      SubtitleFormat::subtitles(self),
      guideline,
    ));
    issues
  }

  fn merge_adjacent(&mut self, max_gap_ms: u64) {
    self.sort();
    let subs = self.subtitles_mut();
    let mut i = 0;
    while i + 1 < subs.len() {
      let gap = subs[i + 1].start.saturating_sub(subs[i].end);
      if gap <= max_gap_ms {
        let next_text = std::mem::take(&mut subs[i + 1].text);
        subs[i].end = subs[i + 1].end;
        subs[i].text.push('\n');
        subs[i].text.push_str(&next_text);
        subs.remove(i + 1);
      } else {
        i += 1;
      }
    }
  }

  fn remove_overlaps(&mut self) {
    self.sort();
    let subs = self.subtitles_mut();
    for i in 0..subs.len().saturating_sub(1) {
      if subs[i + 1].start < subs[i].end {
        subs[i + 1].start = subs[i].end;
      }
    }
  }

  fn enforce_min_duration(&mut self, min_ms: u64) {
    self.sort();
    let subs = self.subtitles_mut();
    for i in 0..subs.len() {
      let dur = subs[i].duration_ms();
      if dur < min_ms {
        let max_end = if i + 1 < subs.len() {
          subs[i + 1].start
        } else {
          u64::MAX
        };
        let desired_end = subs[i].start + min_ms;
        subs[i].end = desired_end.min(max_end);
      }
    }
  }

  fn enforce_max_duration(&mut self, max_ms: u64) {
    for sub in self.subtitles_mut().iter_mut() {
      let dur = sub.duration_ms();
      if dur > max_ms {
        sub.end = sub.start + max_ms;
      }
    }
  }

  /// Ensure at least `min_gap_ms` between consecutive subtitles by pulling
  /// back the earlier cue's end. Start times are sync-critical, so they stay
  /// fixed. When shortening would invert the earlier cue's duration (the gap
  /// is impossible without moving starts or shortening below zero), the pair
  /// is left untouched — `validate_guideline` reports it as `TooShortGap`.
  fn enforce_min_gap(&mut self, min_gap_ms: u64) {
    self.sort();
    let subs = self.subtitles_mut();
    for i in 1..subs.len() {
      let gap = subs[i].start.saturating_sub(subs[i - 1].end);
      if gap >= min_gap_ms {
        continue;
      }
      let new_end = subs[i].start.saturating_sub(min_gap_ms);
      if new_end > subs[i - 1].start {
        subs[i - 1].end = new_end;
      }
    }
  }

  /// Repair roll-up style captions: collapse each run of consecutive
  /// (in time order) subtitles with identical text into a single cue
  /// spanning the whole run. Only *adjacent* duplicates collapse — the
  /// same line recurring later (song chorus) is intentional repetition
  /// and survives.
  fn remove_repeating_lines(&mut self) {
    self.sort();
    let subs = self.subtitles_mut();
    let mut write = 0;
    for read in 1..subs.len() {
      if subs[read].text.trim() == subs[write].text.trim() {
        subs[write].end = subs[write].end.max(subs[read].end);
      } else {
        write += 1;
        subs.swap(write, read);
      }
    }
    subs.truncate(write + 1);
  }

  /// Merge consecutive subtitles with identical text whose gap is at most
  /// `max_gap_ms` into one cue spanning the union of their time ranges.
  /// Overlapping duplicates have a saturated gap of 0 and always merge.
  /// Unlike `remove_repeating_lines`, this keeps repeated lines that are
  /// separated by more than the threshold — the caller decides what
  /// counts as "the same moment".
  fn merge_identical(&mut self, max_gap_ms: u64) {
    self.sort();
    let subs = self.subtitles_mut();
    let mut write = 0;
    for read in 1..subs.len() {
      let same_text = subs[read].text.trim() == subs[write].text.trim();
      let gap = subs[read].start.saturating_sub(subs[write].end);
      if same_text && gap <= max_gap_ms {
        subs[write].end = subs[write].end.max(subs[read].end);
      } else {
        write += 1;
        subs.swap(write, read);
      }
    }
    subs.truncate(write + 1);
  }

  fn auto_extend_for_cps(&mut self, max_cps: f64) {
    self.sort();
    let subs = self.subtitles_mut();
    for i in 0..subs.len() {
      let chars = subs[i].plaintext().chars().count() as f64;
      let needed_ms = (chars / max_cps * 1000.0).ceil() as u64;
      let current = subs[i].duration_ms();
      if current < needed_ms {
        let max_end = if i + 1 < subs.len() {
          subs[i + 1].start
        } else {
          u64::MAX
        };
        subs[i].end = (subs[i].start + needed_ms).min(max_end);
      }
    }
  }

  fn extract_range(&self, start_ms: u64, end_ms: u64) -> Vec<Subtitle> {
    self
      .subtitles()
      .iter()
      .filter(|s| s.start < end_ms && s.end > start_ms)
      .map(|s| {
        let mut clone = s.clone();
        if clone.start < start_ms {
          clone.start = start_ms;
        }
        if clone.end > end_ms {
          clone.end = end_ms;
        }
        clone
      })
      .collect()
  }

  fn split_long(&mut self, max_chars: usize) {
    self.sort();
    let subs = self.subtitles_mut();

    let mut i = 0;
    while i < subs.len() {
      let char_count = subs[i].text.chars().count();
      if char_count <= max_chars {
        i += 1;
        continue;
      }

      let start = subs[i].start;
      let end = subs[i].end;
      // Guard: zero or negative-duration subtitles cannot be split into
      // positive-duration chunks; skip them.
      if end <= start {
        i += 1;
        continue;
      }
      let style = subs[i].style.clone();
      let actor = subs[i].actor.clone();
      let text = std::mem::take(&mut subs[i].text);

      let chunks = split_text_chunks(&text, max_chars);
      let num_chunks = chunks.len() as u64;
      // Each chunk must have a positive duration. When the original
      // duration is too short to divide evenly (duration < num_chunks),
      // stretch the effective end so that chunk_duration = max(1) and
      // the chunks remain contiguous and monotonic. The last chunk's
      // end is the (possibly stretched) end, not the original end.
      let chunk_duration = ((end - start) / num_chunks).max(1);
      let effective_end = start + chunk_duration * num_chunks;

      subs[i].text = chunks[0].clone();
      subs[i].end = start + chunk_duration;

      let mut new_subs: Vec<Subtitle> = Vec::with_capacity(chunks.len() - 1);
      for (chunk_idx, chunk) in chunks.iter().enumerate().skip(1) {
        let new_start = start + (chunk_idx as u64) * chunk_duration;
        let new_end = if chunk_idx + 1 == chunks.len() {
          effective_end
        } else {
          start + ((chunk_idx + 1) as u64) * chunk_duration
        };
        let mut new_sub = Subtitle::new(new_start, new_end, chunk);
        new_sub.style = style.clone();
        new_sub.actor = actor.clone();
        new_subs.push(new_sub);
      }

      let insert_at = i + 1;
      let inserted = new_subs.len();
      subs.splice(insert_at..insert_at, new_subs);
      i += 1 + inserted;
    }
  }

  fn transform_framerate(&mut self, in_fps: f64, out_fps: f64) {
    let ratio = out_fps / in_fps;
    for sub in self.subtitles_mut().iter_mut() {
      sub.start = ((sub.start as f64) * ratio).round() as u64;
      sub.end = ((sub.end as f64) * ratio).round() as u64;
    }
  }

  /// Round all timestamps to whole frame boundaries of `fps` — useful
  /// after a linear framerate conversion whose targets feed frame-indexed
  /// formats (MicroDVD, MPL2, SCC) and may otherwise sit between frames.
  fn snap_to_frames(&mut self, fps: f64) {
    for sub in self.subtitles_mut().iter_mut() {
      sub.start = frames_to_ms(ms_to_frames(sub.start, fps), fps);
      sub.end = frames_to_ms(ms_to_frames(sub.end, fps), fps);
    }
  }

  /// Retime between timebase interpretations: the current millisecond
  /// values are read as displays of `from` and rewritten as the same
  /// displays in `to`. This repairs files whose SMPTE drop-frame timecodes
  /// were parsed as non-drop (drifting ~3.6 s per hour) or vice versa, and
  /// is not a wall-clock conversion — identical displays in different
  /// timebases legitimately mean different wall-clock times.
  fn reinterpret_framerate(&mut self, from: Timebase, to: Timebase) {
    for sub in self.subtitles_mut().iter_mut() {
      let (sh, sm, ss, sf) = from.ms_to_display(sub.start);
      let (eh, em, es, ef) = from.ms_to_display(sub.end);
      sub.start = to.display_to_ms(sh, sm, ss, sf);
      sub.end = to.display_to_ms(eh, em, es, ef);
    }
  }

  /// Repair roll-up style captions where each cue repeats everything so
  /// far and appends new lines (`"A"` → `"A\nB"` → `"A\nB\nC"`). Each
  /// cue's text shrinks to the lines it adds over its predecessor,
  /// yielding progressive cues (`"A"`, `"B"`, `"C"`) with timings
  /// unchanged. Cues that repeat their predecessor verbatim are left for
  /// `remove_repeating_lines` / `merge_identical`; non-accumulating cues
  /// are untouched.
  fn convert_rollup(&mut self) {
    self.sort();
    let subs = self.subtitles_mut();
    // Walk backwards so each cue is compared against its predecessor's
    // still-accumulated text — chains unroll one step per cue.
    for i in (1..subs.len()).rev() {
      let prev = subs[i - 1].text.trim_end().to_string();
      if prev.is_empty() {
        continue;
      }
      if let Some(suffix) = subs[i].text.strip_prefix(prev.as_str()) {
        let suffix = suffix.trim_start_matches(['\n', '\r', ' ']).to_string();
        if !suffix.is_empty() {
          subs[i].text = suffix;
        }
      }
    }
  }

  /// Trim cues so they respect shot changes (Netflix-style rule: a cue
  /// must end at least `before_frames` before a cut and start at least
  /// `after_frames` after one).
  ///
  /// For each cut (ms) the guard zone is `[cut - before, cut + after]`:
  /// - a cue that ends inside the before-guard or spans the cut is trimmed
  ///   to `cut - before`;
  /// - a cue that starts inside the after-guard is trimmed to
  ///   `cut + after`;
  /// - a cue spanning the cut keeps its **larger** side (before-side vs
  ///   after-side) so the least text timing is lost;
  /// - a cue entirely inside the guard zone cannot be trimmed without
  ///   collapsing and is left untouched — report it separately if needed.
  ///
  /// Text is never altered; only timings move. Cuts are best applied in
  /// chronological order ( [`crate::shotlist::parse_edl_cuts`] returns
  /// them sorted).
  fn apply_shot_changes(
    &mut self,
    cuts_ms: &[u64],
    before_frames: u64,
    after_frames: u64,
    fps: f64,
  ) {
    if cuts_ms.is_empty() {
      return;
    }
    let before_ms = frames_to_ms(before_frames, fps);
    let after_ms = frames_to_ms(after_frames, fps);
    self.sort();
    let subs = self.subtitles_mut();
    for cut in cuts_ms {
      let guard_lo = cut.saturating_sub(before_ms);
      let guard_hi = cut.saturating_add(after_ms);
      for sub in subs.iter_mut() {
        let (s, e) = (sub.start, sub.end);
        if s < *cut && e > *cut {
          // Spans the cut: keep the larger side of the guard zone.
          let before_side = guard_lo.saturating_sub(s);
          let after_side = e.saturating_sub(guard_hi);
          if before_side >= after_side {
            if guard_lo > s {
              sub.end = guard_lo;
            }
          } else if guard_hi < e {
            sub.start = guard_hi;
          }
        } else if e > guard_lo && e <= *cut && s < guard_lo {
          // Ends inside the before-guard zone.
          sub.end = guard_lo;
        } else if s >= *cut && s < guard_hi && e > guard_hi {
          // Starts inside the after-guard zone.
          sub.start = guard_hi;
        }
      }
    }
  }
}
