use super::format::{Format, SubtitleFile};
use super::subtitle::Subtitle;
#[cfg(any(feature = "ass", feature = "ssa"))]
use super::types::{AssData, AssStyle};

/// Builder for constructing `SubtitleFile` with a fluent API.
///
/// # Example
///
/// ```no_run
/// use subtitler::model::{SubtitleFileBuilder, Subtitle, Format};
///
/// let file = SubtitleFileBuilder::new(Format::Srt)
///   .add_subtitle(Subtitle::new(0, 5000, "Hello"))
///   .add_subtitle(Subtitle::new(6000, 10000, "World"))
///   .build()
///   .unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct SubtitleFileBuilder {
  format: Format,
  subtitles: Vec<Subtitle>,
  fps: Option<f64>,
  header: Option<String>,
  #[cfg(any(feature = "ass", feature = "ssa"))]
  styles: Vec<AssStyle>,
}

impl SubtitleFileBuilder {
  /// Create a new builder for the specified format.
  pub fn new(format: Format) -> Self {
    Self {
      format,
      subtitles: Vec::new(),
      fps: None,
      header: None,
      #[cfg(any(feature = "ass", feature = "ssa"))]
      styles: Vec::new(),
    }
  }

  /// Add a subtitle to the file.
  pub fn add_subtitle(mut self, subtitle: Subtitle) -> Self {
    self.subtitles.push(subtitle);
    self
  }

  /// Add multiple subtitles to the file.
  pub fn add_subtitles(mut self, subtitles: impl IntoIterator<Item = Subtitle>) -> Self {
    self.subtitles.extend(subtitles);
    self
  }

  /// Set the frame rate (required for MicroDVD format).
  pub fn with_fps(mut self, fps: f64) -> Self {
    self.fps = Some(fps);
    self
  }

  /// Set the header (optional for VTT, SubViewer, TTML formats).
  pub fn with_header(mut self, header: impl Into<String>) -> Self {
    self.header = Some(header.into());
    self
  }

  /// Add an ASS style (for ASS/SSA formats).
  #[cfg(any(feature = "ass", feature = "ssa"))]
  pub fn add_style(mut self, style: AssStyle) -> Self {
    self.styles.push(style);
    self
  }

  /// Add multiple ASS styles.
  #[cfg(any(feature = "ass", feature = "ssa"))]
  pub fn add_styles(mut self, styles: impl IntoIterator<Item = AssStyle>) -> Self {
    self.styles.extend(styles);
    self
  }

