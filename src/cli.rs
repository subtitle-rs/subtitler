use clap::{Parser, Subcommand};

/// A simple CLI application example
#[derive(Parser)]
#[command(name = "subtitler cli")]
#[command(about = "A simple CLI application written in Rust.")]
pub struct CLI {
  /// The command to run
  #[command(subcommand)]
  pub command: Option<Commands>,
}

/// Available commands
#[derive(Subcommand)]
pub enum Commands {
  File {
    path: String,
  },
  Url {
    url: String,
  },
}
