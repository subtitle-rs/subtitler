use crate::model::{Format, SubtitleFormat};
use crate::{detect_format, parse_bytes};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct SubtitlerResult {
  #[wasm_bindgen(getter_with_clone)]
  pub subtitle_count: u32,
  #[wasm_bindgen(getter_with_clone)]
  pub format: String,
  #[wasm_bindgen(getter_with_clone)]
  pub output: String,
  error: Option<String>,
}

#[wasm_bindgen]
impl SubtitlerResult {
  #[wasm_bindgen(getter)]
  pub fn error(&self) -> Option<String> {
    self.error.clone()
  }

  #[wasm_bindgen(getter)]
  pub fn is_ok(&self) -> bool {
    self.error.is_none()
  }
}

#[wasm_bindgen]
pub fn parse_subtitles(content: &str) -> SubtitlerResult {
  let fmt = match detect_format(content.as_bytes()) {
    Some(f) => f,
    None => {
      return SubtitlerResult {
        subtitle_count: 0,
        format: String::new(),
        output: String::new(),
        error: Some("Could not detect subtitle format".to_string()),
      };
    }
  };

  match parse_bytes(content.as_bytes()) {
    Ok(file) => {
      let count = file.subtitles().len() as u32;
      let out = file.to_string();
      SubtitlerResult {
        subtitle_count: count,
        format: format!("{:?}", fmt),
        output: out,
        error: None,
      }
    }
    Err(e) => SubtitlerResult {
      subtitle_count: 0,
      format: format!("{:?}", fmt),
      output: String::new(),
      error: Some(e.to_string()),
    },
  }
}

#[wasm_bindgen]
pub fn convert_format(content: &str, target_format: &str) -> SubtitlerResult {
  let target = match target_format.to_lowercase().as_str() {
    #[cfg(feature = "srt")]
    "srt" => Format::Srt,
    #[cfg(feature = "vtt")]
    "vtt" => Format::Vtt,
    #[cfg(feature = "ass")]
    "ass" => Format::Ass,
    #[cfg(feature = "ssa")]
    "ssa" => Format::Ssa,
    #[cfg(feature = "microdvd")]
    "microdvd" => Format::MicroDvd,
    #[cfg(feature = "subviewer")]
    "subviewer" => Format::SubViewer,
    #[cfg(feature = "ttml")]
    "ttml" => Format::Ttml,
    #[cfg(feature = "sbv")]
    "sbv" => Format::Sbv,
    #[cfg(feature = "lrc")]
    "lrc" => Format::Lrc,
    #[cfg(feature = "sami")]
    "sami" => Format::Sami,
    #[cfg(feature = "mpl2")]
    "mpl2" => Format::Mpl2,
    #[cfg(feature = "scc")]
    "scc" => Format::Scc,
    #[cfg(feature = "ebu_stl")]
    "ebu_stl" => Format::EbuStl,
    _ => {
      return SubtitlerResult {
        subtitle_count: 0,
        format: String::new(),
        output: String::new(),
        error: Some(format!("Unsupported target format: {}", target_format)),
      };
    }
  };

  match parse_bytes(content.as_bytes()) {
    Ok(file) => {
      let count = file.subtitles().len() as u32;
      let out = file.to_string_with_format(&target);
      SubtitlerResult {
        subtitle_count: count,
        format: format!("{:?}", target),
        output: out,
        error: None,
      }
    }
    Err(e) => SubtitlerResult {
      subtitle_count: 0,
      format: format!("{:?}", target),
      output: String::new(),
      error: Some(e.to_string()),
    },
  }
}