  /// Build the `SubtitleFile`.
  ///
  /// Returns `None` if required fields are missing:
  /// - MicroDVD requires `fps`
  pub fn build(self) -> Option<SubtitleFile> {
    match self.format {
      #[cfg(feature = "srt")]
      Format::Srt => Some(SubtitleFile::Srt(self.subtitles)),

      #[cfg(feature = "vtt")]
      Format::Vtt => Some(SubtitleFile::Vtt {
        header: self.header,
        subtitles: self.subtitles,
      }),

      #[cfg(feature = "ass")]
      Format::Ass => Some(SubtitleFile::Ass(AssData {
        info: std::collections::HashMap::new(),
        styles: if self.styles.is_empty() {
          vec![AssStyle::default_style()]
        } else {
          self.styles
        },
        subtitles: self.subtitles,
      })),

      #[cfg(feature = "ssa")]
      Format::Ssa => Some(SubtitleFile::Ssa(AssData {
        info: std::collections::HashMap::new(),
        styles: if self.styles.is_empty() {
          vec![AssStyle::default_style()]
        } else {
          self.styles
        },
        subtitles: self.subtitles,
      })),

      #[cfg(feature = "microdvd")]
      Format::MicroDvd => {
        let fps = self.fps?;
        Some(SubtitleFile::MicroDvd {
          fps,
          subtitles: self.subtitles,
        })
      }

      #[cfg(feature = "subviewer")]
      Format::SubViewer => Some(SubtitleFile::SubViewer {
        header: self.header,
        subtitles: self.subtitles,
      }),

      #[cfg(feature = "ttml")]
      Format::Ttml => Some(SubtitleFile::Ttml {
        header: self.header,
        subtitles: self.subtitles,
      }),

      #[cfg(feature = "sbv")]
      Format::Sbv => Some(SubtitleFile::Sbv(self.subtitles)),

      #[cfg(feature = "lrc")]
      Format::Lrc => {
        let data = crate::lrc::LrcData::default();
        let flatten = data.to_subtitles();
        Some(SubtitleFile::Lrc {
          data,
          subtitles: if self.subtitles.is_empty() {
            flatten
          } else {
            self.subtitles
          },
        })
      }

      #[cfg(feature = "sami")]
      Format::Sami => Some(SubtitleFile::Sami(crate::sami::SamiData {
        header: self.header,
        styles: std::collections::HashMap::new(),
        subtitles: self.subtitles,
      })),

      #[cfg(feature = "mpl2")]
      Format::Mpl2 => Some(SubtitleFile::Mpl2(self.subtitles)),

      #[cfg(feature = "scc")]
      Format::Scc => Some(SubtitleFile::Scc(crate::scc::SccData {
        fps: crate::scc::DEFAULT_FPS,
        drop_frame: true,
        subtitles: self.subtitles,
      })),

      #[cfg(feature = "ebu_stl")]
      Format::EbuStl => {
        let tti_blocks: Vec<crate::ebu_stl::TtiBlock> = self
          .subtitles
          .iter()
          .enumerate()
          .map(|(i, sub)| crate::ebu_stl::TtiBlock {
            subtitle_group: 0,
            subtitle_number: (i + 1) as u16,
            extension_block: 0,
            cumulative_status: 0,
            timecode_start: (sub.start / 40) as u32,
            timecode_end: (sub.end / 40) as u32,
            vertical_position: 20,
            justification: 0,
            comment_flag: false,
            text: sub.text.clone(),
          })
          .collect();
        Some(SubtitleFile::EbuStl(Box::new(crate::ebu_stl::EbuStlData {
          gsi: crate::ebu_stl::GsiBlock::default(),
          subtitles: self.subtitles,
          tti_blocks,
        })))
      }
    }
  }
}

/// Configuration options for subtitle parsing behavior.
///
/// # Example
///
/// ```no_run
/// use subtitler::model::ParseConfig;
///
/// let config = ParseConfig::new()
///   .preserve_indices(true)       // Keep original indices
///   .lenient_mode(true)           // Tolerate format errors
///   .auto_detect_encoding(true);  // Auto-detect encoding
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ParseConfig {
  /// Preserve original subtitle indices (cue numbers).
  /// Default: false (re-index from 1)
  pub preserve_indices: bool,

  /// Lenient parsing mode: tolerate certain format errors.
  /// Default: false (strict parsing)
  pub lenient_mode: bool,

  /// Auto-detect text encoding (e.g., UTF-8, Latin-1).
  /// Default: true
  pub auto_detect_encoding: bool,

  /// Maximum allowed subtitle duration in ms (0 = no limit).
  /// Default: 0
  pub max_duration_ms: u64,

  /// Minimum allowed subtitle duration in ms.
  /// Default: 0
  pub min_duration_ms: u64,
}

impl Default for ParseConfig {
  fn default() -> Self {
    Self {
      preserve_indices: false,
      lenient_mode: false,
      auto_detect_encoding: true,
      max_duration_ms: 0,
      min_duration_ms: 0,
    }
  }
}

impl ParseConfig {
  /// Create a new ParseConfig with default values.
  pub fn new() -> Self {
    Self::default()
  }

  /// Preserve original subtitle indices (cue numbers).
  pub fn preserve_indices(mut self, preserve: bool) -> Self {
    self.preserve_indices = preserve;
    self
  }

  /// Enable lenient parsing mode (tolerate format errors).
  pub fn lenient_mode(mut self, lenient: bool) -> Self {
    self.lenient_mode = lenient;
    self
  }

  /// Auto-detect text encoding.
  pub fn auto_detect_encoding(mut self, detect: bool) -> Self {
    self.auto_detect_encoding = detect;
    self
  }

  /// Set maximum allowed subtitle duration (0 = no limit).
  pub fn max_duration_ms(mut self, ms: u64) -> Self {
    self.max_duration_ms = ms;
    self
  }

  /// Set minimum allowed subtitle duration.
  pub fn min_duration_ms(mut self, ms: u64) -> Self {
    self.min_duration_ms = ms;
    self
  }
}
