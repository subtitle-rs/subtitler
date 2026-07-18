//! Shared I/O helpers for subtitle file writing.
//!
//! The `open_with_policy` helper centralizes the `WritePolicy` →
//! `tokio::fs::OpenOptions` mapping that every format's `generate()`
//! function needs. Format-specific quirks (e.g. SRT's append-mode
//! blank-line separator) are left to the caller.

use crate::error::SubtitleError;
use crate::model::WritePolicy;
use crate::types::AnyResult;
use std::path::Path;

/// Open a file for writing according to the given [`WritePolicy`].
///
/// - `Overwrite` (default): create or truncate.
/// - `RefuseIfExists`: fail with [`SubtitleError::FileExists`] if the
///   path already exists; otherwise create.
/// - `Append`: open for append (create if missing).
///
/// Returns the opened file. The caller is responsible for writing
/// content, flushing, and any format-specific separators.
#[cfg(not(target_arch = "wasm32"))]
pub async fn open_with_policy(
  path: impl AsRef<Path>,
  policy: Option<WritePolicy>,
) -> AnyResult<tokio::fs::File> {
  let path = path.as_ref();
  let policy = policy.unwrap_or_default();

  if matches!(policy, WritePolicy::RefuseIfExists) && path.exists() {
    return Err(
      SubtitleError::FileExists {
        path: path.to_path_buf(),
      }
      .into(),
    );
  }

  let mut open_opts = tokio::fs::OpenOptions::new();
  let file = match policy {
    WritePolicy::Append => open_opts.create(true).append(true).open(path).await,
    _ => open_opts
      .create(true)
      .write(true)
      .truncate(true)
      .open(path)
      .await,
  }?;
  Ok(file)
}

/// Convenience: open, write all bytes, flush. For formats with no
/// append-mode separator quirks.
///
/// SRT's `generate` does NOT use this (it needs a blank-line separator
/// in append mode); other formats can.
#[cfg(not(target_arch = "wasm32"))]
pub async fn write_with_policy(
  path: impl AsRef<Path>,
  bytes: &[u8],
  policy: Option<WritePolicy>,
) -> AnyResult<()> {
  use tokio::io::AsyncWriteExt;
  let mut dest = open_with_policy(path, policy).await?;
  dest.write_all(bytes).await?;
  dest.flush().await?;
  Ok(())
}
