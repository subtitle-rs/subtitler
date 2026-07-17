use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, ValueEnum)]
pub enum Format {
  #[cfg(feature = "srt")]
  Srt,
  #[cfg(feature = "vtt")]
  Vtt,
  #[cfg(feature = "ass")]
  Ass,
  #[cfg(feature = "ssa")]
  Ssa,
  #[cfg(feature = "microdvd")]
  #[value(name = "microdvd")]
  MicroDvd,
  #[cfg(feature = "subviewer")]
  #[value(name = "subviewer")]
  SubViewer,
  #[cfg(feature = "ttml")]
  Ttml,
  #[cfg(feature = "sbv")]
  Sbv,
  #[cfg(feature = "lrc")]
  Lrc,
  #[cfg(feature = "sami")]
  Sami,
  #[cfg(feature = "mpl2")]
  #[value(name = "mpl2")]
  Mpl2,
  #[cfg(feature = "scc")]
  #[value(name = "scc")]
  Scc,
  #[cfg(feature = "ebu_stl")]
  #[value(name = "stl")]
  EbuStl,
}

impl Format {
  pub fn from_ext(path: &str) -> Option<Self> {
    let lower = path.to_lowercase();
    #[cfg(feature = "srt")]
    if lower.ends_with(".srt") {
      return Some(Format::Srt);
    }
    #[cfg(feature = "vtt")]
    if lower.ends_with(".vtt") {
      return Some(Format::Vtt);
    }
    #[cfg(feature = "ass")]
    if lower.ends_with(".ass") {
      return Some(Format::Ass);
    }
    #[cfg(feature = "ssa")]
    if lower.ends_with(".ssa") {
      return Some(Format::Ssa);
    }
    #[cfg(feature = "microdvd")]
    if lower.ends_with(".sub") {
      return Some(Format::MicroDvd);
    }
    #[cfg(feature = "ttml")]
    if lower.ends_with(".ttml") || lower.ends_with(".xml") {
      return Some(Format::Ttml);
    }
    #[cfg(feature = "sbv")]
    if lower.ends_with(".sbv") {
      return Some(Format::Sbv);
    }
    #[cfg(feature = "lrc")]
    if lower.ends_with(".lrc") {
      return Some(Format::Lrc);
    }
    #[cfg(feature = "sami")]
    if lower.ends_with(".smi") || lower.ends_with(".sami") {
      return Some(Format::Sami);
    }
    #[cfg(feature = "mpl2")]
    if lower.ends_with(".mpl") || lower.ends_with(".txt") {
      return Some(Format::Mpl2);
    }
    #[cfg(feature = "scc")]
    if lower.ends_with(".scc") {
      return Some(Format::Scc);
    }
    #[cfg(feature = "ebu_stl")]
    if lower.ends_with(".stl") {
      return Some(Format::EbuStl);
    }
    None
  }
}

/// 集中维护的 CLI ↔ 库格式枚举转换，避免 main.rs 出现两份冗余的 match。
impl From<&subtitler::model::Format> for Format {
  fn from(f: &subtitler::model::Format) -> Self {
    use subtitler::model::Format as M;
    match f {
      #[cfg(feature = "srt")]
      M::Srt => Format::Srt,
      #[cfg(feature = "vtt")]
      M::Vtt => Format::Vtt,
      #[cfg(feature = "ass")]
      M::Ass => Format::Ass,
      #[cfg(feature = "ssa")]
      M::Ssa => Format::Ssa,
      #[cfg(feature = "microdvd")]
      M::MicroDvd => Format::MicroDvd,
      #[cfg(feature = "subviewer")]
      M::SubViewer => Format::SubViewer,
      #[cfg(feature = "ttml")]
      M::Ttml => Format::Ttml,
      #[cfg(feature = "sbv")]
      M::Sbv => Format::Sbv,
      #[cfg(feature = "lrc")]
      M::Lrc => Format::Lrc,
      #[cfg(feature = "sami")]
      M::Sami => Format::Sami,
      #[cfg(feature = "mpl2")]
      M::Mpl2 => Format::Mpl2,
      #[cfg(feature = "scc")]
      M::Scc => Format::Scc,
      #[cfg(feature = "ebu_stl")]
      M::EbuStl => Format::EbuStl,
    }
  }
}

impl From<&Format> for subtitler::model::Format {
  fn from(f: &Format) -> Self {
    use subtitler::model::Format as M;
    match f {
      #[cfg(feature = "srt")]
      Format::Srt => M::Srt,
      #[cfg(feature = "vtt")]
      Format::Vtt => M::Vtt,
      #[cfg(feature = "ass")]
      Format::Ass => M::Ass,
      #[cfg(feature = "ssa")]
      Format::Ssa => M::Ssa,
      #[cfg(feature = "microdvd")]
      Format::MicroDvd => M::MicroDvd,
      #[cfg(feature = "subviewer")]
      Format::SubViewer => M::SubViewer,
      #[cfg(feature = "ttml")]
      Format::Ttml => M::Ttml,
      #[cfg(feature = "sbv")]
      Format::Sbv => M::Sbv,
      #[cfg(feature = "lrc")]
      Format::Lrc => M::Lrc,
      #[cfg(feature = "sami")]
      Format::Sami => M::Sami,
      #[cfg(feature = "mpl2")]
      Format::Mpl2 => M::Mpl2,
      #[cfg(feature = "scc")]
      Format::Scc => M::Scc,
      #[cfg(feature = "ebu_stl")]
      Format::EbuStl => M::EbuStl,
    }
  }
}

