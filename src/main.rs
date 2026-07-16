mod cli;
mod types;

use crate::types::AnyResult;
use clap::Parser;
use cli::{Commands, Format as CliFormat};
#[cfg(feature = "ass")]
use subtitler::ass;
use subtitler::model::{Format, SubtitleFile, SubtitleFormat};
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
  match subtitler::detect_format(data) {
    #[cfg(feature = "srt")]
    Some(Format::Srt) => Some(CliFormat::Srt),
    #[cfg(feature = "vtt")]
    Some(Format::Vtt) => Some(CliFormat::Vtt),
    #[cfg(feature = "ass")]
    Some(Format::Ass) => Some(CliFormat::Ass),
    #[cfg(feature = "ssa")]
    Some(Format::Ssa) => Some(CliFormat::Ssa),
    #[cfg(feature = "microdvd")]
    Some(Format::MicroDvd) => Some(CliFormat::MicroDvd),
    #[cfg(feature = "subviewer")]
    Some(Format::SubViewer) => Some(CliFormat::SubViewer),
    #[cfg(feature = "ttml")]
    Some(Format::Ttml) => Some(CliFormat::Ttml),
    #[cfg(feature = "sbv")]
    Some(Format::Sbv) => Some(CliFormat::Sbv),
    #[cfg(feature = "lrc")]
    Some(Format::Lrc) => Some(CliFormat::Lrc),
    None => None,
  }
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

#[allow(deprecated)]
async fn parse_to_file(data: &[u8], format: CliFormat) -> AnyResult<SubtitleFile> {
  let text = String::from_utf8(data.to_vec())?;
  match format {
    #[cfg(feature = "srt")]
    CliFormat::Srt => {
      let subs = srt::parse_content(&text)?;
      Ok(SubtitleFile::Srt(subs))
    }
    #[cfg(feature = "vtt")]
    CliFormat::Vtt => {
      let (header, subs) = vtt::parse_content_full(&text)?;
      Ok(SubtitleFile::Vtt {
        header,
        subtitles: subs,
      })
    }
    #[cfg(feature = "ass")]
    CliFormat::Ass => ass::parse_content(&text),
    #[cfg(feature = "ssa")]
    CliFormat::Ssa => ass::parse_content(&text),
    #[cfg(feature = "microdvd")]
    CliFormat::MicroDvd => {
      let file = subtitler::microdvd::parse_content(&text, None)?;
      Ok(file)
    }
    #[cfg(feature = "subviewer")]
    CliFormat::SubViewer => subtitler::subviewer::parse_content(&text),
    #[cfg(feature = "ttml")]
    CliFormat::Ttml => {
      let subs = subtitler::ttml::parse_content(&text)?;
      Ok(SubtitleFile::Ttml {
        header: None,
        subtitles: subs,
      })
    }
    #[cfg(feature = "sbv")]
    CliFormat::Sbv => Ok(SubtitleFile::Sbv(subtitler::sbv::parse_content(&text)?)),
    #[cfg(feature = "lrc")]
    CliFormat::Lrc => Ok(SubtitleFile::Lrc(subtitler::lrc::parse_content(&text)?)),
  }
}

// ── Commands ──

