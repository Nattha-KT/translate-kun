//! The `--json` output: one JSON object on stdout. The schema is a public
//! contract (ADR 0002); see `docs/json-output.md`.

use serde::Serialize;

/// Bump this only for a breaking change to the object below.
pub const SCHEMA_VERSION: u32 = 1;

/// The object printed by `--json`. It holds the user's text, so it has no
/// `Debug`. Field order here is the order in the output.
#[derive(Serialize)]
pub struct JsonOutput {
    pub schema_version: u32,
    pub text: String,
    pub source: Option<String>,
    pub target: String,
    pub provider: String,
    pub warnings: Vec<String>,
}

impl JsonOutput {
    /// The object as one line of JSON.
    pub fn render(&self) -> String {
        // All fields are plain strings and numbers, so this cannot fail.
        serde_json::to_string(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_the_documented_fields_in_order() {
        let output = JsonOutput {
            schema_version: SCHEMA_VERSION,
            text: "สวัสดี \"x\"".to_string(),
            source: None,
            target: "th".to_string(),
            provider: "google".to_string(),
            warnings: vec!["w".to_string()],
        };
        assert_eq!(
            output.render(),
            r#"{"schema_version":1,"text":"สวัสดี \"x\"","source":null,"target":"th","provider":"google","warnings":["w"]}"#
        );
    }
}
