mod cli;
mod types;

use crate::types::AnyResult;
use clap::Parser;
use cli::{Commands, Format as CliFormat};
#[cfg(feature = "ass")]
use subtitler::ass;
#[cfg(feature = "ebu_stl")]
use subtitler::ebu_stl;
use subtitler::model::convert::{MS_PER_HOUR, MS_PER_MINUTE, MS_PER_SECOND};
use subtitler::model::{Format, SubtitleFile, SubtitleFormat};
use subtitler::pipeline::{Pipeline, SubtitleBuilder};
#[cfg(feature = "scc")]
use subtitler::scc;
#[cfg(feature = "srt")]
use subtitler::srt;
#[cfg(feature = "vtt")]
use subtitler::vtt;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main(flavor = "current_thread")]
async fn main() -> AnyResult<()> {
  let subscriber = FmtSubscriber::builder()
    .with_max_level(Level::WARN)
    .finish();
  tracing::subscriber::set_global_default(subscriber)
    .unwrap_or_else(|e| eprintln!("warning: could not set tracing subscriber: {}", e));

  let cli = cli::Cli::parse();

  match cli.command {
    Commands::Parse(args) => cmd_parse(args).await?,
    Commands::Convert(args) => cmd_convert(args).await?,
    Commands::Validate(args) => cmd_validate(args).await?,
    Commands::Edit(args) => cmd_edit(args).await?,
    Commands::Pipeline(args) => cmd_pipeline(args).await?,
    Commands::Info(args) => cmd_info(args).await?,
    Commands::Detect(args) => cmd_detect(args).await?,
    Commands::Quality(args) => cmd_quality(args).await?,
    Commands::Normalize(args) => cmd_normalize(args).await?,
    Commands::Shift(args) => cmd_shift(args).await?,
  }

  Ok(())
}

// ── Input helpers ──

async fn read_input(input: &str) -> AnyResult<(Vec<u8>, Option<CliFormat>)> {
  let ext_format = CliFormat::from_ext(input);

  if input == "-" {
    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut std::io::stdin(), &mut buf)?;
    return Ok((buf, None));
  }

  if input.starts_with("http://") || input.starts_with("https://") {
    #[cfg(feature = "http")]
    {
      let resp = reqwest::get(input).await?;
      let bytes = resp.bytes().await?;
      return Ok((bytes.to_vec(), ext_format));
    }
    #[cfg(not(feature = "http"))]
    {
      anyhow::bail!("HTTP support requires the `http` feature. Rebuild with default features.");
    }
  }

  let data = tokio::fs::read(input).await?;
  Ok((data, ext_format))
}

fn resolve_format(data: &[u8], hint: Option<CliFormat>) -> Option<CliFormat> {
  if let Some(f) = hint {
    return Some(f);
  }
  // 通过 cli::Format 的 From<&model::Format> 实现转换，避免在 main.rs 重复维护 match
  subtitler::detect_format(data).map(|f| CliFormat::from(&f))
}

fn resolve_output_format(output: &str, hint: Option<CliFormat>) -> AnyResult<CliFormat> {
  if let Some(f) = hint {
    return Ok(f);
  }
  CliFormat::from_ext(output).ok_or_else(|| {
    anyhow::anyhow!(
      "Cannot determine output format from '{}'. Use --to to specify.",
      output
    )
  })
}