#[allow(deprecated)]
async fn cmd_parse(args: cli::ParseArgs) -> AnyResult<()> {
  let (data, ext) = read_input(&args.input).await?;
  let format = resolve_format(&data, args.format.or(ext))
    .ok_or_else(|| anyhow::anyhow!("Cannot detect subtitle format. Use --format to specify."))?;

  let content = String::from_utf8(data.to_vec())?;
  let subs = match format {
    #[cfg(feature = "srt")]
    CliFormat::Srt => srt::parse_content(&content)?,
    #[cfg(feature = "vtt")]
    CliFormat::Vtt => vtt::parse_content(&content)?,
    #[cfg(feature = "ass")]
    CliFormat::Ass => ass::parse_content(&content)?.subtitles().to_vec(),
    #[cfg(feature = "ssa")]
    CliFormat::Ssa => ass::parse_content(&content)?.subtitles().to_vec(),
    #[cfg(feature = "microdvd")]
    CliFormat::MicroDvd => subtitler::microdvd::parse_content(&content, None)?
      .subtitles()
      .to_vec(),
    #[cfg(feature = "subviewer")]
    CliFormat::SubViewer => subtitler::subviewer::parse_content(&content)?
      .subtitles()
      .to_vec(),
    #[cfg(feature = "ttml")]
    CliFormat::Ttml => subtitler::ttml::parse_content(&content)?,
    #[cfg(feature = "sbv")]
    CliFormat::Sbv => subtitler::sbv::parse_content(&content)?,
    #[cfg(feature = "lrc")]
    CliFormat::Lrc => subtitler::lrc::parse_content(&content)?,
  };

  if args.json {
    println!("{}", serde_json::to_string_pretty(&subs)?);
  } else {
    for (i, sub) in subs.iter().enumerate() {
      println!(
        "[{}] {:0>2}:{:0>2}:{:0>2},{:0>3} --> {:0>2}:{:0>2}:{:0>2},{:0>3}",
        i + 1,
        sub.start / 3600000,
        (sub.start % 3600000) / 60000,
        (sub.start % 60000) / 1000,
        sub.start % 1000,
        sub.end / 3600000,
        (sub.end % 3600000) / 60000,
        (sub.end % 60000) / 1000,
        sub.end % 1000,
      );
      println!("{}\n", sub.text);
    }
    eprintln!("{} subtitles parsed (format: {})", subs.len(), format);
  }
  Ok(())
}

async fn cmd_convert(args: cli::ConvertArgs) -> AnyResult<()> {
  let (data, ext) = read_input(&args.input).await?;
  let from = resolve_format(&data, args.from.or(ext))
    .ok_or_else(|| anyhow::anyhow!("Cannot detect source format. Use --from to specify."))?;
  let to = resolve_output_format(&args.output, args.to)?;

  let mut file = parse_to_file(&data, from).await?;

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

  let issues = if args.basic {
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
  }
}

fn format_to_subtitle_format(f: &CliFormat) -> Format {
  match f {
    #[cfg(feature = "srt")]
    CliFormat::Srt => Format::Srt,
    #[cfg(feature = "vtt")]
    CliFormat::Vtt => Format::Vtt,
    #[cfg(feature = "ass")]
    CliFormat::Ass => Format::Ass,
    #[cfg(feature = "ssa")]
    CliFormat::Ssa => Format::Ssa,
    #[cfg(feature = "microdvd")]
    CliFormat::MicroDvd => Format::MicroDvd,
    #[cfg(feature = "subviewer")]
    CliFormat::SubViewer => Format::SubViewer,
    #[cfg(feature = "ttml")]
    CliFormat::Ttml => Format::Ttml,
    #[cfg(feature = "sbv")]
    CliFormat::Sbv => Format::Sbv,
    #[cfg(feature = "lrc")]
    CliFormat::Lrc => Format::Lrc,
  }
}

async fn cmd_edit(args: cli::EditArgs) -> AnyResult<()> {
  let (data, ext) = read_input(&args.input).await?;
  let from = resolve_format(&data, args.from.or(ext))
    .ok_or_else(|| anyhow::anyhow!("Cannot detect source format. Use --from to specify."))?;
  let to = resolve_output_format(&args.output, args.to).unwrap_or(from.clone());
  let target_fmt = format_to_subtitle_format(&to);

  let mut file = parse_to_file(&data, from).await?;

  let mut ops = 0;

  if args.sort {
    file.sort();
    ops += 1;
  }

  if let Some(ms) = args.shift {
    file.shift_all(ms);
    ops += 1;
  }

  if let Some(gap) = args.merge {
    file.merge_adjacent(gap);
    ops += 1;
  }

  if let Some(max_chars) = args.split {
    file.split_long(max_chars);
    ops += 1;
  }

  if let Some(fps_pair) = args.transform_fps {
    // clap enforces number_of_values = 2, so fps_pair.len() is always 2
    file.transform_framerate(fps_pair[0], fps_pair[1]);
    ops += 1;
  }

  if ops == 0 {
    anyhow::bail!(
      "No edit operations specified. Use --sort, --shift, --merge, --split, or --transform-fps."
    );
  }
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
  let min_dur = durations.iter().min().unwrap();
  let max_dur = durations.iter().max().unwrap();
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

  for sub in file.subtitles_mut() {
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
