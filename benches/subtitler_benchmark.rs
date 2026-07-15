use criterion::{Criterion, black_box, criterion_group, criterion_main};
use subtitler::model::{Subtitle, SubtitleFile, frames_to_ms, ms_to_frames};
use subtitler::{ass, srt, utils, vtt};

// ── Utility Benchmarks ──

fn bench_parse_timestamp(c: &mut Criterion) {
  c.bench_function("parse_timestamp", |b| {
    b.iter(|| black_box(utils::parse_timestamp("01:02:03.456").unwrap()))
  });
}

fn bench_parse_timestamps(c: &mut Criterion) {
  c.bench_function("parse_timestamps", |b| {
    b.iter(|| {
      black_box(utils::parse_timestamps("00:01:02.000 --> 00:01:05.000 align:start").unwrap())
    })
  });
}

fn bench_pad_left(c: &mut Criterion) {
  c.bench_function("pad_left", |b| b.iter(|| black_box(utils::pad_left(5, 3))));
}

fn bench_format_timestamp_srt(c: &mut Criterion) {
  c.bench_function("format_timestamp_srt", |b| {
    b.iter(|| black_box(utils::format_timestamp(3723456, "srt")))
  });
}

fn bench_format_timestamp_vtt(c: &mut Criterion) {
  c.bench_function("format_timestamp_vtt", |b| {
    b.iter(|| black_box(utils::format_timestamp(3723456, "WebVTT")))
  });
}

fn bench_frame_ms_roundtrip(c: &mut Criterion) {
  c.bench_function("frame_ms_roundtrip", |b| {
    b.iter(|| {
      let frames = black_box(ms_to_frames(3600000, 23.976));
      black_box(frames_to_ms(frames, 23.976))
    })
  });
}

// ── SRT Benchmarks ──

fn small_srt() -> String {
  r#"1
00:00:01,000 --> 00:00:03,500
Hello! How are you today?

2
00:00:04,000 --> 00:00:06,500
I'm doing well, thank you!

3
00:00:07,000 --> 00:00:09,500
What are your plans for the weekend?

4
00:00:10,000 --> 00:00:12,500
I might go hiking if the weather is nice.

5
00:00:13,000 --> 00:00:15,500
That sounds like a great idea!

6
00:00:16,000 --> 00:00:18,500
Would you like to join me?

7
00:00:19,000 --> 00:00:21,500
I would love to! What time do you want to go?

8
00:00:22,000 --> 00:00:24,500
How about 8 AM? Early bird gets the worm!

9
00:00:25,000 --> 00:00:27,500
Perfect! I'll see you then.

10
00:00:28,000 --> 00:00:30,500
Great! Looking forward to it.
"#
  .to_string()
}

fn large_srt(num_entries: usize) -> String {
  let mut s = String::new();
  for i in 0..num_entries {
    let start = i as u64 * 3000;
    let end = start + 2500;
    s.push_str(&format!(
      "{}\n{:0>2}:{:0>2}:{:0>2},{:0>3} --> {:0>2}:{:0>2}:{:0>2},{:0>3}\nSubtitle entry number {} with some extra text to make it realistic.\n\n",
      i + 1,
      start / 3600000,
      (start % 3600000) / 60000,
      (start % 60000) / 1000,
      start % 1000,
      end / 3600000,
      (end % 3600000) / 60000,
      (end % 60000) / 1000,
      end % 1000,
      i + 1
    ));
  }
  s
}

fn bench_srt_parse_small(c: &mut Criterion) {
  let content = small_srt();
  c.bench_function("srt_parse_small", |b| {
    let rt = tokio::runtime::Runtime::new().unwrap();
    b.iter(|| rt.block_on(async { black_box(srt::parse_content(&content).await.unwrap()) }))
  });
}

fn bench_srt_parse_large(c: &mut Criterion) {
  let content = large_srt(1000);
  c.bench_function("srt_parse_large", |b| {
    let rt = tokio::runtime::Runtime::new().unwrap();
    b.iter(|| rt.block_on(async { black_box(srt::parse_content(&content).await.unwrap()) }))
  });
}

