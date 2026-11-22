//! `kun --dry-run`: show exactly what a real run would send, and send nothing.
//!
//! The caller has already run the same steps as a real run (arguments, config,
//! size limit, Secret Guard, Target Language). This module applies Protected
//! Terms, names the credential *source* (never a key) and renders the report.
//! It never builds a Provider and never touches the network.

use kun_core::config::{EffectiveConfig, LanguagePair, ProviderName};
use kun_core::language::LanguageCode;
use kun_core::protected_terms::protect;
use kun_core::secret_guard::Finding;
use kun_core::translate::TargetPlan;

use crate::auth::key_source;
use crate::error::CliError;
use crate::guard::{DryDecision, DryGuarded};
use crate::json_output::{
    DryRunBudget, DryRunFinding, DryRunOutput, DryRunSecretGuard, SCHEMA_VERSION,
};
use crate::providers::{
    CredentialSources, anthropic_model, provider_host, provider_label, reject_endpoint,
};

const BEGIN_MARKER: &str = "----- begin text that would be sent -----";
const END_MARKER: &str = "----- end text that would be sent -----";

/// Everything a dry run reports. It holds the user's text, so it has no `Debug`.
pub struct DryRunReport {
    /// `google`, `anthropic` or `fake`.
    pub provider: String,
    pub host: Option<&'static str>,
    /// Anthropic only.
    pub model: Option<String>,
    /// `None` = the Provider detects it.
    pub source: Option<LanguageCode>,
    /// Set for a single Target Language.
    pub target: Option<LanguageCode>,
    /// Set for a Language Pair.
    pub pair: Option<LanguagePair>,
    /// A Language Pair with a detected Source Language may make a second call.
    pub second_call_possible: bool,
    /// `false` when a real run makes no request (Source equals Target).
    pub would_send: bool,
    pub credentials: &'static str,
    pub decision: DryDecision,
    pub findings: Vec<Finding>,
    /// After redaction and Protected Terms placeholders.
    pub text: String,
    /// `None` when no Usage Budget is set for the Provider. The caller fills
    /// it in after `build_report`.
    pub budget: Option<DryRunBudget>,
    pub warnings: Vec<String>,
}

impl DryRunReport {
    /// How many characters a real run would send in its first call: the text
    /// after redaction and Protected Terms placeholders (0 when nothing is sent).
    pub fn chars_to_send(&self) -> u64 {
        if self.would_send {
            u64::try_from(self.text.chars().count()).unwrap_or(u64::MAX)
        } else {
            0
        }
    }
}

/// Builds the report. Fails like a real run where a real run would fail before
/// sending (a configured endpoint). Missing credentials are *not* an error:
/// the report says `none`.
pub fn build_report(
    provider_name: &str,
    config: &EffectiveConfig,
    sources: &CredentialSources,
    source: Option<LanguageCode>,
    plan: &TargetPlan,
    guarded: DryGuarded,
    warnings: Vec<String>,
) -> Result<DryRunReport, CliError> {
    let provider = ProviderName::ALL
        .into_iter()
        .find(|candidate| provider_label(*candidate) == provider_name);

    // The fake Provider (tests only) has no host, no model and no credentials.
    let credentials = match provider {
        Some(name) => {
            reject_endpoint(name, config)?;
            key_source(name, sources.env, sources.store, sources.user_file).label()
        }
        None => "not needed (fake provider)",
    };
    let model = (provider == Some(ProviderName::Anthropic)).then(|| anthropic_model(config));

    let (target, pair) = match plan {
        TargetPlan::Fixed(code) => (Some(code.clone()), None),
        TargetPlan::Pair(pair) => (None, Some(pair.clone())),
        // The caller reports a missing Target Language before getting here.
        TargetPlan::Missing => return Err(CliError::Usage("no Target Language")),
    };
    // A Source Language equal to the Target Language needs no request at all.
    let would_send = match (&source, &target) {
        (Some(source), Some(target)) => source != target,
        _ => true,
    };
    let second_call_possible = pair.is_some() && source.is_none();

    let (text, _placeholders) = protect(&guarded.text, &config.protected_terms);

    Ok(DryRunReport {
        provider: provider_name.to_string(),
        host: provider_host(provider_name),
        model,
        source,
        target,
        pair,
        second_call_possible,
        would_send,
        credentials,
        decision: guarded.decision,
        findings: guarded.findings,
        text,
        budget: None,
        warnings,
    })
}