#[wasm_bindgen]
pub fn validate_subtitles(content: &str) -> JsValue {
  let fmt = match detect_format(content.as_bytes()) {
    Some(f) => f,
    None => {
      let result = serde_json::json!({
        "error": "Could not detect subtitle format"
      });
      return serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL);
    }
  };

  match parse_bytes(content.as_bytes()) {
    Ok(file) => {
      let issues = file.validate();
      let result = serde_json::json!({
        "format": format!("{:?}", fmt),
        "subtitle_count": file.subtitles().len(),
        "issues": issues.iter().map(|i| i.to_string()).collect::<Vec<_>>(),
        "issue_count": issues.len(),
      });
      serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
    }
    Err(e) => {
      let result = serde_json::json!({
        "error": e.to_string()
      });
      serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
    }
  }
}

#[wasm_bindgen]
pub fn detect(content: &str) -> String {
  match detect_format(content.as_bytes()) {
    Some(f) => format!("{:?}", f),
    None => "unknown".to_string(),
  }
}

#[wasm_bindgen]
pub fn get_info(content: &str) -> JsValue {
  let fmt = match detect_format(content.as_bytes()) {
    Some(f) => f,
    None => {
      let result = serde_json::json!({ "error": "unknown format" });
      return serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL);
    }
  };

  match parse_bytes(content.as_bytes()) {
    Ok(file) => {
      let subs = file.subtitles();
      let total_duration = if subs.is_empty() {
        0
      } else {
        subs.last().unwrap().end - subs.first().unwrap().start
      };
      let result = serde_json::json!({
        "format": format!("{:?}", fmt),
        "subtitle_count": subs.len(),
        "total_duration_ms": total_duration,
        "first_timestamp": subs.first().map(|s| s.start).unwrap_or(0),
        "last_timestamp": subs.last().map(|s| s.end).unwrap_or(0),
      });
      serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
    }
    Err(e) => {
      let result = serde_json::json!({ "error": e.to_string() });
      serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
    }
  }
}

#[wasm_bindgen]
pub fn normalize_text(content: &str) -> String {
  let _fmt = match detect_format(content.as_bytes()) {
    Some(f) => f,
    None => return content.to_string(),
  };

  match parse_bytes(content.as_bytes()) {
    Ok(mut file) => {
      for sub in file.subtitles_mut() {
        sub.strip_tags();
      }
      file.to_string()
    }
    Err(_) => content.to_string(),
  }
}

#[cfg(test)]
#[wasm_bindgen_test::wasm_bindgen_test]
fn test_parse_subtitles_srt() {
  let srt = "1\n00:00:01,000 --> 00:00:03,500\nHello\n\n";
  let result = parse_subtitles(srt);
  assert!(result.is_ok());
  assert_eq!(result.subtitle_count, 1);
  assert!(result.output.contains("Hello"));
}

#[cfg(test)]
#[wasm_bindgen_test::wasm_bindgen_test]
fn test_parse_subtitles_unknown_format() {
  let result = parse_subtitles("garbage");
  assert!(!result.is_ok());
  assert_eq!(result.subtitle_count, 0);
}

#[cfg(test)]
#[wasm_bindgen_test::wasm_bindgen_test]
fn test_detect_srt() {
  let srt = "1\n00:00:01,000 --> 00:00:03,500\nHello\n\n";
  assert_eq!(detect(srt), "Srt");
}

#[cfg(test)]
#[wasm_bindgen_test::wasm_bindgen_test]
fn test_detect_unknown() {
  assert_eq!(detect("garbage"), "unknown");
}

#[cfg(test)]
#[wasm_bindgen_test::wasm_bindgen_test]
fn test_convert_format_srt_to_vtt() {
  let srt = "1\n00:00:01,000 --> 00:00:03,500\nHello\n\n";
  let result = convert_format(srt, "vtt");
  assert!(result.is_ok());
  assert!(result.output.contains("WEBVTT"));
}

#[cfg(test)]
#[wasm_bindgen_test::wasm_bindgen_test]
fn test_validate_and_info() {
  let srt = "1\n00:00:01,000 --> 00:00:03,500\nHello\n\n";
  let v = validate_subtitles(srt);
  assert!(!v.is_null());
  let info = get_info(srt);
  assert!(!info.is_null());
}
