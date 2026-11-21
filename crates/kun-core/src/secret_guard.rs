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
//!     // `Decision` is `#[non_exhaustive]`, so outside this crate a `_` arm is required.
//!     _ => { /* AskUser, Block or a future decision: handled by the CLI */ }
//! }
//! ```

use std::fmt;
use std::ops::Range;

use regex::Regex;

/// The text that replaces each secret when redacting.
pub const REDACTED: &str = "[REDACTED]";

/// What kind of credential a [`Finding`] looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SecretKind {
    /// An AWS access key ID.
    AwsAccessKey,
    /// A GitHub token.
    GitHubToken,
    /// An Anthropic API key.
    AnthropicApiKey,
    /// An OpenAI API key.
    OpenAiApiKey,
    /// A Google API key.
    GoogleApiKey,
    /// A Slack token.
    SlackToken,
    /// A PEM private key block.
    PrivateKeyBlock,
    /// A JSON Web Token.
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
    /// What kind of credential the match looks like.
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

/// Why a [`SecretGuard`] could not be built.
#[derive(Debug, thiserror::Error)]
pub enum SecretGuardError {
    /// An extra pattern is not a valid regular expression.
    /// `index` is its position in the `extra` list (0-based).
    ///
    /// Deliberately holds no pattern text and no `regex::Error` (whose
    /// message quotes the pattern): a user may have pasted a real key as a
    /// pattern, and this error may be printed.
    #[error("extra secret pattern #{index} is not a valid regex")]
    InvalidPattern {
        /// Position of the bad pattern in the `extra` list (0-based).
        index: usize,
    },
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
const BUILT_IN: &[BuiltIn] = &[
    BuiltIn {
        kind: SecretKind::AwsAccessKey,
        regex: r"(?-u:\b)(?:AKIA|ASIA)[A-Z0-9]{16}(?-u:\b)",
        not_followed_by: &[],
    },
    BuiltIn {
        kind: SecretKind::GitHubToken,
        regex: r"(?-u:\b)(?:gh[pousr]_[A-Za-z0-9]{36,255}|github_pat_[A-Za-z0-9_]{22,255})(?-u:\b)",
        not_followed_by: &[],
    },
    BuiltIn {
        kind: SecretKind::AnthropicApiKey,
        regex: r"(?-u:\b)sk-ant-[A-Za-z0-9_\-]{20,}",
        not_followed_by: &[],
    },
    // OpenAI legacy keys: `sk-` + 40 or more letters/digits only. If the run
    // goes on with `-` or `_` it is a long kebab/snake-case name (for example
    // a Kubernetes resource), not a key.
    // Neither OpenAI pattern can match an Anthropic `sk-ant-…` key (the `-`
    // after `ant` breaks the run), so those are reported once, as Anthropic.
    BuiltIn {
        kind: SecretKind::OpenAiApiKey,
        regex: r"(?-u:\b)sk-[A-Za-z0-9]{40,}",
        not_followed_by: &['-', '_'],
    },
    // OpenAI prefixed keys may contain `-` and `_`.
    BuiltIn {
        kind: SecretKind::OpenAiApiKey,
        regex: r"(?-u:\b)sk-(?:proj|svcacct|admin)-[A-Za-z0-9_\-]{32,}",
        not_followed_by: &[],
    },
    BuiltIn {
        kind: SecretKind::GoogleApiKey,
        regex: r"(?-u:\b)AIza[A-Za-z0-9_\-]{35}",
        not_followed_by: &[],
    },
    BuiltIn {
        kind: SecretKind::SlackToken,
        regex: r"(?-u:\b)xox[abprs]-[A-Za-z0-9\-]{10,}",
        not_followed_by: &[],
    },
    // PEM private keys are not here: see `private_key_block`.

    // Three base64url segments; the header always starts `eyJ` (`{"`).
    // The payload may be as short as `e30` (`{}`, empty claims).
    BuiltIn {
        kind: SecretKind::Jwt,
        regex: r"(?-u:\b)eyJ[A-Za-z0-9_\-]{5,}\.[A-Za-z0-9_\-]{2,}\.[A-Za-z0-9_\-]{5,}",
        not_followed_by: &[],
    },
];

/// One built-in pattern, before it is compiled.
struct BuiltIn {
    kind: SecretKind,
    regex: &'static str,
    /// A match is dropped if the character right after it is one of these.
    /// (The `regex` crate has no look-ahead, so this is checked by hand.)
    not_followed_by: &'static [char],
}

struct Pattern {
    kind: SecretKind,
    regex: Regex,
    not_followed_by: &'static [char],
}

/// The first line of a PEM private key. What follows it decides whether it
/// is a finding (see `private_key_block`).
const PRIVATE_KEY_HEADER: &str = r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----";

/// The built-in patterns plus any extra patterns, compiled once and ready to
/// scan many texts.
pub struct SecretGuard {
    patterns: Vec<Pattern>,
    private_key_header: Regex,
}

impl SecretGuard {
    /// Builds a Secret Guard from the built-in patterns plus `extra` regexes
    /// (for example from the Project Config). Extra patterns can only *add*
    /// detections; their matches are reported as [`SecretKind::Custom`].
    pub fn new(extra: &[String]) -> Result<Self, SecretGuardError> {
        let mut patterns = Vec::new();
        for built_in in BUILT_IN {
            // A broken built-in pattern is a bug in `kun`, caught by the tests.
            let regex = Regex::new(built_in.regex).expect("built-in pattern is valid");
            patterns.push(Pattern {
                kind: built_in.kind,
                regex,
                not_followed_by: built_in.not_followed_by,
            });
        }
        for (index, source) in extra.iter().enumerate() {
            let regex =
                Regex::new(source).map_err(|_| SecretGuardError::InvalidPattern { index })?;
            patterns.push(Pattern {
                kind: SecretKind::Custom,
                regex,
                not_followed_by: &[],
            });
        }
        let private_key_header = Regex::new(PRIVATE_KEY_HEADER).expect("built-in pattern is valid");
        Ok(SecretGuard {
            patterns,
            private_key_header,
        })
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
                let next_char = text[m.end()..].chars().next();
                if next_char.is_some_and(|c| pattern.not_followed_by.contains(&c)) {
                    continue;
                }
                findings.push(Finding {
                    kind: pattern.kind,
                    range: m.range(),
                });
            }
        }
        for header in self.private_key_header.find_iter(text) {
            if let Some(range) = private_key_block(text, header.start(), header.end()) {
                findings.push(Finding {
                    kind: SecretKind::PrivateKeyBlock,
                    range,
                });
            }
        }
        // Stable sort: findings at the same position keep pattern order.
        findings.sort_by_key(|f| f.range.start);
        findings
    }
}

