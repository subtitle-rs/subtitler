//! Anchor-based subtitle synchronization.
//!
//! Given a "wrong" subtitle file (drifted offset/framerate) and a
//! reference version of the same content, solve for the time
//! transformation that best aligns them — using only text and
//! timestamps (the alass approach: no audio decoding).
//!
//! Pipeline: match cues 1:1 by normalized text similarity within a time
//! window (greedy best-score assignment), treat each match as an anchor
//! `(wrong_start, offset)`, then fit a piecewise-constant offset
//! function over the anchors with an L2 dynamic program (a split is
//! worth its `split_penalty` when it removes enough squared deviation).
//! This covers the three real-world cases: constant shift, drift
//! (framerate mismatch → segmented offsets), and multi-part files.

use crate::model::{Subtitle, SubtitleFile, SubtitleFormat};

/// Synchronization tuning knobs.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SyncOptions {
  /// Search window around a cue's own start when looking for reference
  /// matches, in milliseconds.
  pub max_offset_ms: u64,
  /// Minimum text similarity for a pair to become an anchor (0..1).
  pub min_score: f64,
  /// Cost (ms²) of one extra piecewise segment in the offset fit. A
  /// split pays for itself when it removes more than this much squared
  /// deviation across its anchors.
  pub split_penalty: f64,
  /// Cap on anchors fed to the piecewise fit (uniformly sampled by
  /// position when exceeded — bounds the O(n²) DP).
  pub max_anchors: usize,
}

impl Default for SyncOptions {
  fn default() -> Self {
    SyncOptions {
      max_offset_ms: 300_000,
      min_score: 0.5,
      split_penalty: 1_000_000.0,
      max_anchors: 2_000,
    }
  }
}

/// One piecewise segment: cues starting at `from_ms` or later (until the
/// next segment) are shifted by `offset_ms`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SyncSegment {
  pub from_ms: u64,
  pub offset_ms: i64,
  #[serde(default)]
  pub slope: f64,
}

/// Result of an auto-sync pass.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SyncReport {
  /// Offset segments, ordered by `from_ms` (empty when nothing matched).
  pub segments: Vec<SyncSegment>,
  /// 1:1 text matches used as anchors.
  pub anchors: usize,
  /// Cues in the wrong file that received an applied offset.
  pub matched: usize,
  /// Total cues in the wrong file.
  pub total: usize,
  /// Mean text similarity of the anchors (0 when none).
  pub mean_score: f64,
}

impl SyncReport {
  /// Fraction of wrong-file cues covered by anchors (0..1).
  pub fn coverage(&self) -> f64 {
    if self.total == 0 {
      0.0
    } else {
      self.anchors as f64 / self.total as f64
    }
  }

  /// True when a single constant offset covers the whole timeline.
  pub fn is_single_shift(&self) -> bool {
    self.segments.len() == 1
  }

  /// The offset in effect at `at_ms` (0 when no segments).
  pub fn offset_at(&self, at_ms: u64) -> i64 {
    let mut offset = 0i64;
    for seg in &self.segments {
      if seg.from_ms <= at_ms {
        offset = seg.offset_ms;
      } else {
        break;
      }
    }
    offset
  }
}

/// Lowercase and keep only alphanumeric characters — the comparison
/// basis for matching (punctuation/case/whitespace differences between
/// releases must not affect similarity).
pub fn normalized_text(text: &str) -> String {
  text
    .chars()
    .filter(|c| c.is_alphanumeric())
    .flat_map(|c| c.to_lowercase())
    .collect()
}

/// Levenshtein edit distance over chars.
fn levenshtein(a: &[char], b: &[char]) -> usize {
  if a.is_empty() {
    return b.len();
  }
  if b.is_empty() {
    return a.len();
  }
  let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
  let mut prev: Vec<usize> = (0..=short.len()).collect();
  let mut cur = vec![0usize; short.len() + 1];
  for (i, lc) in long.iter().enumerate() {
    cur[0] = i + 1;
    for (j, sc) in short.iter().enumerate() {
      let cost = if lc == sc { 0 } else { 1 };
      cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
    }
    std::mem::swap(&mut prev, &mut cur);
  }
  prev[short.len()]
}

