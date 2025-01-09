<h1 align="center">Subtitler</h1>
<div align="center">
 <strong>
  subtitler is a library for parsing and generating subtitles
 </strong>
</div>

<br />

<div align="center">
  <!-- Crates version -->
  <a href="https://crates.io/crates/subtitler">
    <img src="https://img.shields.io/crates/v/subtitler.svg?style=flat-square"
    alt="Crates.io version" />
  </a>
  <!-- License -->
  <a href="https://crates.io/crates/subtitler">
    <img src="https://img.shields.io/crates/l/subtitler"
      alt="License" />
  </a>
  <!-- Downloads -->
  <a href="https://crates.io/crates/subtitler">
    <img src="https://img.shields.io/crates/d/subtitler.svg?style=flat-square"
      alt="Download" />
  </a>
  <!-- docs.rs docs -->
  <a href="https://docs.rs/subtitler">
    <img src="https://img.shields.io/badge/docs-latest-blue.svg?style=flat-square"
      alt="docs.rs docs" />
  </a>
  <!-- Ci -->
  <a href="https://github.com/rs-videos/subtitler/actions">
    <img src="https://github.com/subtitle-rs/subtitler/workflows/Rust/badge.svg"
      alt="github actions" />
  </a>
</div>

<div align="center">
  <h3>
    <a href="https://docs.rs/subtitler">
      API Docs
    </a>
  </h3>
</div>

## parse srt file

```rust
use thiserror::Error;
#[derive(Debug, Error)]
pub enum Error {
  #[error("json error {0:?}")]
  Json(#[from] serde_json::Error),
  #[error("IO error {0:?}")]
  IO(#[from] std::io::Error),
  #[error("any error {0:?}")]
  Any(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[macro_use]
extern crate tracing;

use std::env;
use std::path::PathBuf;
use subtitler::srt::parse_file;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
#[tokio::main]
async fn main() -> Result<()> {
  let subscriber = FmtSubscriber::builder()
    .with_max_level(Level::INFO)
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
  let mut file_path: PathBuf = env::current_dir().expect("Failed to get current_dir");
  file_path.push("examples/example.srt");

  let subtitle = parse_file(file_path.to_str().unwrap()).await?;
  info!("subtitle {:?}", subtitle);
  info!("subtitle json {}", serde_json::to_string(&subtitle)?);
  Ok(())
}
```