/// Decides whether a PEM private key header (at `header_start..header_end`)
/// starts a real key, and if so returns the range to report.
///
/// A line break is a real newline or the two characters `\n` (keys escaped
/// inside JSON). Walking the lines after the header:
///
/// 1. Look for the nearest END line before any next BEGIN header. If there
///    is one, the block is complete: the finding runs from the header through
///    END (whatever blank or metadata lines lie between), but only if at
///    least one base64 line lies between them.
/// 2. With no END, see `truncated_private_key_block`.
///
/// So a header on its own, or followed by a blank line or prose (as in
/// documentation), is not a finding.
fn private_key_block(text: &str, header_start: usize, header_end: usize) -> Option<Range<usize>> {
    let mut pos = header_end;
    let mut has_base64 = false;

    while let Some(line) = next_line(text, pos) {
        pos = line.raw_end;
        let content = &text[line.content_start..line.content_end];
        if content.contains("-----BEGIN ") {
            // Another key starts: this header has no END of its own.
            break;
        }
        match classify_pem_line(content) {
            PemLine::End { marker_len } => {
                if has_base64 {
                    return Some(header_start..line.content_start + marker_len);
                }
                return None;
            }
            PemLine::Base64 => has_base64 = true,
            PemLine::Metadata | PemLine::Blank | PemLine::Other => {}
        }
    }

    truncated_private_key_block(text, header_start, header_end)
}

