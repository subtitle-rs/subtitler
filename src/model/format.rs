use serde::{Deserialize, Serialize};

use super::subtitle::Subtitle;
use super::r#trait::SubtitleFormat;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
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
  MicroDvd,
  #[cfg(feature = "subviewer")]
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
  Mpl2,
  #[cfg(feature = "scc")]
  Scc,
  #[cfg(feature = "ebu_stl")]
  EbuStl,
  #[cfg(feature = "dfxp")]
  Dfxp,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum SubtitleFile {
  #[cfg(feature = "srt")]
  Srt(Vec<Subtitle>),

  #[cfg(feature = "vtt")]
  Vtt {
    #[serde(skip_serializing_if = "Option::is_none")]
    header: Option<String>,
    subtitles: Vec<Subtitle>,
  },

  #[cfg(feature = "ass")]
  Ass(super::types::AssData),

  #[cfg(feature = "ssa")]
  Ssa(super::types::AssData),

  #[cfg(feature = "microdvd")]
  MicroDvd { fps: f64, subtitles: Vec<Subtitle> },

  #[cfg(feature = "subviewer")]
  SubViewer {
    #[serde(skip_serializing_if = "Option::is_none")]
    header: Option<String>,
    subtitles: Vec<Subtitle>,
  },

  #[cfg(feature = "ttml")]
  Ttml {
    #[serde(skip_serializing_if = "Option::is_none")]
    header: Option<String>,
    subtitles: Vec<Subtitle>,
  },

  #[cfg(feature = "sbv")]
  Sbv(Vec<Subtitle>),

  #[cfg(feature = "lrc")]
  Lrc {
    data: crate::lrc::LrcData,
    subtitles: Vec<Subtitle>,
  },

  #[cfg(feature = "sami")]
  Sami(crate::sami::SamiData),

  #[cfg(feature = "mpl2")]
  Mpl2(Vec<Subtitle>),

  #[cfg(feature = "scc")]
  Scc(crate::scc::SccData),

  #[cfg(feature = "ebu_stl")]
  EbuStl(Box<crate::ebu_stl::EbuStlData>),

  #[cfg(feature = "dfxp")]
  Dfxp { header: Option<String>, subtitles: Vec<Subtitle> },
}

impl SubtitleFormat for SubtitleFile {
  fn subtitles(&self) -> &[Subtitle] {
    match self {
      #[cfg(feature = "srt")]
      SubtitleFile::Srt(subs) => subs,
      #[cfg(feature = "vtt")]
      SubtitleFile::Vtt { subtitles, .. } => subtitles,
      #[cfg(feature = "ass")]
      SubtitleFile::Ass(data) => &data.subtitles,
      #[cfg(feature = "ssa")]
      SubtitleFile::Ssa(data) => &data.subtitles,
      #[cfg(feature = "microdvd")]
      SubtitleFile::MicroDvd { subtitles, .. } => subtitles,
      #[cfg(feature = "subviewer")]
      SubtitleFile::SubViewer { subtitles, .. } => subtitles,
      #[cfg(feature = "ttml")]
      SubtitleFile::Ttml { subtitles, .. } => subtitles,
      #[cfg(feature = "sbv")]
      SubtitleFile::Sbv(subs) => subs,
      #[cfg(feature = "lrc")]
      SubtitleFile::Lrc { subtitles, .. } => subtitles,
      #[cfg(feature = "sami")]
      SubtitleFile::Sami(data) => &data.subtitles,
      #[cfg(feature = "mpl2")]
      SubtitleFile::Mpl2(subs) => subs,
      #[cfg(feature = "scc")]
      SubtitleFile::Scc(data) => &data.subtitles,
      #[cfg(feature = "ebu_stl")]
      SubtitleFile::EbuStl(data) => &data.subtitles,
      #[cfg(feature = "dfxp")]
      SubtitleFile::Dfxp { subtitles, .. } => subtitles,
    }
  }

  fn subtitles_mut(&mut self) -> &mut Vec<Subtitle> {
    match self {
      #[cfg(feature = "srt")]
      SubtitleFile::Srt(subs) => subs,
      #[cfg(feature = "vtt")]
      SubtitleFile::Vtt { subtitles, .. } => subtitles,
      #[cfg(feature = "ass")]
      SubtitleFile::Ass(data) => &mut data.subtitles,
      #[cfg(feature = "ssa")]
      SubtitleFile::Ssa(data) => &mut data.subtitles,
      #[cfg(feature = "microdvd")]
      SubtitleFile::MicroDvd { subtitles, .. } => subtitles,
      #[cfg(feature = "subviewer")]
      SubtitleFile::SubViewer { subtitles, .. } => subtitles,
      #[cfg(feature = "ttml")]
      SubtitleFile::Ttml { subtitles, .. } => subtitles,
      #[cfg(feature = "sbv")]
      SubtitleFile::Sbv(subs) => subs,
      #[cfg(feature = "lrc")]
      SubtitleFile::Lrc { subtitles, .. } => subtitles,
      #[cfg(feature = "sami")]
      SubtitleFile::Sami(data) => &mut data.subtitles,
      #[cfg(feature = "mpl2")]
      SubtitleFile::Mpl2(subs) => subs,
      #[cfg(feature = "scc")]
      SubtitleFile::Scc(data) => &mut data.subtitles,
      #[cfg(feature = "ebu_stl")]
      SubtitleFile::EbuStl(data) => &mut data.subtitles,
      #[cfg(feature = "dfxp")]
      SubtitleFile::Dfxp { subtitles, .. } => subtitles,
    }
  }

