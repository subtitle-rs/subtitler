use thiserror::Error;
#[derive(Debug, Error)]
pub enum Error {
  #[error("json error {0:?}")]
  Json(#[from] serde_json::Error),
  #[error("IO error {0:?}")]
  IO(#[from] std::io::Error),
  #[error("any error {0:?}")]
  Any(#[from] anyhow::Error),
  #[error("regex Error {0:?}")]
  RegexError(#[from] regex::Error),
  #[error("error msg {msg:?}")]
  Message { msg: String },
}

pub fn error_msg(msg: &str) -> Error {
  Error::Message {
    msg: msg.to_string(),
  }
}
