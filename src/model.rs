use serde::{Deserialize, Serialize};
#[derive(Debug, Deserialize, Serialize)]
pub struct Subtitle {
  pub index: usize,
  pub start: u64,
  pub end: u64,
  pub text: String,
  pub settings: Option<String>,
}

impl Subtitle {
  pub fn new(index: usize, start: u64, end: u64, text: &str) -> Self {
    Subtitle {
      index,
      start,
      end,
      settings: None,
      text: text.to_string(),
    }
  }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Timestamp {
  pub start: u64,
  pub end: u64,
  pub settings: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Cue {
  pub start: u64,
  pub end: u64,
  pub settings: Option<String>,
  pub text: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Format {
  SRT,
  WebVTT,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FormatOptions {
  pub format: Format,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NodeHeader {
  pub node_type: String, // 使用 String 以便表示类型
  pub data: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NodeCue {
  pub node_type: String, // 使用 String 以便表示类型
  pub data: Cue,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Node {
  pub header: NodeHeader,
  pub cue: NodeCue,
}