  fn format(&self) -> Format {
    match self {
      #[cfg(feature = "srt")]
      SubtitleFile::Srt(_) => Format::Srt,
      #[cfg(feature = "vtt")]
      SubtitleFile::Vtt { .. } => Format::Vtt,
      #[cfg(feature = "ass")]
      SubtitleFile::Ass(_) => Format::Ass,
      #[cfg(feature = "ssa")]
      SubtitleFile::Ssa(_) => Format::Ssa,
      #[cfg(feature = "microdvd")]
      SubtitleFile::MicroDvd { .. } => Format::MicroDvd,
      #[cfg(feature = "subviewer")]
      SubtitleFile::SubViewer { .. } => Format::SubViewer,
      #[cfg(feature = "ttml")]
      SubtitleFile::Ttml { .. } => Format::Ttml,
      #[cfg(feature = "sbv")]
      SubtitleFile::Sbv(_) => Format::Sbv,
      #[cfg(feature = "lrc")]
      SubtitleFile::Lrc { .. } => Format::Lrc,
      #[cfg(feature = "sami")]
      SubtitleFile::Sami(_) => Format::Sami,
      #[cfg(feature = "mpl2")]
      SubtitleFile::Mpl2(_) => Format::Mpl2,
      #[cfg(feature = "scc")]
      SubtitleFile::Scc(_) => Format::Scc,
      #[cfg(feature = "ebu_stl")]
      SubtitleFile::EbuStl(_) => Format::EbuStl,
      #[cfg(feature = "dfxp")]
      SubtitleFile::Dfxp { .. } => Format::Dfxp,
    }
  }

  fn to_string_with_format(&self, format: &Format) -> String {
    let subs = SubtitleFormat::subtitles(self);
    match format {
      #[cfg(feature = "srt")]
      Format::Srt => crate::srt::to_string(subs),
      #[cfg(feature = "vtt")]
      Format::Vtt => crate::vtt::to_string(subs, None),
      #[cfg(feature = "ass")]
      Format::Ass => ass_to_string_impl(self, subs),
      #[cfg(feature = "ssa")]
      Format::Ssa => ass_to_string_impl(self, subs),
      #[cfg(feature = "microdvd")]
      Format::MicroDvd => {
        let fps = match self {
          SubtitleFile::MicroDvd { fps, .. } => Some(*fps),
          _ => None,
        };
        match fps {
          Some(f) if (f - crate::microdvd::DEFAULT_FPS).abs() > f64::EPSILON => {
            crate::microdvd::to_string_with_fps_header(subs, f)
          }
          _ => crate::microdvd::to_string(subs, fps),
        }
      }
      #[cfg(feature = "subviewer")]
      Format::SubViewer => {
        let header = match self {
          SubtitleFile::SubViewer { header, .. } => header.as_deref(),
          _ => None,
        };
        crate::subviewer::to_string(subs, header)
      }
      #[cfg(feature = "ttml")]
      Format::Ttml => {
        let header = match self {
          SubtitleFile::Ttml { header, .. } => header.as_deref(),
          _ => None,
        };
        crate::ttml::to_string(subs, header)
      }
      #[cfg(feature = "sbv")]
      Format::Sbv => crate::sbv::to_string(subs),
      #[cfg(feature = "lrc")]
      Format::Lrc => match self {
        SubtitleFile::Lrc { data, .. } => data.render(),
        _ => crate::lrc::to_string(subs),
      },
      #[cfg(feature = "sami")]
      Format::Sami => {
        let header = match self {
          SubtitleFile::Sami(data) => data.header.as_deref(),
          _ => None,
        };
        crate::sami::to_string(subs, header)
      }
      #[cfg(feature = "mpl2")]
      Format::Mpl2 => crate::mpl2::to_string(subs, None),
      #[cfg(feature = "scc")]
      Format::Scc => {
        let drop_frame = match self {
          SubtitleFile::Scc(data) => data.drop_frame,
          _ => true,
        };
        crate::scc::to_string(subs, drop_frame)
      }
      #[cfg(feature = "ebu_stl")]
      Format::EbuStl => String::from_utf8_lossy(&crate::ebu_stl::to_string(subs)).to_string(),
      #[cfg(feature = "dfxp")]
      Format::Dfxp => {
        let header = match self {
          SubtitleFile::Dfxp { header, .. } => header.as_deref(),
          _ => None,
        };
        crate::dfxp::to_string(subs, header)
      }
    }
  }
}

#[cfg(any(feature = "ass", feature = "ssa"))]
fn ass_to_string_impl(file: &SubtitleFile, subs: &[Subtitle]) -> String {
  let (info, styles) = match file {
    #[cfg(feature = "ass")]
    SubtitleFile::Ass(data) => (data.info.clone(), data.styles.clone()),
    #[cfg(feature = "ssa")]
    SubtitleFile::Ssa(data) => (data.info.clone(), data.styles.clone()),
    #[allow(unreachable_patterns)]
    _ => (
      std::collections::HashMap::new(),
      vec![super::types::AssStyle::default_style()],
    ),
  };
  crate::ass::to_string(&info, &styles, subs)
}

impl SubtitleFile {
  pub fn concatenate(&mut self, other: &SubtitleFile, gap_ms: u64) {
    let own_end = self.subtitles().iter().map(|s| s.end).max().unwrap_or(0);
    let offset = own_end + gap_ms;
    let subs = self.subtitles_mut();
    for sub in other.subtitles() {
      let mut clone = sub.clone();
      clone.shift(offset as i64);
      subs.push(clone);
    }
    self.sort();
  }
}