fn bench_srt_stringify_small(c: &mut Criterion) {
  let content = small_srt();
  let rt = tokio::runtime::Runtime::new().unwrap();
  let subs = rt.block_on(async { srt::parse_content(&content).await.unwrap() });
  c.bench_function("srt_stringify_small", |b| {
    b.iter(|| black_box(srt::to_string(&subs)))
  });
}

fn bench_srt_stringify_large(c: &mut Criterion) {
  let content = large_srt(1000);
  let rt = tokio::runtime::Runtime::new().unwrap();
  let subs = rt.block_on(async { srt::parse_content(&content).await.unwrap() });
  c.bench_function("srt_stringify_large", |b| {
    b.iter(|| black_box(srt::to_string(&subs)))
  });
}

fn bench_srt_detect_format(c: &mut Criterion) {
  let content = small_srt();
  c.bench_function("srt_detect_format", |b| {
    b.iter(|| black_box(srt::detect_format(content.as_bytes())))
  });
}

// ── VTT Benchmarks ──

fn small_vtt() -> String {
  r#"WEBVTT

1
00:00:01.000 --> 00:00:03.500
Hi there! How have you been?

2
00:00:04.000 --> 00:00:06.500
I've been doing well, thanks for asking!

3
00:00:07.000 --> 00:00:09.500
What have you been up to lately?

4
00:00:10.000 --> 00:00:12.500
Just working on some projects.

5
00:00:13.000 --> 00:00:15.500
Oh cool! What kind of projects?

6
00:00:16.000 --> 00:00:18.500
Mostly open source stuff on GitHub.

7
00:00:19.000 --> 00:00:21.500
That sounds interesting! Tell me more.

8
00:00:22.000 --> 00:00:24.500
I'm building a subtitle library in Rust.

9
00:00:25.000 --> 00:00:27.500
Nice! Is it on crates.io?

10
00:00:28.000 --> 00:00:30.500
Yes! It's called subtitler.
"#
  .to_string()
}

fn large_vtt(num_entries: usize) -> String {
  let mut s = String::from("WEBVTT\n\n");
  for i in 0..num_entries {
    let start = i as u64 * 3000;
    let end = start + 2500;
    s.push_str(&format!(
      "{}\n{:0>2}:{:0>2}:{:0>2}.{:0>3} --> {:0>2}:{:0>2}:{:0>2}.{:0>3}\nSubtitle entry number {} with some extra text to make it realistic.\n\n",
      i + 1,
      start / 3600000,
      (start % 3600000) / 60000,
      (start % 60000) / 1000,
      start % 1000,
      end / 3600000,
      (end % 3600000) / 60000,
      (end % 60000) / 1000,
      end % 1000,
      i + 1
    ));
  }
  s
}

fn bench_vtt_parse_small(c: &mut Criterion) {
  let content = small_vtt();
  c.bench_function("vtt_parse_small", |b| {
    let rt = tokio::runtime::Runtime::new().unwrap();
    b.iter(|| rt.block_on(async { black_box(vtt::parse_content(&content).await.unwrap()) }))
  });
}

fn bench_vtt_parse_large(c: &mut Criterion) {
  let content = large_vtt(1000);
  c.bench_function("vtt_parse_large", |b| {
    let rt = tokio::runtime::Runtime::new().unwrap();
    b.iter(|| rt.block_on(async { black_box(vtt::parse_content(&content).await.unwrap()) }))
  });
}

fn bench_vtt_stringify_small(c: &mut Criterion) {
  let content = small_vtt();
  let rt = tokio::runtime::Runtime::new().unwrap();
  let subs = rt.block_on(async { vtt::parse_content(&content).await.unwrap() });
  c.bench_function("vtt_stringify_small", |b| {
    b.iter(|| black_box(vtt::to_string(&subs, None)))
  });
}

fn bench_vtt_stringify_large(c: &mut Criterion) {
  let content = large_vtt(1000);
  let rt = tokio::runtime::Runtime::new().unwrap();
  let subs = rt.block_on(async { vtt::parse_content(&content).await.unwrap() });
  c.bench_function("vtt_stringify_large", |b| {
    b.iter(|| black_box(vtt::to_string(&subs, None)))
  });
}