/// Similarity in [0, 1]: 1 - normalized edit distance over the
/// alphanumeric-lowercase projections. Identical projections score 1.0.
pub fn similarity(a: &str, b: &str) -> f64 {
  let a = normalized_text(a);
  let b = normalized_text(b);
  if a.is_empty() && b.is_empty() {
    return 1.0;
  }
  let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
  let max = a.len().max(b.len());
  if max == 0 {
    return 1.0;
  }
  1.0 - levenshtein(&a, &b) as f64 / max as f64
}

/// Match wrong cues to reference cues 1:1, greedily by text similarity
/// within the time window. Returns `(wrong_index, ref_index, score)`
/// sorted by wrong start time.
fn match_anchors(
  wrong: &[Subtitle],
  reference: &[Subtitle],
  opts: &SyncOptions,
) -> Vec<(usize, usize, f64)> {
  // Reference cues sorted by start for window lookups.
  let mut r_sorted: Vec<(u64, usize)> = reference
    .iter()
    .enumerate()
    .map(|(i, r)| (r.start, i))
    .collect();
  r_sorted.sort_unstable();
  let r_starts: Vec<u64> = r_sorted.iter().map(|(s, _)| *s).collect();

  // Fast path: identical normalized texts pair at score 1.0 regardless
  // of window (a same-release copy shifted wholesale).
  let mut by_text: std::collections::HashMap<&str, Vec<usize>> = std::collections::HashMap::new();
  for (i, r) in reference.iter().enumerate() {
    let norm = normalized_text(&r.text);
    if !norm.is_empty() {
      by_text
        .entry(Box::leak(norm.into_boxed_str()))
        .or_default()
        .push(i);
    }
  }

  let mut candidates: Vec<(f64, usize, usize)> = Vec::new();
  for (wi, w) in wrong.iter().enumerate() {
    let w_norm = normalized_text(&w.text);
    if w_norm.is_empty() {
      continue;
    }
    // Identical-text references first (score 1.0, any offset).
    if let Some(matches) = by_text.get(Box::leak(w_norm.clone().into_boxed_str())) {
      for &ri in matches {
        candidates.push((1.0, wi, ri));
      }
    }
    // Windowed fuzzy candidates, best-length-prefiltered.
    let lo = w.start.saturating_sub(opts.max_offset_ms);
    let hi = w.start + opts.max_offset_ms;
    let begin = r_starts.partition_point(|s| *s < lo);
    let mut considered = 0usize;
    for (r_start, ri) in r_sorted[begin..].iter() {
      if *r_start > hi {
        break;
      }
      considered += 1;
      if considered > 200 {
        break; // bound the quadratic blow-up on dense files
      }
      let r = &reference[*ri];
      let r_norm = normalized_text(&r.text);
      if r_norm.is_empty() || r_norm == w_norm {
        continue; // identical handled above; empty has no signal
      }
      // Length-ratio prefilter: wildly different lengths cannot reach
      // the score threshold.
      let (a, b) = (w_norm.chars().count(), r_norm.chars().count());
      let (lo_len, hi_len) = (a.min(b) as f64, a.max(b) as f64);
      if hi_len == 0.0 || lo_len / hi_len < opts.min_score {
        continue;
      }
      let score = similarity(&w.text, &r.text);
      if score >= opts.min_score {
        candidates.push((score, wi, *ri));
      }
    }
  }

  // Greedy 1:1 assignment by descending score.
  candidates.sort_by(|x, y| y.0.total_cmp(&x.0));
  let mut used_w = vec![false; wrong.len()];
  let mut used_r = vec![false; reference.len()];
  let mut anchors = Vec::new();
  let mut score_sum = 0.0;
  for (score, wi, ri) in candidates {
    if used_w[wi] || used_r[ri] {
      continue;
    }
    used_w[wi] = true;
    used_r[ri] = true;
    anchors.push((wi, ri, score));
    score_sum += score;
  }
  anchors.sort_by_key(|(wi, _, _)| wrong[*wi].start);
  let _ = score_sum;
  anchors
}