async fn parse_to_file(data: &[u8], format: CliFormat) -> AnyResult<SubtitleFile> {
  // Binary formats (EBU STL) take raw bytes; skip text decoding which
  // would fail on arbitrary binary content with InvalidEncoding.
  #[cfg(feature = "ebu_stl")]
  if matches!(format, CliFormat::EbuStl) {
    return ebu_stl::parse_content(data);
  }

  let text = subtitler::encoding::decode_to_string(data)?;
  match format {
    #[cfg(feature = "srt")]
    CliFormat::Srt => srt::parse_content(&text),
    #[cfg(feature = "vtt")]
    CliFormat::Vtt => vtt::parse_content(&text),
    #[cfg(feature = "ass")]
    CliFormat::Ass => Ok(ass::parse_content(&text)?),
    #[cfg(feature = "ssa")]
    CliFormat::Ssa => Ok(ass::parse_content(&text)?),
    #[cfg(feature = "microdvd")]
    CliFormat::MicroDvd => Ok(subtitler::microdvd::parse_content(&text, None)?),
    #[cfg(feature = "subviewer")]
    CliFormat::SubViewer => Ok(subtitler::subviewer::parse_content(&text)?),
    #[cfg(feature = "ttml")]
    CliFormat::Ttml => subtitler::ttml::parse_content(&text),
    #[cfg(feature = "sbv")]
    CliFormat::Sbv => subtitler::sbv::parse_content(&text),
    #[cfg(feature = "lrc")]
    CliFormat::Lrc => Ok(subtitler::lrc::parse_content(&text)?),
    #[cfg(feature = "sami")]
    CliFormat::Sami => Ok(subtitler::sami::parse_content(&text)?),
    #[cfg(feature = "mpl2")]
    CliFormat::Mpl2 => Ok(subtitler::mpl2::parse_content(&text)?),
    #[cfg(feature = "scc")]
    CliFormat::Scc => Ok(scc::parse_content(&text)?),
    #[cfg(feature = "ebu_stl")]
    CliFormat::EbuStl => unreachable!("EBU STL handled above to skip text decoding"),
    #[cfg(feature = "spruce")]
    CliFormat::Spruce => Ok(subtitler::spruce::parse_content(&text, None)?),
    #[cfg(feature = "dfxp")]
    CliFormat::Dfxp => Ok(subtitler::dfxp::parse_content(&text)?),
    #[cfg(feature = "itt")]
    CliFormat::Itt => Ok(subtitler::itt::parse_content(&text)?),
    #[cfg(feature = "whisper")]
    CliFormat::Whisper => Ok(subtitler::whisper::parse_content(&text)?),
  }
}

// ── Commands ──

/// Parse text-based formats. EBU STL (binary) is handled separately in
/// the callers to skip `decode_to_string`. See `cmd_parse` and `parse_to_file`.
fn cmd_parse_text(data: &[u8], format: CliFormat) -> AnyResult<SubtitleFile> {
  let content = subtitler::encoding::decode_to_string(data)?;
  let file = match format {
    #[cfg(feature = "srt")]
    CliFormat::Srt => srt::parse_content(&content)?,
    #[cfg(feature = "vtt")]
    CliFormat::Vtt => vtt::parse_content(&content)?,
    #[cfg(feature = "ass")]
    CliFormat::Ass => ass::parse_content(&content)?,
    #[cfg(feature = "ssa")]
    CliFormat::Ssa => ass::parse_content(&content)?,
    #[cfg(feature = "microdvd")]
    CliFormat::MicroDvd => subtitler::microdvd::parse_content(&content, None)?,
    #[cfg(feature = "subviewer")]
    CliFormat::SubViewer => subtitler::subviewer::parse_content(&content)?,
    #[cfg(feature = "ttml")]
    CliFormat::Ttml => subtitler::ttml::parse_content(&content)?,
    #[cfg(feature = "sbv")]
    CliFormat::Sbv => subtitler::sbv::parse_content(&content)?,
    #[cfg(feature = "lrc")]
    CliFormat::Lrc => subtitler::lrc::parse_content(&content)?,
    #[cfg(feature = "sami")]
    CliFormat::Sami => subtitler::sami::parse_content(&content)?,
    #[cfg(feature = "mpl2")]
    CliFormat::Mpl2 => subtitler::mpl2::parse_content(&content)?,
    #[cfg(feature = "scc")]
    CliFormat::Scc => subtitler::scc::parse_content(&content)?,
    #[cfg(feature = "ebu_stl")]
    CliFormat::EbuStl => unreachable!("EBU STL is binary; handled by callers"),
    #[cfg(feature = "spruce")]
    CliFormat::Spruce => subtitler::spruce::parse_content(&content, None)?,
    #[cfg(feature = "dfxp")]
    CliFormat::Dfxp => subtitler::dfxp::parse_content(&content)?,
    #[cfg(feature = "itt")]
    CliFormat::Itt => subtitler::itt::parse_content(&content)?,
    #[cfg(feature = "whisper")]
    CliFormat::Whisper => subtitler::whisper::parse_content(&content)?,
  };
  Ok(file)
}