fn bench_vtt_detect_format(c: &mut Criterion) {
  let content = small_vtt();
  c.bench_function("vtt_detect_format", |b| {
    b.iter(|| black_box(vtt::detect_format(content.as_bytes())))
  });
}

// ── ASS Benchmarks ──

fn small_ass() -> String {
  r#"[Script Info]
Title: Benchmark ASS
ScriptType: v4.00+
PlayResX: 384
PlayResY: 288

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,Hello! How are you today?
Dialogue: 0,0:00:04.00,0:00:06.50,Default,,0,0,0,,I'm doing well, thank you!
Dialogue: 0,0:00:07.00,0:00:09.50,Default,,0,0,0,,What are your plans?
Dialogue: 0,0:00:10.00,0:00:12.50,Default,,0,0,0,,I might go hiking if the weather is nice.
Dialogue: 0,0:00:13.00,0:00:15.50,Default,,0,0,0,,That sounds like a great idea!
Dialogue: 0,0:00:16.00,0:00:18.50,Default,,0,0,0,,Would you like to join me?
Dialogue: 0,0:00:19.00,0:00:21.50,Default,,0,0,0,,I would love to! What time?
Dialogue: 0,0:00:22.00,0:00:24.50,Default,,0,0,0,,How about 8 AM?
Dialogue: 0,0:00:25.00,0:00:27.50,Default,,0,0,0,,Perfect! I'll see you then.
Dialogue: 0,0:00:28.00,0:00:30.50,Default,,0,0,0,,Great! Looking forward to it.
"#
  .to_string()
}

fn large_ass(num_entries: usize) -> String {
  let mut s = String::from(
    "[Script Info]\nTitle: Large Benchmark\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
  );
  for i in 0..num_entries {
    let start = i as u64 * 3000;
    let start_s = start as f64 / 1000.0;
    let end_s = (start + 2500) as f64 / 1000.0;
    s.push_str(&format!(
      "Dialogue: 0,0:{:0>2}:{:0>2}.{:0>2},0:{:0>2}:{:0>2}.{:0>2},Default,,0,0,0,,Subtitle entry number {}\n",
      (start_s as u64) / 60,
      (start_s as u64) % 60,
      (start_s.fract() * 100.0) as u64,
      (end_s as u64) / 60,
      (end_s as u64) % 60,
      (end_s.fract() * 100.0) as u64,
      i + 1
    ));
  }
  s
}

fn bench_ass_parse_small(c: &mut Criterion) {
  let content = small_ass();
  c.bench_function("ass_parse_small", |b| {
    b.iter(|| black_box(ass::parse_content(&content).unwrap()))
  });
}

fn bench_ass_parse_large(c: &mut Criterion) {
  let content = large_ass(1000);
  c.bench_function("ass_parse_large", |b| {
    b.iter(|| black_box(ass::parse_content(&content).unwrap()))
  });
}

fn bench_ass_stringify_small(c: &mut Criterion) {
  let content = small_ass();
  let file = ass::parse_content(&content).unwrap();
  c.bench_function("ass_stringify_small", |b| {
    b.iter(|| black_box(file.to_string()))
  });
}

fn bench_ass_stringify_large(c: &mut Criterion) {
  let content = large_ass(1000);
  let file = ass::parse_content(&content).unwrap();
  c.bench_function("ass_stringify_large", |b| {
    b.iter(|| black_box(file.to_string()))
  });
}

fn bench_ass_detect_format(c: &mut Criterion) {
  let content = small_ass();
  c.bench_function("ass_detect_format", |b| {
    b.iter(|| black_box(ass::detect_format(content.as_bytes())))
  });
}

// ── Top-level detect_format ──

fn bench_detect_format_srt(c: &mut Criterion) {
  let content = small_srt();
  c.bench_function("detect_format_srt", |b| {
    b.iter(|| black_box(subtitler::detect_format(content.as_bytes())))
  });
}

fn bench_detect_format_vtt(c: &mut Criterion) {
  let content = small_vtt();
  c.bench_function("detect_format_vtt", |b| {
    b.iter(|| black_box(subtitler::detect_format(content.as_bytes())))
  });
}