impl std::fmt::Display for Format {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      #[cfg(feature = "srt")]
      Format::Srt => write!(f, "srt"),
      #[cfg(feature = "vtt")]
      Format::Vtt => write!(f, "vtt"),
      #[cfg(feature = "ass")]
      Format::Ass => write!(f, "ass"),
      #[cfg(feature = "ssa")]
      Format::Ssa => write!(f, "ssa"),
      #[cfg(feature = "microdvd")]
      Format::MicroDvd => write!(f, "microdvd"),
      #[cfg(feature = "subviewer")]
      Format::SubViewer => write!(f, "subviewer"),
      #[cfg(feature = "ttml")]
      Format::Ttml => write!(f, "ttml"),
      #[cfg(feature = "sbv")]
      Format::Sbv => write!(f, "sbv"),
      #[cfg(feature = "lrc")]
      Format::Lrc => write!(f, "lrc"),
      #[cfg(feature = "sami")]
      Format::Sami => write!(f, "sami"),
      #[cfg(feature = "mpl2")]
      Format::Mpl2 => write!(f, "MPL2"),
      #[cfg(feature = "scc")]
      Format::Scc => write!(f, "SCC"),
      #[cfg(feature = "ebu_stl")]
      Format::EbuStl => write!(f, "EBU STL"),
    }
  }
}

/// A CLI tool for parsing, converting, validating, and editing subtitles.
#[derive(Parser)]
#[command(name = "subtitler")]
#[command(
  about = "Subtitle toolkit: parse, convert, validate, edit, and analyze subtitles across 13 formats."
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

  /// Process subtitles through a pipeline of operations (JSON config)
  Pipeline(PipelineArgs),

  /// Show subtitle file statistics
  Info(InfoArgs),

  /// Detect subtitle format
  Detect(DetectArgs),

  /// Generate a quality report (JSON or human-readable)
  Quality(QualityArgs),

  /// Normalize subtitle text (OCR fix, hearing-impaired removal, whitespace)
  Normalize(NormalizeArgs),

  /// Shift all timestamps by a fixed offset
  Shift(ShiftArgs),
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

/// Generate a quality report.
#[derive(clap::Args)]
pub struct QualityArgs {
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

  /// Output as JSON
  #[arg(short, long)]
  pub json: bool,
}

/// Normalize subtitle text.
#[derive(clap::Args)]
pub struct NormalizeArgs {
  /// Input file path or URL
  pub input: String,

  /// Output file path (use "-" for stdout)
  #[arg(short, long)]
  pub output: String,

  /// Remove hearing-impaired tags ([LAUGHS], (APPLAUSE), ♪, etc.)
  #[arg(long)]
  pub strip_hi: bool,

  /// Fix common OCR errors (0→o, l→1, O→0)
  #[arg(long)]
  pub fix_ocr: bool,

  /// Normalize quotes (smart quotes → ASCII)
  #[arg(long)]
  pub quotes: bool,

  /// Normalize whitespace (collapse multiple spaces, trim)
  #[arg(long)]
  pub whitespace: bool,

  /// Apply all normalizations (equivalent to --strip-hi --fix-ocr --quotes --whitespace)
  #[arg(long)]
  pub all: bool,

  /// Force input format (auto-detected by default)
  #[arg(short, long)]
  pub format: Option<Format>,
}

/// Shift all timestamps by a fixed offset.
#[derive(clap::Args)]
pub struct ShiftArgs {
  /// Input file path or URL
  pub input: String,

  /// Output file path (use "-" for stdout)
  #[arg(short, long)]
  pub output: String,

  /// Offset in milliseconds (positive = delay, negative = advance)
  #[arg(allow_hyphen_values = true)]
  pub offset: i64,

  /// Force input format (auto-detected by default)
  #[arg(short, long)]
  pub format: Option<Format>,
}

/// Process subtitles through a pipeline of operations defined in a JSON config file.
///
/// Example pipeline.json:
/// ```json
/// {
///   "operations": [
///     {"op": "Sort"},
///     {"op": "Shift", "offset_ms": 500},
///     {"op": "SplitLong", "max_chars": 42},
///     {"op": "FilterEmpty"}
///   ]
/// }
/// ```
///
/// Usage:
///   subtitler pipeline input.srt output.vtt --config pipeline.json
///   subtitler pipeline input.srt output.srt --config pipeline.json --to srt
#[derive(clap::Args)]
pub struct PipelineArgs {
  /// Input file path or URL
  pub input: String,

  /// Output file path (use "-" for stdout)
  pub output: String,

  /// JSON config file defining pipeline operations
  #[arg(short, long)]
  pub config: String,

  /// Target format (inferred from output extension if not specified)
  #[arg(short, long)]
  pub to: Option<Format>,

  /// Force input format (auto-detect if not specified)
  #[arg(short, long)]
  pub from: Option<Format>,
}