/// Fit piecewise-affine offsets over `anchors` (sorted by wrong start)
/// with an L2 DP: each segment solves `offset(t) = a·t + b` by least
/// squares (O(1) per candidate segment from prefix sums), and each
/// additional segment costs `split_penalty`. Constant shifts fall out
/// as a = 0; framerate drift as a ≈ scale − 1.
fn fit_segments(anchors: &[(u64, i64)], opts: &SyncOptions) -> Vec<SyncSegment> {
  if anchors.is_empty() {
    return Vec::new();
  }
  let n = anchors.len();
  let t: Vec<f64> = anchors.iter().map(|(s, _)| *s as f64).collect();
  let o: Vec<f64> = anchors.iter().map(|(_, off)| *off as f64).collect();
  let (mut pt, mut pt2, mut po, mut pto, mut po2) = (
    vec![0f64; n + 1],
    vec![0f64; n + 1],
    vec![0f64; n + 1],
    vec![0f64; n + 1],
    vec![0f64; n + 1],
  );
  for i in 0..n {
    pt[i + 1] = pt[i] + t[i];
    pt2[i + 1] = pt2[i] + t[i] * t[i];
    po[i + 1] = po[i] + o[i];
    pto[i + 1] = pto[i] + t[i] * o[i];
    po2[i + 1] = po2[i] + o[i] * o[i];
  }

  // Least-squares affine fit of anchors i..j (exclusive end): returns
  // (a, b, sse) for offset(t) = a·t + b. n == 1 degenerates to a = 0.
  let fit = |i: usize, j: usize| -> (f64, f64, f64) {
    let count = (j - i) as f64;
    let st = pt[j] - pt[i];
    let st2 = pt2[j] - pt2[i];
    let so = po[j] - po[i];
    let sto = pto[j] - pto[i];
    let so2 = po2[j] - po2[i];
    if count == 1.0 || (st2 * count - st * st).abs() < 1e-9 {
      let b = so / count;
      return (0.0, b, 0.0);
    }
    let denom = count * st2 - st * st;
    let a = (count * sto - st * so) / denom;
    let b = (so - a * st) / count;
    let sse = (so2 - 2.0 * a * sto - 2.0 * b * so + a * a * st2 + 2.0 * a * b * st + b * b * count)
      .max(0.0);
    (a, b, sse)
  };

  let lambda = opts.split_penalty;
  let mut f = vec![0f64; n + 1];
  let mut back = vec![0usize; n + 1];
  for i in 1..=n {
    let mut best = f64::INFINITY;
    let mut best_k = 0usize;
    for (k, fk) in f[..i].iter().enumerate() {
      let candidate = fk + lambda + fit(k, i).2;
      if candidate < best {
        best = candidate;
        best_k = k;
      }
    }
    f[i] = best;
    back[i] = best_k;
  }

  // Reconstruct segments from backpointers.
  let mut bounds = vec![n];
  let mut cur = n;
  while cur > 0 {
    cur = back[cur];
    bounds.push(cur);
  }
  bounds.reverse();
  let mut segments = Vec::with_capacity(bounds.len() - 1);
  for w in bounds.windows(2) {
    let (from_idx, to_idx) = (w[0], w[1]);
    let (a, b, _) = fit(from_idx, to_idx);
    let from_ms = anchors[from_idx].0;
    segments.push(SyncSegment {
      from_ms,
      offset_ms: (a * from_ms as f64 + b).round() as i64,
      slope: a * 1000.0,
    });
  }
  segments
}

/// Compute the sync plan for two cue lists: anchors, mean score and the
/// fitted segments (see [`auto_sync_file`]).
pub fn plan_sync(wrong: &[Subtitle], reference: &[Subtitle], opts: &SyncOptions) -> SyncReport {
  let total = wrong.len();
  let matches = match_anchors(wrong, reference, opts);
  let anchors = matches.len();
  let mean_score = if anchors > 0 {
    matches.iter().map(|(_, _, s)| *s).sum::<f64>() / anchors as f64
  } else {
    0.0
  };

  // Downsample anchors for the O(n²) DP when necessary.
  let mut anchor_pairs: Vec<(u64, i64)> = matches
    .iter()
    .map(|(wi, ri, _)| {
      let o = reference[*ri].start as i64 - wrong[*wi].start as i64;
      (wrong[*wi].start, o)
    })
    .collect();
  if anchor_pairs.len() > opts.max_anchors {
    let step = anchor_pairs.len() as f64 / opts.max_anchors as f64;
    anchor_pairs = (0..opts.max_anchors)
      .map(|k| anchor_pairs[(k as f64 * step) as usize])
      .collect();
  }

  let segments = fit_segments(&anchor_pairs, opts);
  SyncReport {
    segments,
    anchors,
    matched: 0,
    total,
    mean_score,
  }
}

