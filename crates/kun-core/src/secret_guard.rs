//! The Secret Guard: the check that inspects outgoing text for credentials
//! before it is sent to a Provider (requirements §7).
//!
//! This module only *finds* and *redacts* secrets and *decides* what to do
//! about them. It does no terminal I/O: asking the user is the CLI's job.
//!
//! Typical use:
//!
//! ```
//! use kun_core::secret_guard::{decide, redact, Decision, GuardPolicy, SecretGuard};
//!
//! let guard = SecretGuard::new(&[]).unwrap();
//! let text = "hello";
//! let findings = guard.scan(text);
//! let policy = GuardPolicy { allow_secrets: false, force_redact: false };
//! match decide(&findings, policy, false) {
//!     Decision::Send => { /* send `text` */ }
//!     Decision::Redact => { let _safe = redact(text, &findings); }
//!     Decision::AskUser | Decision::Block => { /* handled by the CLI */ }
//! }
//! ```

use std::fmt;
use std::ops::Range;

use regex::Regex;

/// The text that replaces each secret when redacting.
pub const REDACTED: &str = "[REDACTED]";

/// What kind of credential a [`Finding`] looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKind {
    AwsAccessKey,
    GitHubToken,
    AnthropicApiKey,
    OpenAiApiKey,
    GoogleApiKey,
    SlackToken,
    PrivateKeyBlock,
    Jwt,
    /// Matched one of the extra patterns given to [`SecretGuard::new`].
    Custom,
}

impl fmt::Display for SecretKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            SecretKind::AwsAccessKey => "AWS access key",
            SecretKind::GitHubToken => "GitHub token",
            SecretKind::AnthropicApiKey => "Anthropic API key",
            SecretKind::OpenAiApiKey => "OpenAI API key",
            SecretKind::GoogleApiKey => "Google API key",
            SecretKind::SlackToken => "Slack token",
            SecretKind::PrivateKeyBlock => "private key block",
            SecretKind::Jwt => "JWT",
            SecretKind::Custom => "secret (custom pattern)",
        };
        f.write_str(name)
    }
}

/// One possible secret found in a text.
///
/// It stores only the kind and the byte range, never the matched text, so
/// printing a `Finding` (with `{}` or `{:?}`) can never leak the secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub kind: SecretKind,
    /// Byte range of the match inside the scanned text.
    pub range: Range<usize>,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "possible {} at bytes {}..{}",
            self.kind, self.range.start, self.range.end
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SecretGuardError {
    /// An extra pattern is not a valid regular expression.
    /// `index` is its position in the `extra` list (0-based).
    ///
    /// Deliberately holds no pattern text and no `regex::Error` (whose
    /// message quotes the pattern): a user may have pasted a real key as a
    /// pattern, and this error may be printed.
    #[error("extra secret pattern #{index} is not a valid regex")]
    InvalidPattern { index: usize },
}

/// The built-in, high-precision patterns. They are always active: there is
/// deliberately no way to remove or disable one (ADR 0003).
///
/// The `regex` crate runs in linear time, so even a hostile extra pattern
/// cannot make a scan hang.
///
/// `(?-u:\b)` is an ASCII-only word boundary. A plain `\b` is Unicode-aware
/// and treats Thai letters as word characters, so a key written straight
/// after a Thai word (Thai has no spaces) would be missed.
const BUILT_IN: &[(SecretKind, &str)] = &[
    (
        SecretKind::AwsAccessKey,
        r"(?-u:\b)(?:AKIA|ASIA)[A-Z0-9]{16}(?-u:\b)",
    ),
    (
        SecretKind::GitHubToken,
        r"(?-u:\b)(?:gh[pousr]_[A-Za-z0-9]{36,255}|github_pat_[A-Za-z0-9_]{22,255})(?-u:\b)",
    ),
    (
        SecretKind::AnthropicApiKey,
        r"(?-u:\b)sk-ant-[A-Za-z0-9_\-]{20,}",
    ),
    // OpenAI legacy keys: `sk-` + 40 or more letters/digits only. No `-` or
    // `_`, so long kebab-case names such as `sk-my-resource-name` are ignored.
    // Neither OpenAI pattern can match an Anthropic `sk-ant-…` key (the `-`
    // after `ant` breaks the run), so those are reported once, as Anthropic.
    (SecretKind::OpenAiApiKey, r"(?-u:\b)sk-[A-Za-z0-9]{40,}"),
    // OpenAI prefixed keys may contain `-` and `_`.
    (
        SecretKind::OpenAiApiKey,
        r"(?-u:\b)sk-(?:proj|svcacct|admin)-[A-Za-z0-9_\-]{32,}",
    ),
    (SecretKind::GoogleApiKey, r"(?-u:\b)AIza[A-Za-z0-9_\-]{35}"),
    (
        SecretKind::SlackToken,
        r"(?-u:\b)xox[abprs]-[A-Za-z0-9\-]{10,}",
    ),
    // A PEM private key. A line break is a real newline or the two
    // characters `\n` (keys escaped inside JSON). Two shapes, tried in order:
    // 1. A whole block: BEGIN, lines of base64 (or `Name: value` headers, or
    //    blank), then END.
    // 2. A cut-off block: BEGIN followed by at least one base64 line of 20+
    //    characters; the match ends after the last such line.
    // A BEGIN header with no body (e.g. in documentation) is not a finding.
    (
        SecretKind::PrivateKeyBlock,
        concat!(
            r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----",
            r"(?:",
            r"(?:(?:\r?\n|\\n)[ \t]*(?:[A-Za-z0-9+/=]*|[A-Za-z-]+:[^\r\n\\]*)[ \t]*)*",
            r"(?:\r?\n|\\n)[ \t]*-----END [A-Z0-9 ]*PRIVATE KEY-----",
            r"|",
            r"(?:(?:\r?\n|\\n)[ \t]*[A-Za-z0-9+/=]{20,})+",
            r")",
        ),
    ),
    // Three base64url segments; the header always starts `eyJ` (`{"`).
    // The payload may be as short as `e30` (`{}`, empty claims). Real
    // signatures are 43+ characters, so 16 is a safe minimum.
    (
        SecretKind::Jwt,
        r"(?-u:\b)eyJ[A-Za-z0-9_\-]{5,}\.[A-Za-z0-9_\-]{2,}\.[A-Za-z0-9_\-]{16,}",
    ),
];