impl DryRunReport {
    /// The human report for stdout.
    pub fn render_human(&self) -> String {
        let mut out = String::from("Dry run: nothing was sent.\n");
        out.push_str(&format!("provider: {}\n", self.provider));
        if let Some(host) = self.host {
            out.push_str(&format!("host: {host}\n"));
        }
        if let Some(model) = &self.model {
            out.push_str(&format!("model: {model}\n"));
        }
        match &self.source {
            Some(code) => out.push_str(&format!("source language: {code}\n")),
            None => out.push_str("source language: detect\n"),
        }
        match (&self.target, &self.pair) {
            (Some(code), _) => out.push_str(&format!("target language: {code}\n")),
            (None, Some(pair)) => out.push_str(&format!(
                "language pair: {}, {}\n",
                pair.first(),
                pair.second()
            )),
            (None, None) => {}
        }
        if self.second_call_possible {
            out.push_str(
                "second call: possible (if the text is in the first language of the pair, \
                 it is sent again with the second as target)\n",
            );
        } else {
            out.push_str("second call: no\n");
        }
        out.push_str(&format!("credentials: {}\n", self.credentials));
        out.push_str(&self.render_guard());
        out.push_str(&self.render_budget());

        if !self.would_send {
            out.push_str(
                "nothing would be sent: the Source Language is the Target Language, \
                 so the text is returned unchanged\n",
            );
            return out;
        }
        out.push_str(BEGIN_MARKER);
        out.push('\n');
        out.push_str(&self.text);
        if !self.text.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(END_MARKER);
        out.push('\n');
        out
    }

    /// The Usage Budget lines. Empty when no budget is set.
    fn render_budget(&self) -> String {
        let Some(budget) = &self.budget else {
            return String::new();
        };
        let mut out = format!(
            "budget: {} of {} used this month; this run needs {}\n",
            budget.used, budget.limit, budget.needs
        );
        if !self.would_send {
            out.push_str("budget: no Provider call, so nothing would be counted\n");
        } else if budget.would_refuse {
            out.push_str("budget: a real run would be refused (exit code 8)\n");
        } else {
            out.push_str("budget: a real run would be sent\n");
        }
        if self.second_call_possible && self.would_send {
            out.push_str(&format!(
                "budget: a second call may need {} more\n",
                budget.needs
            ));
        }
        out
    }

    /// The Secret Guard lines: the decision and the findings (never values).
    fn render_guard(&self) -> String {
        let mut out = String::new();
        match self.decision {
            DryDecision::Send if self.findings.is_empty() => {
                out.push_str("Secret Guard: send (nothing found)\n");
            }
            DryDecision::Send => out.push_str("Secret Guard: send (sent as it is: allowed)\n"),
            DryDecision::Redact => {
                out.push_str("Secret Guard: redact (matches become [REDACTED])\n");
            }
            DryDecision::AskUser => out.push_str(
                "Secret Guard: ask (a real run would ask you first; \
                 the text below is what would be sent if you answered yes)\n",
            ),
        }
        for finding in &self.findings {
            out.push_str(&format!("  - {finding}\n"));
        }
        out
    }