async fn cmd_parse(args: cli::ParseArgs) -> AnyResult<()> {
  let (data, ext) = read_input(&args.input).await?;
  let format = resolve_format(&data, args.format.or(ext))
    .ok_or_else(|| anyhow::anyhow!("Cannot detect subtitle format. Use --format to specify."))?;

  // Binary formats (EBU STL) take raw bytes; skip text decoding which
  // would fail on arbitrary binary content with InvalidEncoding.
  // `format` is cloned into cmd_parse_text because it's still needed
  // for the eprintln! at the end of this function.
  #[cfg(feature = "ebu_stl")]
  let file = if matches!(format, CliFormat::EbuStl) {
    subtitler::ebu_stl::parse_bytes(&data)?
  } else {
    cmd_parse_text(&data, format.clone())?
  };
  #[cfg(not(feature = "ebu_stl"))]
  let file = cmd_parse_text(&data, format.clone())?;
  let subs = file.subtitles();

  if args.json {
    println!("{}", serde_json::to_string_pretty(&subs)?);
  } else {
    for (i, sub) in subs.iter().enumerate() {
      println!(
        "[{}] {:0>2}:{:0>2}:{:0>2},{:0>3} --> {:0>2}:{:0>2}:{:0>2},{:0>3}",
        i + 1,
        sub.start / MS_PER_HOUR,
        (sub.start % MS_PER_HOUR) / MS_PER_MINUTE,
        (sub.start % MS_PER_MINUTE) / MS_PER_SECOND,
        sub.start % MS_PER_SECOND,
        sub.end / MS_PER_HOUR,
        (sub.end % MS_PER_HOUR) / MS_PER_MINUTE,
        (sub.end % MS_PER_MINUTE) / MS_PER_SECOND,
        sub.end % 1000,
      );
      println!("{}\n", sub.text);
    }
    eprintln!("{} subtitles parsed (format: {})", subs.len(), format);
    if subs.is_empty() {
      eprintln!(
        "warning: parsing returned 0 subtitles — the file may be empty, malformed, or in a different format. Try '--format' to force a specific format, or 'detect' to check format detection."
      );
    }
  }
  Ok(())
}

async fn cmd_convert(args: cli::ConvertArgs) -> AnyResult<()> {
  let (data, ext) = read_input(&args.input).await?;
  let from = resolve_format(&data, args.from.or(ext))
    .ok_or_else(|| anyhow::anyhow!("Cannot detect source format. Use --from to specify."))?;
  let to = resolve_output_format(&args.output, args.to)?;

  let mut file = if args.from_words {
    #[cfg(feature = "whisper")]
    {
      if !matches!(from, CliFormat::Whisper) {
        anyhow::bail!("--from-words requires Whisper JSON input.");
      }
      let text = subtitler::encoding::decode_to_string(&data)?;
      subtitler::whisper::parse_content_as_words(
        &text,
        &subtitler::whisper::WordGroupingOptions::default(),
      )?
    }
    #[cfg(not(feature = "whisper"))]
    {
      anyhow::bail!("--from-words requires the `whisper` feature.");
    }
  } else {
    parse_to_file(&data, from).await?
  };

  if let Some(shift) = args.shift {
    file.shift_all(shift);
  }

  let target_fmt = format_to_subtitle_format(&to);
  let output = file.to_string_with_format(&target_fmt);

  if args.output == "-" {
    print!("{output}");
  } else {
    tokio::fs::write(&args.output, &output).await?;
    eprintln!("Converted: {} -> {} ({})", args.input, args.output, to);
  }
  Ok(())
}