struct Pattern {
    kind: SecretKind,
    regex: Regex,
}

/// The built-in patterns plus any extra patterns, compiled once and ready to
/// scan many texts.
pub struct SecretGuard {
    patterns: Vec<Pattern>,
}

impl SecretGuard {
    /// Builds a Secret Guard from the built-in patterns plus `extra` regexes
    /// (for example from the Project Config). Extra patterns can only *add*
    /// detections; their matches are reported as [`SecretKind::Custom`].
    pub fn new(extra: &[String]) -> Result<Self, SecretGuardError> {
        let mut patterns = Vec::new();
        for (kind, source) in BUILT_IN {
            // A broken built-in pattern is a bug in `kun`, caught by the tests.
            let regex = Regex::new(source).expect("built-in pattern is valid");
            patterns.push(Pattern { kind: *kind, regex });
        }
        for (index, source) in extra.iter().enumerate() {
            let regex =
                Regex::new(source).map_err(|_| SecretGuardError::InvalidPattern { index })?;
            patterns.push(Pattern {
                kind: SecretKind::Custom,
                regex,
            });
        }
        Ok(SecretGuard { patterns })
    }

    /// Finds every possible secret in `text`, in order of appearance.
    /// Findings from different patterns may overlap.
    pub fn scan(&self, text: &str) -> Vec<Finding> {
        let mut findings = Vec::new();
        for pattern in &self.patterns {
            for m in pattern.regex.find_iter(text) {
                // An empty match hides nothing, so it is not a finding.
                if m.is_empty() {
                    continue;
                }
                findings.push(Finding {
                    kind: pattern.kind,
                    range: m.range(),
                });
            }
        }
        // Stable sort: findings at the same position keep pattern order.
        findings.sort_by_key(|f| f.range.start);
        findings
    }
}

/// Returns `text` with each finding replaced by `[REDACTED]`.
///
/// Findings that overlap or touch become a single `[REDACTED]`, so no byte of
/// a secret survives. `findings` must come from scanning this same `text`.
pub fn redact(text: &str, findings: &[Finding]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut copied_up_to = 0;
    for range in merge_ranges(findings) {
        out.push_str(&text[copied_up_to..range.start]);
        out.push_str(REDACTED);
        copied_up_to = range.end;
    }
    out.push_str(&text[copied_up_to..]);
    out
}

/// Sorts the findings' ranges and joins any that overlap or touch, so each
/// returned range is separate from the next one.
fn merge_ranges(findings: &[Finding]) -> Vec<Range<usize>> {
    let mut ranges: Vec<Range<usize>> = findings.iter().map(|f| f.range.clone()).collect();
    ranges.sort_by_key(|r| r.start);

    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => {
                last.end = last.end.max(range.end);
            }
            _ => merged.push(range),
        }
    }
    merged
}

/// The settings that change what the Secret Guard does with findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuardPolicy {
    /// Send secrets anyway (`--allow-secrets`; CLI flag or User Config only).
    pub allow_secrets: bool,
    /// Always redact (`--redact`, or forced on by the Project Config).
    pub force_redact: bool,
}

/// What to do with the outgoing text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Send the text as it is.
    Send,
    /// Send the text after [`redact`].
    Redact,
    /// Ask the user `Send anyway? [y/N]` (default No).
    AskUser,
    /// Do not send (exit 6).
    Block,
}

/// Decides what to do with the findings (requirements §7). The first rule
/// that applies wins:
///
/// 1. no findings → `Send`
/// 2. `force_redact` → `Redact`
/// 3. `allow_secrets` → `Send`
/// 4. interactive terminal → `AskUser`
/// 5. otherwise (pipe, script) → `Block`
pub fn decide(findings: &[Finding], policy: GuardPolicy, interactive: bool) -> Decision {
    if findings.is_empty() {
        Decision::Send
    } else if policy.force_redact {
        Decision::Redact
    } else if policy.allow_secrets {
        Decision::Send
    } else if interactive {
        Decision::AskUser
    } else {
        Decision::Block
    }
}