fn bench_detect_format_ass(c: &mut Criterion) {
  let content = small_ass();
  c.bench_function("detect_format_ass", |b| {
    b.iter(|| black_box(subtitler::detect_format(content.as_bytes())))
  });
}

// ── Model/Utility Benchmarks ──

fn bench_subtitle_sort(c: &mut Criterion) {
  let make_file = || {
    SubtitleFile::Srt(
      (0..1000)
        .map(|i| Subtitle::new((1000 - i) as u64 * 100, (1000 - i + 1) as u64 * 100, "test"))
        .collect(),
    )
  };
  c.bench_function("subtitle_sort_1000", |b| {
    b.iter(|| {
      let mut f = make_file();
      f.sort();
      black_box(f)
    })
  });
}

fn bench_subtitle_validate_clean(c: &mut Criterion) {
  let file = SubtitleFile::Srt(
    (0..1000)
      .map(|i| Subtitle::new(i as u64 * 3000, (i as u64 * 3000) + 2500, "test"))
      .collect(),
  );
  c.bench_function("subtitle_validate_clean_1000", |b| {
    b.iter(|| black_box(file.validate()))
  });
}

fn bench_subtitle_validate_extended(c: &mut Criterion) {
  let file = SubtitleFile::Srt(
    (0..100)
      .map(|i| {
        Subtitle::new(
          i as u64 * 3000,
          (i as u64 * 3000) + 2500,
          "A subtitle with some longer text for CPS calculation benchmarks",
        )
      })
      .collect(),
  );
  c.bench_function("subtitle_validate_extended_100", |b| {
    b.iter(|| black_box(file.validate_extended(42, 5000, 25.0)))
  });
}

fn bench_subtitle_merge_adjacent(c: &mut Criterion) {
  let file = || {
    SubtitleFile::Srt(
      (0..100)
        .map(|i| {
          Subtitle::new(
            i as u64 * 2500,
            (i as u64 * 2500) + 2400,
            "merge test",
          )
        })
        .collect(),
    )
  };
  c.bench_function("subtitle_merge_adjacent_100", |b| {
    b.iter(|| {
      let mut f = file();
      f.merge_adjacent(200);
      black_box(f)
    })
  });
}

fn bench_subtitle_split_long(c: &mut Criterion) {
  let file = SubtitleFile::Srt(vec![
    Subtitle::new(
      0,
      10000,
      "This is a very long subtitle that will be split into multiple smaller chunks based on word boundaries to test the splitting performance with a moderate amount of text content for the benchmark",
    ),
  ]);
  c.bench_function("subtitle_split_long", |b| {
    b.iter(|| {
      let mut f = file.clone();
      f.split_long(20);
      black_box(f)
    })
  });
}

fn bench_subtitle_shift_all(c: &mut Criterion) {
  let make_file = || {
    SubtitleFile::Srt(
      (0..1000)
        .map(|i| Subtitle::new(i as u64 * 3000, (i as u64 * 3000) + 2500, "test"))
        .collect(),
    )
  };
  c.bench_function("subtitle_shift_all_1000", |b| {
    b.iter(|| {
      let mut f = make_file();
      f.shift_all(100);
      black_box(f)
    })
  });
}

fn bench_subtitle_transform_framerate(c: &mut Criterion) {
  let make_file = || {
    SubtitleFile::Srt(
      (0..1000)
        .map(|i| Subtitle::new(i as u64 * 3000, (i as u64 * 3000) + 2500, "test"))
        .collect(),
    )
  };
  c.bench_function("subtitle_transform_framerate_1000", |b| {
    b.iter(|| {
      let mut f = make_file();
      f.transform_framerate(23.976, 25.0);
      black_box(f)
    })
  });
}

fn bench_subtitle_map(c: &mut Criterion) {
  let file = || {
    SubtitleFile::Srt(
      (0..1000)
        .map(|i| Subtitle::new(i as u64 * 3000, (i as u64 * 3000) + 2500, "hello world"))
        .collect(),
    )
  };
  c.bench_function("subtitle_map_1000", |b| {
    b.iter(|| {
      let f = file();
      black_box(f.map(|sub| { sub.text = sub.text.to_uppercase(); }))
    })
  });
}

