use crate::types::AnyResult;

use super::subtitle::Subtitle;

/// Trait for streaming subtitle parsers.
///
/// Provides a unified interface for incremental parsing of subtitle files,
/// useful for large files or memory-constrained environments.
///
/// # Example
///
/// ```no_run
/// use subtitler::model::StreamingParser;
///
/// # fn main() -> anyhow::Result<()> {
/// let content = "1\n00:00:01,000 --> 00:00:03,500\nHello\n\n";
/// let mut parser = subtitler::srt::parse_stream(content);
///
/// while let Some(result) = parser.next() {
///   let subtitle = result?;
///   println!("{:?}", subtitle);
/// }
/// # Ok(())
/// # }
/// ```
pub trait StreamingParser: Iterator<Item = AnyResult<Subtitle>> {
  /// Parse all remaining subtitles and return as a vector.
  ///
  /// Returns an error if any subtitle fails to parse.
  fn collect_all(&mut self) -> AnyResult<Vec<Subtitle>> {
    let mut subtitles = Vec::new();
    for result in self {
      subtitles.push(result?);
    }
    Ok(subtitles)
  }

  /// Count remaining subtitles without collecting them.
  ///
  /// This consumes the iterator.
  fn count_remaining(&mut self) -> usize {
    self.filter(|r| r.is_ok()).count()
  }
}