async fn cmd_validate(args: cli::ValidateArgs) -> AnyResult<()> {
  let (data, ext) = read_input(&args.input).await?;
  let format =
    resolve_format(&data, ext).ok_or_else(|| anyhow::anyhow!("Cannot detect subtitle format."))?;
  let file = parse_to_file(&data, format).await?;

  let subs = file.subtitles();

  let issues = if let Some(preset) = args.guideline {
    let guideline = subtitler::guidelines::GuidelinePreset::from(&preset).guideline();
    eprintln!(
      "Guideline preset: {} (max {} chars/line, {} lines, {}–{}ms duration, ≥{}ms gap, ≤{:.0} CPS)",
      guideline.name,
      guideline.max_chars_per_line,
      guideline.max_lines,
      guideline.min_duration_ms,
      guideline.max_duration_ms,
      guideline.min_gap_ms,
      guideline.max_cps
    );
    file.validate_guideline(&guideline)
  } else if args.basic {
    file.validate()
  } else {
    file.validate_extended(args.max_chars, args.max_gap, args.max_cps)
  };

  if issues.is_empty() {
    println!("No issues found in {} subtitles.", subs.len());
    return Ok(());
  }

  if args.json {
    #[derive(serde::Serialize)]
    struct Issue {
      kind: &'static str,
      description: String,
    }
    let json_issues: Vec<Issue> = issues
      .iter()
      .map(|i| Issue {
        kind: issue_kind(i),
        description: i.description(),
      })
      .collect();
    println!("{}", serde_json::to_string_pretty(&json_issues)?);
  } else {
    println!(
      "Found {} issues in {} subtitles:\n",
      issues.len(),
      subs.len()
    );
    for issue in &issues {
      println!("  [{}] {}", issue_kind(issue), issue.description());
    }
  }

  if !issues.is_empty() {
    std::process::exit(1);
  }
  Ok(())
}

fn issue_kind(issue: &subtitler::model::ValidationIssue) -> &'static str {
  use subtitler::model::ValidationIssue::*;
  match issue {
    Overlap { .. } => "OVERLAP",
    NegativeDuration { .. } => "NEG_DUR",
    ZeroDuration { .. } => "ZERO_DUR",
    DecreasingStartTime { .. } => "DECR_START",
    TooLongGap { .. } => "LONG_GAP",
    TextTooLong { .. } => "LONG_TEXT",
    CpsTooHigh { .. } => "HIGH_CPS",
    TooShortDuration { .. } => "SHORT_DUR",
    TooLongDuration { .. } => "LONG_DUR",
    TooShortGap { .. } => "SHORT_GAP",
    LineCountExceeded { .. } => "MANY_LINES",
  }
}

fn format_to_subtitle_format(f: &CliFormat) -> Format {
  // 通过 cli::Format 上实现的 Into<model::Format> 转换
  f.into()
}

fn parse_timebase(s: &str) -> AnyResult<subtitler::model::Timebase> {
  use subtitler::model::Timebase;
  let lower = s.trim().to_lowercase();
  if lower == "29.97df" || lower == "df29.97" {
    return Ok(Timebase::Df2997);
  }
  if lower == "59.94df" || lower == "df59.94" {
    return Ok(Timebase::Df5994);
  }
  let digits = lower.strip_suffix("ndf").unwrap_or(&lower);
  let fps: f64 = digits.parse().map_err(|_| {
    anyhow::anyhow!(
      "Invalid timebase '{}'. Use e.g. 23.976, 25, 29.97ndf, 29.97df, or 59.94df.",
      s
    )
  })?;
  if fps <= 0.0 {
    anyhow::bail!("Timebase FPS must be positive, got '{}'.", s);
  }
  Ok(Timebase::Ndf(fps))
}

