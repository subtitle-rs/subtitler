//! EBU-TT-D (Part D) read support: EBU-TT-D documents are valid TTML
//! with the `urn:ebu:tt` metadata namespace — they parse and convert
//! through the TTML path (read-only scope, roadmap spec §2.13 B3).
#![cfg(all(feature = "ttml", feature = "srt"))]

use subtitler::{SubtitleFormat, parse_bytes};

const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:ebu="urn:ebu:tt:metadata" xml:lang="de" ttp:timeBase="media">
  <head>
    <metadata>
      <ebu:documentMetadata ebu:documentIdentifier="DE-1234" ebu:originalFormat="EBU-TT-D-1"/>
    </metadata>
  </head>
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.000" region="r1">Erste Zeile der EBU-TT-D</p>
      <p begin="00:00:04.000" end="00:00:06.000" region="r1">Zweite Zeile</p>
    </div>
  </body>
</tt>"#;

#[test]
fn ebu_tt_d_parses_and_converts() {
  let file = parse_bytes(SAMPLE.as_bytes()).unwrap();
  assert_eq!(file.subtitles().len(), 2);
  assert_eq!(file.subtitles()[0].text, "Erste Zeile der EBU-TT-D");
  assert_eq!(file.subtitles()[0].start, 1000);

  let srt = file.to_string_with_format(&subtitler::model::Format::Srt);
  assert!(srt.contains("Erste Zeile der EBU-TT-D"));
  assert!(srt.contains("00:00:01,000"));
}

#[test]
fn ebu_tt_d_content_survives_the_ttml_round_trip() {
  let file = parse_bytes(SAMPLE.as_bytes()).unwrap();
  let ttml_out = file.to_string_with_format(&subtitler::model::Format::Ttml);
  let reparsed = parse_bytes(ttml_out.as_bytes()).unwrap();
  assert_eq!(reparsed.subtitles().len(), 2);
  assert_eq!(reparsed.subtitles()[1].text, "Zweite Zeile");
  assert_eq!(reparsed.subtitles()[1].start, 4000);
}

#[test]
fn imsc_validation_applies_to_ebu_tt_d() {
  // Missing xml:lang on an EBU-TT-D document is flagged by the IMSC
  // profile check (EBU-TT-D requires xml:lang too).
  let doc = r#"<tt xmlns="http://www.w3.org/ns/ttml" xmlns:ebu="urn:ebu:tt:metadata"><body><p begin="00:00:01.000" end="00:00:02.000">x</p></body></tt>"#;
  let issues = subtitler::imsc::imsc_validate(doc);
  assert!(issues.iter().any(|i| matches!(
    i,
    subtitler::model::ValidationIssue::ImscMissingAttribute { .. }
  )));
}