/// Synchronize `wrong` in place against `reference` and report what was
/// done. Both files may be in any format; only text and timing matter.
pub fn auto_sync_file(
  wrong: &mut SubtitleFile,
  reference: &SubtitleFile,
  opts: &SyncOptions,
) -> SyncReport {
  let wrong_subs: Vec<Subtitle> = wrong.subtitles().to_vec();
  let ref_subs: Vec<Subtitle> = reference.subtitles().to_vec();
  let mut report = plan_sync(&wrong_subs, &ref_subs, opts);
  let subs = wrong.subtitles_mut();
  let mut views: Vec<&mut Subtitle> = subs.iter_mut().collect();
  report.matched = apply_segments(&mut views, &report.segments);
  report
}

/// Apply offset segments to cues; returns the number of cues changed.
fn apply_segments(subs: &mut [&mut Subtitle], segments: &[SyncSegment]) -> usize {
  if segments.is_empty() {
    return 0;
  }
  let mut changed = 0usize;
  for sub in subs.iter_mut() {
    let seg = segments
      .iter()
      .rev()
      .find(|seg| seg.from_ms <= sub.start)
      .unwrap_or(&segments[0]);
    let transform = |t: u64| -> u64 {
      let shifted = seg.offset_ms as f64 + seg.slope * (t as f64 - seg.from_ms as f64) / 1000.0;
      ((t as i64) + shifted.round() as i64).max(0) as u64
    };
    let new_start = transform(sub.start);
    let new_end = transform(sub.end).max(new_start);
    if new_start != sub.start || new_end != sub.end {
      changed += 1;
    }
    sub.start = new_start;
    sub.end = new_end;
  }
  changed
}