    /// The `--json` object.
    pub fn to_json(&self) -> DryRunOutput {
        let findings = self
            .findings
            .iter()
            .map(|finding| DryRunFinding {
                kind: finding.kind.to_string(),
                start: finding.range.start,
                end: finding.range.end,
            })
            .collect();
        DryRunOutput {
            schema_version: SCHEMA_VERSION,
            dry_run: true,
            provider: self.provider.clone(),
            host: self.host.map(str::to_string),
            model: self.model.clone(),
            source: self.source.as_ref().map(ToString::to_string),
            target: self.target.as_ref().map(ToString::to_string),
            pair: self
                .pair
                .as_ref()
                .map(|pair| vec![pair.first().to_string(), pair.second().to_string()]),
            second_call_possible: self.second_call_possible,
            would_send: self.would_send,
            credentials: self.credentials.to_string(),
            secret_guard: DryRunSecretGuard {
                decision: self.decision.label().to_string(),
                findings,
            },
            text: self.would_send.then(|| self.text.clone()),
            budget: self.budget,
            warnings: self.warnings.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kun_core::config::{Overrides, merge};
    use kun_core::credentials::MemoryStore;
    use kun_core::secret_guard::SecretGuard;

    const KEY: &str = "AKIAFAKEEXAMPLE12345";

    fn lang(code: &str) -> LanguageCode {
        code.parse().unwrap()
    }

    fn no_env(_: &str) -> Option<String> {
        None
    }

    fn report_for(guarded: DryGuarded, source: Option<LanguageCode>) -> DryRunReport {
        let config = merge(&Overrides::default(), None, None);
        let store = MemoryStore::new();
        let sources = CredentialSources {
            env: &no_env,
            store: &store,
            user_file: None,
        };
        build_report(
            "google",
            &config,
            &sources,
            source,
            &TargetPlan::Fixed(lang("th")),
            guarded,
            Vec::new(),
        )
        .unwrap()
    }

    fn clean(text: &str) -> DryGuarded {
        DryGuarded {
            text: text.to_string(),
            decision: DryDecision::Send,
            findings: Vec::new(),
        }
    }

    #[test]
    fn the_report_never_contains_a_found_key() {
        let text = format!("key {KEY}");
        let findings = SecretGuard::new(&[]).unwrap().scan(&text);
        let guarded = DryGuarded {
            text: "key [REDACTED]".to_string(),
            decision: DryDecision::Redact,
            findings,
        };
        let report = report_for(guarded, None);
        let human = report.render_human();
        assert!(human.contains("possible AWS access key at bytes 4..24"));
        assert!(human.contains("key [REDACTED]"));
        assert!(!human.contains(KEY));
        assert!(!report.to_json().render().contains(KEY));
    }

    #[test]
    fn an_ask_decision_says_a_real_run_would_ask() {
        let guarded = DryGuarded {
            text: "x".to_string(),
            decision: DryDecision::AskUser,
            findings: Vec::new(),
        };
        let human = report_for(guarded, None).render_human();
        assert!(human.contains("a real run would ask you first"));
        assert!(human.contains("if you answered yes"));
    }

    #[test]
    fn the_text_is_shown_between_the_markers_with_one_newline_at_most() {
        let human = report_for(clean("hello"), None).render_human();
        assert!(human.ends_with(&format!("{BEGIN_MARKER}\nhello\n{END_MARKER}\n")));
        let human = report_for(clean("hello\n"), None).render_human();
        assert!(human.ends_with(&format!("{BEGIN_MARKER}\nhello\n{END_MARKER}\n")));
    }

    #[test]
    fn the_budget_lines_name_the_numbers_and_the_outcome() {
        let mut report = report_for(clean("hello"), None);
        assert!(!report.render_human().contains("budget:"));

        report.budget = Some(DryRunBudget {
            limit: 100,
            used: 40,
            needs: 5,
            would_refuse: false,
        });
        let human = report.render_human();
        assert!(human.contains("budget: 40 of 100 used this month; this run needs 5\n"));
        assert!(human.contains("budget: a real run would be sent\n"));
        assert_eq!(report.chars_to_send(), 5);

        report.budget = Some(DryRunBudget {
            limit: 42,
            used: 40,
            needs: 5,
            would_refuse: true,
        });
        assert!(
            report
                .render_human()
                .contains("a real run would be refused")
        );
    }

    #[test]
    fn a_dry_run_without_a_request_says_no_provider_call_and_never_refuses() {
        let mut report = report_for(clean("hello"), Some(lang("th")));
        report.budget = Some(DryRunBudget {
            limit: 5,
            used: 10,
            needs: 0,
            would_refuse: false,
        });
        let human = report.render_human();
        assert!(human.contains("budget: 10 of 5 used this month; this run needs 0\n"));
        assert!(human.contains("no Provider call"));
        assert!(!human.contains("refused"));
    }

    #[test]
    fn nothing_to_send_needs_no_budget() {
        let report = report_for(clean("hello"), Some(lang("th")));
        assert_eq!(report.chars_to_send(), 0);
    }

    #[test]
    fn the_same_source_and_target_sends_nothing() {
        let report = report_for(clean("hello"), Some(lang("th")));
        assert!(!report.would_send);
        assert!(!report.render_human().contains(BEGIN_MARKER));
        assert_eq!(report.to_json().text, None);
    }

    #[test]
    fn the_fake_provider_has_no_host_model_or_credentials() {
        let config = merge(&Overrides::default(), None, None);
        let store = MemoryStore::new();
        let sources = CredentialSources {
            env: &no_env,
            store: &store,
            user_file: None,
        };
        let report = build_report(
            "fake",
            &config,
            &sources,
            None,
            &TargetPlan::Fixed(lang("th")),
            clean("hello"),
            Vec::new(),
        )
        .unwrap();
        assert_eq!(report.host, None);
        assert_eq!(report.model, None);
        assert!(report.credentials.starts_with("not needed"));
    }
}
