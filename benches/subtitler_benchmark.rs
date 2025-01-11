use criterion::{black_box, criterion_group, criterion_main, Criterion};
use subtitler::utils::{format_timestamp, pad_left, parse_timestamp, parse_timestamps};

fn bench_parse_timestamp(c: &mut Criterion) {
  c.bench_function("parse_timestamp", |b| {
    b.iter(|| black_box(parse_timestamp("01:02:03.456").unwrap()))
  });
}

fn bench_parse_timestamps(c: &mut Criterion) {
  c.bench_function("parse_timestamps", |b| {
    b.iter(|| black_box(parse_timestamps("00:01:02.000 --> 00:01:05.000").unwrap()))
  });
}

fn bench_pad_left(c: &mut Criterion) {
  c.bench_function("pad_left", |b| b.iter(|| black_box(pad_left(5, 3))));
}

fn bench_format_timestamp(c: &mut Criterion) {
  c.bench_function("format_timestamp", |b| {
    b.iter(|| black_box(format_timestamp(3723456, "WebVTT")))
  });
}

criterion_group!(
  benches,
  bench_parse_timestamp,
  bench_parse_timestamps,
  bench_pad_left,
  bench_format_timestamp
);
criterion_main!(benches);
