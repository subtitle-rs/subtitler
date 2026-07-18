/// Milliseconds per hour (3,600,000).
pub const MS_PER_HOUR: u64 = 3_600_000;
/// Milliseconds per minute (60,000).
pub const MS_PER_MINUTE: u64 = 60_000;
/// Milliseconds per second (1,000).
pub const MS_PER_SECOND: u64 = 1_000;
/// Default tail duration for formats without explicit end times (3 seconds).
pub const DEFAULT_TAIL_MS: u64 = 3_000;

pub fn parse_ass_color(color: &str) -> (u8, u8, u8, u8) {
  let hex = color.trim_start_matches("&H").trim_start_matches("&h");
  let parsed = u32::from_str_radix(hex, 16).unwrap_or(0x00FFFFFF);
  let b = (parsed >> 16 & 0xFF) as u8;
  let g = (parsed >> 8 & 0xFF) as u8;
  let r = (parsed & 0xFF) as u8;
  let a = (parsed >> 24 & 0xFF) as u8;
  (r, g, b, a)
}

pub fn format_ass_color(r: u8, g: u8, b: u8, a: u8) -> String {
  let value = ((a as u32) << 24) | ((b as u32) << 16) | ((g as u32) << 8) | (r as u32);
  format!("&H{:08X}", value)
}

pub fn ms_to_frames(ms: u64, fps: f64) -> u64 {
  ((ms as f64) * fps / 1000.0).round() as u64
}

pub fn frames_to_ms(frames: u64, fps: f64) -> u64 {
  ((frames as f64) * 1000.0 / fps).round() as u64
}

pub fn split_text_chunks(text: &str, max_chars: usize) -> Vec<String> {
  let mut chunks = Vec::new();
  let words: Vec<&str> = text.split_whitespace().collect();
  let mut current = String::with_capacity(max_chars);

  for word in words {
    let needed = if current.is_empty() {
      word.len()
    } else {
      current.len() + 1 + word.len()
    };

    if needed > max_chars && !current.is_empty() {
      chunks.push(std::mem::take(&mut current));
      current.push_str(word);
    } else {
      if !current.is_empty() {
        current.push(' ');
      }
      current.push_str(word);
    }
  }

  if !current.is_empty() {
    chunks.push(current);
  }

  chunks
}
