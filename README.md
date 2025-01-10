# Subtitler

> subtitler is a library for parsing and generating subtitles

[![Crates.io](https://img.shields.io/crates/v/subtitler?style=flat-square)](https://crates.io/crates/subtitler)
[![Crates.io](https://img.shields.io/crates/d/subtitler?style=flat-square)](https://crates.io/crates/subtitler)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue?style=flat-square)](LICENSE-APACHE)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE-MIT)
[![Build Status](https://img.shields.io/github/actions/workflow/status/subtitle-rs/subtitler/rust.yml?branch=main&style=flat-square)](https://github.com/subtitle-rs/subtitler/actions/workflows/rust.yml?query=branch%3Amain)
[![Contributors](https://img.shields.io/github/contributors/subtitle-rs/subtitler?style=flat-square)](https://github.com/subtitle-rs/subtitler/graphs/contributors)

## Install

```sh
cargo install subtitler
```

## parse subtitle from url

```sh
subtitler url your_subtitle_url
```

## parse subtitle from file

```sh
subtitler file your_subtitle_file
```

## Examples

more [examples](https://github.com/subtitle-rs/subtitler/tree/main/examples)。

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
