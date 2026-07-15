use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, ValueEnum)]
pub enum Format {
  Srt,
  Vtt,
  Ass,
  Ssa,
  #[value(name = "microdvd")]
  MicroDvd,
  #[value(name = "subviewer")]
  SubViewer,
}

impl Format {
  pub fn from_ext(path: &str) -> Option<Self> {
    let lower = path.to_lowercase();
    if lower.ends_with(".srt") {
      Some(Format::Srt)
    } else if lower.ends_with(".vtt") {
      Some(Format::Vtt)
    } else if lower.ends_with(".ass") {
      Some(Format::Ass)
    } else if lower.ends_with(".ssa") {
      Some(Format::Ssa)
    } else if lower.ends_with(".sub") {
      Some(Format::MicroDvd)
    } else {
      None
    }
  }
}

impl std::fmt::Display for Format {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Format::Srt => write!(f, "srt"),
      Format::Vtt => write!(f, "vtt"),
      Format::Ass => write!(f, "ass"),
      Format::Ssa => write!(f, "ssa"),
      Format::MicroDvd => write!(f, "microdvd"),
      Format::SubViewer => write!(f, "subviewer"),
    }
  }
}

/// A CLI tool for parsing, converting, validating, and editing subtitles.
#[derive(Parser)]
#[command(name = "subtitler")]
#[command(
  about = "Subtitle toolkit: parse, convert, validate, and edit subtitles in SRT, WebVTT, and ASS/SSA formats."
)]
#[command(version)]
pub struct Cli {
  #[command(subcommand)]
  pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
  /// Parse and display subtitles
  Parse(ParseArgs),

  /// Convert subtitles between formats
  Convert(ConvertArgs),

  /// Validate subtitles for timing and text issues
  Validate(ValidateArgs),

  /// Edit subtitles: sort, shift, merge, split
  Edit(EditArgs),

  /// Show subtitle file statistics
  Info(InfoArgs),

  /// Detect subtitle format
  Detect(DetectArgs),
}

/// Parse a subtitle file or URL and display its contents.
#[derive(clap::Args)]
pub struct ParseArgs {
  /// Input file path (use "-" for stdin) or URL
  pub input: String,

  /// Force input format (auto-detected by default)
  #[arg(short, long)]
  pub format: Option<Format>,

  /// Output as JSON
  #[arg(short, long)]
  pub json: bool,
}

/// Convert between subtitle formats.
#[derive(clap::Args)]
pub struct ConvertArgs {
  /// Source file path (use "-" for stdin) or URL
  pub input: String,

  /// Destination file path (use "-" for stdout)
  pub output: String,

  /// Source format (auto-detect if not specified)
  #[arg(short, long)]
  pub from: Option<Format>,

  /// Target format (inferred from output extension if not specified)
  #[arg(short, long)]
  pub to: Option<Format>,

  /// FPS for frame-based timecode conversion
  #[arg(long)]
  pub fps: Option<f64>,

  /// Shift all timestamps by milliseconds (positive = delay, negative = advance)
  #[arg(long, allow_hyphen_values = true)]
  pub shift: Option<i64>,
}

/// Validate subtitle timing and text quality.
#[derive(clap::Args)]
pub struct ValidateArgs {
  /// Input file path or URL
  pub input: String,

  /// Maximum characters per subtitle line
  #[arg(long, default_value = "42")]
  pub max_chars: usize,

  /// Maximum gap between subtitles in milliseconds
  #[arg(long, default_value = "5000")]
  pub max_gap: u64,

  /// Maximum characters per second
  #[arg(long, default_value = "25.0")]
  pub max_cps: f64,

  /// Only show basic timing validation (no text checks)
  #[arg(long)]
  pub basic: bool,

  /// Output as JSON
  #[arg(short, long)]
  pub json: bool,
}

/// Edit subtitle timing and structure.
#[derive(clap::Args)]
pub struct EditArgs {
  /// Input file path or URL
  pub input: String,

  /// Output file path (required)
  #[arg(short, long)]
  pub output: String,

  /// Sort subtitles by start time
  #[arg(long)]
  pub sort: bool,

  /// Shift all timestamps by milliseconds (positive = delay, negative = advance)
  #[arg(long, allow_hyphen_values = true)]
  pub shift: Option<i64>,

  /// Merge adjacent subtitles (gap threshold in ms)
  #[arg(long)]
  pub merge: Option<u64>,

  /// Split long subtitles at word boundaries (max chars)
  #[arg(long)]
  pub split: Option<usize>,

  /// Transform framerate: FROM_FPS TO_FPS (e.g., "23.976 25.0")
  #[arg(long, value_names = &["FROM_FPS", "TO_FPS"], number_of_values = 2)]
  pub transform_fps: Option<Vec<f64>>,

  /// Force input format (auto-detect if not specified)
  #[arg(short, long)]
  pub from: Option<Format>,

  /// Target format (inferred from output extension if not specified)
  #[arg(short, long)]
  pub to: Option<Format>,
}

/// Show statistics about a subtitle file.
#[derive(clap::Args)]
pub struct InfoArgs {
  /// Input file path or URL
  pub input: String,
}

/// Detect the format of a subtitle file.
#[derive(clap::Args)]
pub struct DetectArgs {
  /// File path or URL
  pub input: String,
}