async fn cmd_edit(args: cli::EditArgs) -> AnyResult<()> {
  let (data, ext) = read_input(&args.input).await?;
  let from = resolve_format(&data, args.from.or(ext))
    .ok_or_else(|| anyhow::anyhow!("Cannot detect source format. Use --from to specify."))?;
  let to = resolve_output_format(&args.output, args.to).unwrap_or(from.clone());
  let target_fmt = format_to_subtitle_format(&to);

  let file = parse_to_file(&data, from).await?;

  let mut builder = SubtitleBuilder::from(file);
  let mut ops = 0;

  if args.sort {
    builder = builder.sort();
    ops += 1;
  }
  if let Some(ms) = args.shift {
    builder = builder.shift(ms);
    ops += 1;
  }
  if let Some(gap) = args.merge {
    builder = builder.merge_adjacent(gap);
    ops += 1;
  }
  if let Some(max_chars) = args.split {
    builder = builder.split_long(max_chars);
    ops += 1;
  }
  if let Some(fps_pair) = args.transform_fps {
    builder = builder.transform_fps(fps_pair[0], fps_pair[1]);
    ops += 1;
  }
  if let Some(fps) = args.snap_to_frames {
    builder = builder.snap_to_frames(fps);
    ops += 1;
  }
  if let Some(pair) = args.reinterpret_timebase {
    if pair.len() == 2 {
      let from = parse_timebase(&pair[0])?;
      let to = parse_timebase(&pair[1])?;
      builder = builder.reinterpret_framerate(from, to);
      ops += 1;
    }
  }
  if args.convert_rollup {
    builder = builder.convert_rollup();
    ops += 1;
  }
  if let Some(edl_path) = args.shot_changes {
    let fps = args.fps.unwrap_or(25.0);
    let (data, _) = read_input(&edl_path).await?;
    let cuts = subtitler::shotlist::parse_edl_cuts(&data, fps)?;
    if cuts.is_empty() {
      anyhow::bail!(
        "No shot changes found in '{}'. Is it a CMX3600 EDL?",
        edl_path
      );
    }
    eprintln!(
      "Shot changes: {} cuts from {} (before {}f, after {}f @ {:.3} fps)",
      cuts.len(),
      edl_path,
      args.before_frames,
      args.after_frames,
      fps
    );
    builder = builder.apply_shot_changes(&cuts, args.before_frames, args.after_frames, fps);
    ops += 1;
  }

  if ops == 0 {
    anyhow::bail!(
      "No edit operations specified. Use --sort, --shift, --merge, --split, --transform-fps, --snap-to-frames, --reinterpret-timebase, --convert-rollup, or --shot-changes."
    );
  }

  let file = builder.build();
  let output = file.to_string_with_format(&target_fmt);

  if args.output == "-" {
    print!("{output}");
  } else {
    tokio::fs::write(&args.output, &output).await?;
    eprintln!(
      "Applied {} operation(s): {} -> {} ({})",
      ops, args.input, args.output, to
    );
  }
  Ok(())
}

async fn cmd_pipeline(args: cli::PipelineArgs) -> AnyResult<()> {
  let (data, ext) = read_input(&args.input).await?;
  let from = resolve_format(&data, args.from.or(ext))
    .ok_or_else(|| anyhow::anyhow!("Cannot detect source format. Use --from to specify."))?;
  let to = resolve_output_format(&args.output, args.to).unwrap_or(from.clone());
  let target_fmt = format_to_subtitle_format(&to);

  let config = tokio::fs::read_to_string(&args.config).await?;
  let pipeline: Pipeline = serde_json::from_str(&config)
    .map_err(|e| anyhow::anyhow!("Invalid pipeline config '{}': {}", args.config, e))?;

  if pipeline.operations.is_empty() {
    anyhow::bail!("Pipeline config '{}' contains no operations.", args.config);
  }

  let file = parse_to_file(&data, from).await?;
  let file = pipeline.apply(file);
  let output = file.to_string_with_format(&target_fmt);

  if args.output == "-" {
    print!("{output}");
  } else {
    tokio::fs::write(&args.output, &output).await?;
    eprintln!(
      "Pipeline applied: {} ops, {} -> {} ({})",
      pipeline.operations.len(),
      args.input,
      args.output,
      to,
    );
  }
  Ok(())
}

