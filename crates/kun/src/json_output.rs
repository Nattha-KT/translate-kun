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

/// The Usage Budget part of a dry run. Numbers only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DryRunBudget {
    pub limit: u64,
    /// Characters counted so far this month.
    pub used: u64,
    /// Characters this run would send (the first call).
    pub needs: u64,
    /// A real run would be refused by the budget.
    pub would_refuse: bool,
}

/// One Provider in the `kun budget status --json` object.
#[derive(Serialize)]
pub struct BudgetStatusProvider {
    /// `null` = no limit.
    pub limit: Option<u64>,
    pub used: u64,
    /// `null` = no limit.
    pub remaining: Option<u64>,
}

/// The Providers in the `kun budget status --json` object.
#[derive(Serialize)]
pub struct BudgetStatusProviders {
    pub google: BudgetStatusProvider,
    pub anthropic: BudgetStatusProvider,
}

/// The object printed by `kun budget status --json`. Numbers and dates only.
/// Field order here is the order in the output.
#[derive(Serialize)]
pub struct BudgetStatusOutput {
    pub schema_version: u32,
    /// The UTC month the counters belong to, `YYYY-MM`.
    pub month: String,
    /// The date the counters reset, `YYYY-MM-DD` (UTC).
    pub resets_on: String,
    pub providers: BudgetStatusProviders,
}

impl BudgetStatusOutput {
    /// The object as one line of JSON.
    pub fn render(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
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
    /// `null` when no Usage Budget is set for the Provider.
    pub budget: Option<DryRunBudget>,
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
            budget: Some(DryRunBudget {
                limit: 10,
                used: 4,
                needs: 2,
                would_refuse: false,
            }),
            warnings: Vec::new(),
        };
        assert_eq!(
            output.render(),
            r#"{"schema_version":1,"dry_run":true,"provider":"google","host":"translation.googleapis.com","model":null,"source":null,"target":"th","pair":null,"second_call_possible":false,"would_send":true,"credentials":"none","secret_guard":{"decision":"redact","findings":[{"kind":"JWT","start":1,"end":2}]},"text":"hi","budget":{"limit":10,"used":4,"needs":2,"would_refuse":false},"warnings":[]}"#
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