/// Apply offset segments to any trait object's cues (see
/// [`SubtitleFormat::auto_sync`]).
pub fn apply_to(subs: &mut [&mut Subtitle], segments: &[SyncSegment]) -> usize {
  apply_segments(subs, segments)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn cue(start: u64, end: u64, text: &str) -> Subtitle {
    Subtitle::new(start, end, text)
  }

  #[test]
  fn similarity_basics() {
    assert_eq!(similarity("Hello, World!", "hello world"), 1.0);
    assert!(similarity("Hello world", "Hello word") >= 0.9);
    assert!(similarity("abc", "xyz") < 0.1);
    assert_eq!(similarity("", ""), 1.0);
  }

  #[test]
  fn constant_shift_single_segment() {
    // Wrong file is 5s late; texts identical.
    let mut wrong = SubtitleFile::Srt(vec![
      cue(6000, 8000, "Hello world"),
      cue(9000, 11000, "Second line"),
    ]);
    let reference = SubtitleFile::Srt(vec![
      cue(1000, 3000, "Hello world"),
      cue(4000, 6000, "Second line"),
    ]);
    let report = auto_sync_file(&mut wrong, &reference, &SyncOptions::default());
    assert_eq!(report.anchors, 2);
    assert!(report.is_single_shift());
    assert_eq!(report.segments[0].offset_ms, -5000);
    assert_eq!(report.segments[0].from_ms, 6000);
    let subs = wrong.subtitles();
    assert_eq!(subs[0].start, 1000);
    assert_eq!(subs[0].end, 3000);
    assert_eq!(subs[1].start, 4000);
    assert_eq!(report.coverage(), 1.0);
  }

  #[test]
  fn drift_is_fit_with_multiple_segments() {
    // 4% framerate drift: wrong times = correct / 1.04. Offsets grow
    // with position, so the piecewise fit must split.
    let reference: Vec<Subtitle> = (0..40)
      .map(|i| {
        let start = i * 10_000;
        cue(
          start,
          start + 5_000,
          &format!("line number {} of the test", i),
        )
      })
      .collect();
    let wrong: Vec<Subtitle> = reference
      .iter()
      .map(|s| {
        let w = |t: u64| (t as f64 / 1.04) as u64;
        cue(w(s.start), w(s.end), &s.text)
      })
      .collect();
    let mut wrong_file = SubtitleFile::Srt(wrong);
    let reference_file = SubtitleFile::Srt(reference);
    let report = auto_sync_file(&mut wrong_file, &reference_file, &SyncOptions::default());
    assert!(report.anchors >= 20, "anchors: {}", report.anchors);
    // The affine fit nails the drift with few segments: both cue starts
    // and ends land within 50ms of truth.
    let subs = wrong_file.subtitles();
    let truth: Vec<(u64, u64)> = (0..40).map(|i| (i * 10_000, i * 10_000 + 5_000)).collect();
    for (i, sub) in subs.iter().enumerate() {
      let start_err = sub.start.abs_diff(truth[i].0);
      let end_err = sub.end.abs_diff(truth[i].1);
      assert!(
        start_err <= 50 && end_err <= 50,
        "cue {} off by {}/{}ms",
        i,
        start_err,
        end_err
      );
    }
  }

  #[test]
  fn two_part_offsets_split() {
    // Part 1 unshifted, part 2 delayed by 30s (multi-part release).
    let mut wrong = SubtitleFile::Srt(vec![
      cue(1000, 3000, "first part line one"),
      cue(4000, 6000, "first part line two"),
      cue(45000, 47000, "second part line one"),
      cue(48000, 50000, "second part line two"),
    ]);
    let reference = SubtitleFile::Srt(vec![
      cue(1000, 3000, "first part line one"),
      cue(4000, 6000, "first part line two"),
      cue(15000, 17000, "second part line one"),
      cue(18000, 20000, "second part line two"),
    ]);
    let report = auto_sync_file(&mut wrong, &reference, &SyncOptions::default());
    assert_eq!(report.anchors, 4);
    assert_eq!(report.segments.len(), 2, "{:?}", report.segments);
    let subs = wrong.subtitles();
    assert_eq!(subs[0].start, 1000);
    assert_eq!(subs[2].start, 15000);
    assert_eq!(subs[3].start, 18000);
  }

  #[test]
  fn no_similarity_means_no_segments() {
    let mut wrong = SubtitleFile::Srt(vec![cue(1000, 3000, "completely different words")]);
    let reference = SubtitleFile::Srt(vec![cue(1000, 3000, "zzz qqq xxx")]);
    let report = auto_sync_file(&mut wrong, &reference, &SyncOptions::default());
    assert_eq!(report.anchors, 0);
    assert!(report.segments.is_empty());
    assert_eq!(report.coverage(), 0.0);
    // Nothing was moved.
    assert_eq!(wrong.subtitles()[0].start, 1000);
  }

  #[test]
  fn negative_offset_clamps_at_zero() {
    let mut wrong = SubtitleFile::Srt(vec![cue(500, 2000, "early cue text")]);
    let reference = SubtitleFile::Srt(vec![cue(0, 1500, "early cue text")]);
    let report = auto_sync_file(&mut wrong, &reference, &SyncOptions::default());
    assert_eq!(report.segments[0].offset_ms, -500);
    let subs = wrong.subtitles();
    assert_eq!(subs[0].start, 0);
    assert_eq!(subs[0].end, 1500);
  }

  #[test]
  fn identical_text_fast_path_beats_window_limits() {
    // Offset larger than the search window: the identical-text fast
    // path still anchors (whole-file shift beyond max_offset).
    let opts = SyncOptions {
      max_offset_ms: 1_000,
      ..SyncOptions::default()
    };
    let mut wrong = SubtitleFile::Srt(vec![cue(100_000, 102_000, "the very same text")]);
    let reference = SubtitleFile::Srt(vec![cue(50_000, 52_000, "the very same text")]);
    let report = auto_sync_file(&mut wrong, &reference, &opts);
    assert_eq!(report.anchors, 1);
    assert_eq!(report.segments[0].offset_ms, -50_000);
    assert_eq!(wrong.subtitles()[0].start, 50_000);
  }

  #[test]
  fn report_serializes() {
    let report = SyncReport {
      segments: vec![SyncSegment {
        from_ms: 0,
        offset_ms: -5000,
        slope: 0.0,
      }],
      anchors: 2,
      matched: 2,
      total: 2,
      mean_score: 1.0,
    };
    let json = serde_json::to_string(&report).unwrap();
    assert!(json.contains("\"offset_ms\":-5000"));
    let back: SyncReport = serde_json::from_str(&json).unwrap();
    assert_eq!(back, report);
  }

  #[test]
  fn offset_at_walks_segments() {
    let report = SyncReport {
      segments: vec![
        SyncSegment {
          from_ms: 0,
          offset_ms: 0,
          slope: 0.0,
        },
        SyncSegment {
          from_ms: 60_000,
          offset_ms: 30_000,
          slope: 0.0,
        },
      ],
      ..Default::default()
    };
    assert_eq!(report.offset_at(10_000), 0);
    assert_eq!(report.offset_at(60_000), 30_000);
    assert_eq!(report.offset_at(100_000), 30_000);
  }
}
