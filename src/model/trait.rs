use super::convert::split_text_chunks;
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
}