async fn cmd_info(args: cli::InfoArgs) -> AnyResult<()> {
  let (data, ext) = read_input(&args.input).await?;
  let format =
    resolve_format(&data, ext).ok_or_else(|| anyhow::anyhow!("Cannot detect subtitle format."))?;
  let file = parse_to_file(&data, format.clone()).await?;
  let subs = file.subtitles();

  if subs.is_empty() {
    println!("File: {}", args.input);
    println!("Format: {}", format);
    println!("Subtitles: 0");
    return Ok(());
  }

  let first = &subs[0];
  let last = &subs[subs.len() - 1];
  let total_duration = last.end.saturating_sub(first.start);
  let durations: Vec<u64> = subs.iter().map(|s| s.duration_ms()).collect();
  let avg_dur = durations.iter().sum::<u64>() / subs.len() as u64;
  let min_dur = durations.iter().min().copied().unwrap_or(0);
  let max_dur = durations.iter().max().copied().unwrap_or(0);
  let total_chars: usize = subs.iter().map(|s| s.text.chars().count()).sum();
  let max_cps = subs
    .iter()
    .map(|s| s.chars_per_second())
    .fold(0.0f64, f64::max);

  let validation = file.validate();

  println!("File:         {}", args.input);
  println!("Format:       {}", format);
  println!("Subtitles:    {}", subs.len());
  println!("Time range:   {}ms -> {}ms", first.start, last.end);
  println!(
    "Duration:     {}ms ({:.1}s)",
    total_duration,
    total_duration as f64 / 1000.0
  );
  println!("Avg duration: {}ms", avg_dur);
  println!("Min duration: {}ms", min_dur);
  println!("Max duration: {}ms", max_dur);
  println!("Total chars:  {}", total_chars);
  println!("Max CPS:      {:.1}", max_cps);
  println!("Timing issues: {}", validation.len());
  Ok(())
}

async fn cmd_detect(args: cli::DetectArgs) -> AnyResult<()> {
  let (data, _) = read_input(&args.input).await?;
  match subtitler::detect_format(&data) {
    #[cfg(feature = "srt")]
    Some(Format::Srt) => println!("srt"),
    #[cfg(feature = "vtt")]
    Some(Format::Vtt) => println!("vtt"),
    #[cfg(feature = "ass")]
    Some(Format::Ass) => println!("ass"),
    #[cfg(feature = "ssa")]
    Some(Format::Ssa) => println!("ssa"),
    #[cfg(feature = "microdvd")]
    Some(Format::MicroDvd) => println!("microdvd"),
    #[cfg(feature = "subviewer")]
    Some(Format::SubViewer) => println!("subviewer"),
    #[cfg(feature = "ttml")]
    Some(Format::Ttml) => println!("ttml"),
    #[cfg(feature = "sbv")]
    Some(Format::Sbv) => println!("sbv"),
    #[cfg(feature = "lrc")]
    Some(Format::Lrc) => println!("lrc"),
    #[cfg(feature = "sami")]
    Some(Format::Sami) => println!("sami"),
    #[cfg(feature = "mpl2")]
    Some(Format::Mpl2) => println!("mpl2"),
    #[cfg(feature = "scc")]
    Some(Format::Scc) => println!("scc"),
    #[cfg(feature = "ebu_stl")]
    Some(Format::EbuStl) => println!("ebu_stl"),
    #[cfg(feature = "spruce")]
    Some(Format::Spruce) => println!("spruce"),
    #[cfg(feature = "dfxp")]
    Some(Format::Dfxp) => println!("dfxp"),
    #[cfg(feature = "itt")]
    Some(Format::Itt) => println!("itt"),
    #[cfg(feature = "whisper")]
    Some(Format::Whisper) => println!("whisper"),
    None => {
      eprintln!("Unknown format");
      std::process::exit(1);
    }
  }
  Ok(())
}