/// A PEM private key with no END line (for example, cut off when copying).
///
/// The block runs over the following base64 and `Name: value` metadata lines
/// and stops at a blank line, any other line, or the end of the text. It is
/// a finding only if at least one line was base64, and it ends after the
/// last base64 line, so metadata-like lines after the key (`Notes: …`) stay.
///
/// One blank line is allowed, once, right after `Proc-Type`/`DEK-Info`
/// metadata: encrypted PEM keys put it before the base64 body.
fn truncated_private_key_block(
    text: &str,
    header_start: usize,
    header_end: usize,
) -> Option<Range<usize>> {
    let mut pos = header_end;
    let mut has_base64 = false;
    let mut after_encryption_metadata = false;
    let mut blank_allowance_used = false;
    let mut block_end = header_end;

    while let Some(line) = next_line(text, pos) {
        pos = line.raw_end;
        let content = &text[line.content_start..line.content_end];
        match classify_pem_line(content) {
            PemLine::Base64 => {
                has_base64 = true;
                after_encryption_metadata = false;
                block_end = line.content_end;
            }
            PemLine::Metadata => {
                after_encryption_metadata =
                    content.starts_with("Proc-Type:") || content.starts_with("DEK-Info:");
            }
            PemLine::Blank if after_encryption_metadata && !blank_allowance_used => {
                blank_allowance_used = true;
                after_encryption_metadata = false;
            }
            PemLine::Blank | PemLine::Other | PemLine::End { .. } => break,
        }
    }

    if has_base64 {
        Some(header_start..block_end)
    } else {
        None
    }
}

/// What one line after a PEM header looks like.
enum PemLine {
    /// `-----END … PRIVATE KEY-----`; `marker_len` is its length in bytes.
    End {
        marker_len: usize,
    },
    /// 4 or more characters of `A-Za-z0-9+/=`.
    Base64,
    /// `Name: value`, e.g. `Proc-Type: 4,ENCRYPTED`.
    Metadata,
    Blank,
    Other,
}

fn classify_pem_line(content: &str) -> PemLine {
    if content.is_empty() {
        return PemLine::Blank;
    }
    if let Some(marker_len) = end_marker_len(content) {
        return PemLine::End { marker_len };
    }
    let is_base64 = content.len() >= 4
        && content
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=');
    if is_base64 {
        return PemLine::Base64;
    }
    let is_metadata = match content.split_once(':') {
        Some((name, _value)) => {
            !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        }
        None => false,
    };
    if is_metadata {
        PemLine::Metadata
    } else {
        PemLine::Other
    }
}

/// If `content` starts with `-----END <optional words> PRIVATE KEY-----`,
/// returns that marker's length in bytes.
fn end_marker_len(content: &str) -> Option<usize> {
    const START: &str = "-----END ";
    const TAIL: &str = "PRIVATE KEY-----";
    let rest = content.strip_prefix(START)?;
    let label_len = rest.find(TAIL)?;
    let label = &rest[..label_len];
    let label_ok = label
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == ' ');
    if label_ok {
        Some(START.len() + label_len + TAIL.len())
    } else {
        None
    }
}

/// One line of text, as byte positions.
struct Line {
    /// Where the line's content starts and ends, without surrounding spaces,
    /// tabs or `\r`.
    content_start: usize,
    content_end: usize,
    /// Where the line ends: at the next line break, or the end of the text.
    raw_end: usize,
}

/// Reads the line that starts after the line break at `pos`. Returns `None`
/// if there is no line break at `pos` (the end of the text, or more text on
/// the same line).
fn next_line(text: &str, pos: usize) -> Option<Line> {
    let rest = &text[pos..];
    let break_len = if rest.starts_with("\r\n") {
        2
    } else if rest.starts_with('\n') {
        1
    } else if rest.starts_with(r"\n") {
        2
    } else {
        return None;
    };
    let start = pos + break_len;

    // Find the next line break. Working on bytes is safe here: `\n` and `\`
    // are ASCII, so they never appear inside a multi-byte character.
    let bytes = &text.as_bytes()[start..];
    let mut len = 0;
    while len < bytes.len() {
        let real_newline = bytes[len] == b'\n';
        let escaped_newline = bytes[len] == b'\\' && bytes.get(len + 1) == Some(&b'n');
        if real_newline || escaped_newline {
            break;
        }
        len += 1;
    }

    let raw = &text[start..start + len];
    let without_trailing = raw.trim_end_matches([' ', '\t', '\r']);
    let trimmed = without_trailing.trim_start_matches([' ', '\t']);
    let content_end = start + without_trailing.len();
    Some(Line {
        content_start: content_end - trimmed.len(),
        content_end,
        raw_end: start + len,
    })
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
#[non_exhaustive]
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
