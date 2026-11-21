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

/// One Secret Guard finding in the dry-run object: kind and byte range only.
#[derive(Serialize)]
pub struct DryRunFinding {
    pub kind: String,
    pub start: usize,
    pub end: usize,
}

/// The Secret Guard part of the dry-run object.
#[derive(Serialize)]
pub struct DryRunSecretGuard {
    /// `send`, `redact` or `ask`.
    pub decision: String,
    pub findings: Vec<DryRunFinding>,
}

/// The object printed by `--dry-run --json`. It holds the text that would be
/// sent, so it has no `Debug`. Field order here is the order in the output.
#[derive(Serialize)]
pub struct DryRunOutput {
    pub schema_version: u32,
    pub dry_run: bool,
    pub provider: String,
    pub host: Option<String>,
    pub model: Option<String>,
    pub source: Option<String>,
    pub target: Option<String>,
    pub pair: Option<Vec<String>>,
    pub second_call_possible: bool,
    pub would_send: bool,
    pub credentials: String,
    pub secret_guard: DryRunSecretGuard,
    pub text: Option<String>,
    pub warnings: Vec<String>,
}

impl DryRunOutput {
    /// The object as one line of JSON.
    pub fn render(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_the_dry_run_fields_in_order() {
        let output = DryRunOutput {
            schema_version: SCHEMA_VERSION,
            dry_run: true,
            provider: "google".to_string(),
            host: Some("translation.googleapis.com".to_string()),
            model: None,
            source: None,
            target: Some("th".to_string()),
            pair: None,
            second_call_possible: false,
            would_send: true,
            credentials: "none".to_string(),
            secret_guard: DryRunSecretGuard {
                decision: "redact".to_string(),
                findings: vec![DryRunFinding {
                    kind: "JWT".to_string(),
                    start: 1,
                    end: 2,
                }],
            },
            text: Some("hi".to_string()),
            warnings: Vec::new(),
        };
        assert_eq!(
            output.render(),
            r#"{"schema_version":1,"dry_run":true,"provider":"google","host":"translation.googleapis.com","model":null,"source":null,"target":"th","pair":null,"second_call_possible":false,"would_send":true,"credentials":"none","secret_guard":{"decision":"redact","findings":[{"kind":"JWT","start":1,"end":2}]},"text":"hi","warnings":[]}"#
        );
    }

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