fn bench_subtitle_filter(c: &mut Criterion) {
  let file = || {
    SubtitleFile::Srt(
      (0..1000)
        .map(|i| {
          let text = if i % 2 == 0 { "keep" } else { "drop" };
          Subtitle::new(i as u64 * 3000, (i as u64 * 3000) + 2500, text)
        })
        .collect(),
    )
  };
  c.bench_function("subtitle_filter_1000", |b| {
    b.iter(|| {
      let f = file();
      black_box(f.filter(|sub| sub.text == "keep"))
    })
  });
}

// ── Format conversion round-trip ──

fn bench_srt_to_vtt_convert(c: &mut Criterion) {
  let content = large_srt(100);
  let rt = tokio::runtime::Runtime::new().unwrap();
  let subs = rt.block_on(async { srt::parse_content(&content).await.unwrap() });
  c.bench_function("srt_to_vtt_convert_100", |b| {
    b.iter(|| black_box(vtt::to_string(&subs, None)))
  });
}

fn bench_srt_to_ass_convert(c: &mut Criterion) {
  let content = large_srt(100);
  let rt = tokio::runtime::Runtime::new().unwrap();
  let subs = rt.block_on(async { srt::parse_content(&content).await.unwrap() });
  c.bench_function("srt_to_ass_convert_100", |b| {
    b.iter(|| {
      black_box(ass::to_string(
        &Default::default(),
        &[subtitler::model::AssStyle::default_style()],
        &subs,
      ))
    })
  });
}

fn bench_regex_hotspots(c: &mut Criterion) {
  let mut group = c.benchmark_group("regex_hotspots");

  let tagged = "<b>Bold</b> <i>italic</i> <u>under</u> <font color=\"#ff0000\">red</font> plain tail";
  let sub = subtitler::model::Subtitle::new(0, 1000, tagged);

  group.bench_function("plaintext", |b| {
    b.iter(|| {
      black_box(sub.plaintext());
    });
  });

  group.bench_function("strip_tags", |b| {
    b.iter(|| {
      let mut s = sub.clone();
      s.strip_tags();
      black_box(s.text);
    });
  });

  let noisy = "12O456 and 1l0 with w0rd plus somern";
  group.bench_function("fix_ocr_errors", |b| {
    b.iter(|| {
      black_box(subtitler::normalize::fix_ocr_errors(noisy));
    });
  });

  group.finish();
}

criterion_group!(
  benches,
  // utility
  bench_parse_timestamp,
  bench_parse_timestamps,
  bench_pad_left,
  bench_format_timestamp_srt,
  bench_format_timestamp_vtt,
  bench_frame_ms_roundtrip,
  // srt parse/stringify
  bench_srt_parse_small,
  bench_srt_parse_large,
  bench_srt_stringify_small,
  bench_srt_stringify_large,
  bench_srt_detect_format,
  // vtt parse/stringify
  bench_vtt_parse_small,
  bench_vtt_parse_large,
  bench_vtt_stringify_small,
  bench_vtt_stringify_large,
  bench_vtt_detect_format,
  // ass parse/stringify
  bench_ass_parse_small,
  bench_ass_parse_large,
  bench_ass_stringify_small,
  bench_ass_stringify_large,
  bench_ass_detect_format,
  // detect
  bench_detect_format_srt,
  bench_detect_format_vtt,
  bench_detect_format_ass,
  // model operations
  bench_subtitle_sort,
  bench_subtitle_validate_clean,
  bench_subtitle_validate_extended,
  bench_subtitle_merge_adjacent,
  bench_subtitle_split_long,
  bench_subtitle_shift_all,
  bench_subtitle_transform_framerate,
  bench_subtitle_map,
  bench_subtitle_filter,
  // conversion
  bench_srt_to_vtt_convert,
  bench_srt_to_ass_convert,
  // regex hotspots (perf regression tracking)
  bench_regex_hotspots,
);
criterion_main!(benches);
