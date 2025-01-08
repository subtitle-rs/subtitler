use serde::{Deserialize, Serialize};
#[derive(Debug, Deserialize, Serialize)]
pub struct Subtitle {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub index: Option<usize>,
  pub start: u64,
  pub end: u64,
  pub text: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub settings: Option<String>,
}

impl Subtitle {
  pub fn new(start: u64, end: u64, text: &str) -> Self {
    Subtitle {
      index: None,
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
  #[serde(skip_serializing_if = "Option::is_none")]
  pub settings: Option<String>,
}