async fn cmd_quality(args: cli::QualityArgs) -> AnyResult<()> {
  let (data, ext) = read_input(&args.input).await?;
  let format =
    resolve_format(&data, ext).ok_or_else(|| anyhow::anyhow!("Cannot detect subtitle format."))?;
  let file = parse_to_file(&data, format).await?;

  let report = subtitler::quality::generate_report(
    file.subtitles(),
    args.max_chars,
    args.max_gap,
    args.max_cps,
  );

  if args.json {
    println!("{}", serde_json::to_string_pretty(&report)?);
  } else {
    println!("=== Quality Report ===");
    println!("File:         {}", args.input);
    println!("Subtitles:    {}", report.total_subtitles);
    println!("Total issues: {}", report.total_issues);
    println!("Avg duration: {} ms", report.avg_duration_ms);
    println!("Avg CPS:      {:.1}", report.avg_cps);
    println!("Avg WPM:      {:.1}", report.avg_wpm);
    if let Some(worst) = report.subtitles.iter().max_by_key(|s| s.issues.len()) {
      if !worst.issues.is_empty() {
        println!(
          "Worst: subtitle #{} — {} issues",
          worst.index + 1,
          worst.issues.len()
        );
      }
    }
  }
  Ok(())
}

async fn cmd_normalize(args: cli::NormalizeArgs) -> AnyResult<()> {
  let (data, ext) = read_input(&args.input).await?;
  let format = resolve_format(&data, args.format.or(ext))
    .ok_or_else(|| anyhow::anyhow!("Cannot detect subtitle format. Use --format to specify."))?;
  let mut file = parse_to_file(&data, format).await?;

  let mut keep_languages: Vec<subtitler::normalize::Language> = Vec::new();
  if let Some(lang) = args.filter_language {
    keep_languages.push(subtitler::normalize::Language::from(&lang));
  }
  if let Some(lang) = args.second_language {
    keep_languages.push(subtitler::normalize::Language::from(&lang));
  }

  for sub in file.subtitles_mut() {
    if !keep_languages.is_empty() {
      sub.text = subtitler::normalize::remove_other_language_chars(&sub.text, &keep_languages);
    }
    if let Some(max) = args.merge_short_lines {
      sub.text = subtitler::normalize::merge_short_lines(&sub.text, max);
    }
    if args.remove_linebreaks {
      sub.text = subtitler::normalize::remove_all_newlines(&sub.text);
    }
    if args.linebreaks_to_pipe {
      sub.text = subtitler::normalize::replace_newlines(&sub.text, "|");
    }
    if args.fix_hyphens {
      sub.text = subtitler::normalize::fix_opening_hyphen_spacing(&sub.text);
    }
    if args.fix_caps {
      sub.text = subtitler::normalize::normalize_all_caps(&sub.text);
    }
    if let Some(between) = &args.remove_between {
      if between.len() == 2 {
        sub.text = subtitler::normalize::remove_text_between(&sub.text, &between[0], &between[1]);
      }
    }
    if args.all || args.fix_ocr {
      sub.text = subtitler::normalize::fix_ocr_errors(&sub.text);
    }
    if args.all || args.strip_hi {
      sub.text = subtitler::normalize::strip_hearing_impaired(&sub.text);
    }
    if args.all || args.quotes {
      sub.text = subtitler::normalize::normalize_quotes(&sub.text);
    }
    if args.all || args.whitespace {
      sub.text = subtitler::normalize::normalize_whitespace(&sub.text);
    }
  }

  let output = file.to_string();
  if args.output == "-" {
    print!("{output}");
  } else {
    tokio::fs::write(&args.output, &output).await?;
    eprintln!(
      "Wrote: {} ({} subtitles)",
      args.output,
      file.subtitles().len()
    );
  }
  Ok(())
}

async fn cmd_shift(args: cli::ShiftArgs) -> AnyResult<()> {
  let (data, ext) = read_input(&args.input).await?;
  let format = resolve_format(&data, args.format.or(ext))
    .ok_or_else(|| anyhow::anyhow!("Cannot detect subtitle format. Use --format to specify."))?;
  let mut file = parse_to_file(&data, format).await?;

  file.shift_all(args.offset);

  let output = file.to_string();
  if args.output == "-" {
    print!("{output}");
  } else {
    tokio::fs::write(&args.output, &output).await?;
    eprintln!(
      "Shifted by {} ms: {} -> {}",
      args.offset, args.input, args.output
    );
  }
  Ok(())
}
