//! The config types: what a config file holds and what the merge produces.

use std::collections::HashMap;
use std::fmt;

use serde::Deserialize;

use crate::credentials::Secret;
use crate::language::LanguageCode;

/// A built-in Provider that a config file may select.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ProviderName {
    /// Google Cloud Translation.
    Google,
    /// Anthropic Claude.
    Anthropic,
}

impl ProviderName {
    /// Every built-in Provider.
    pub const ALL: [ProviderName; 2] = [ProviderName::Google, ProviderName::Anthropic];
}

/// Two different languages configured together (a Language Pair).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguagePair {
    first: LanguageCode,
    second: LanguageCode,
}

impl LanguagePair {
    /// Returns `None` when both codes are the same language.
    pub fn new(first: LanguageCode, second: LanguageCode) -> Option<Self> {
        if first == second {
            return None;
        }
        Some(LanguagePair { first, second })
    }

    /// The first language of the pair.
    pub fn first(&self) -> &LanguageCode {
        &self.first
    }

    /// The second language of the pair.
    pub fn second(&self) -> &LanguageCode {
        &self.second
    }
}

/// Stands in for a free-text value in `Debug` output.
///
/// A user may paste a credential into any free-text setting (`style`,
/// `endpoint`, `protected_terms`, ...), so the config types never print those.
pub(super) struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// `None` stays `None`; `Some(text)` becomes `Some(<redacted>)`.
fn redact_text(value: &Option<String>) -> Option<Redacted> {
    value.as_ref().map(|_| Redacted)
}

/// Same as [`redact_text`] for an optional list of strings.
fn redact_list(value: &Option<Vec<String>>) -> Option<Redacted> {
    value.as_ref().map(|_| Redacted)
}

/// Per-Provider settings from `[providers.<name>]`.
#[derive(Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderSettings {
    /// The model name to use, if the file sets one.
    pub model: Option<String>,
    /// User Config only.
    pub endpoint: Option<String>,
}

impl fmt::Debug for ProviderSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderSettings")
            .field("model", &redact_text(&self.model))
            .field("endpoint", &redact_text(&self.endpoint))
            .finish()
    }
}

/// The `[secret_guard]` table.
#[derive(Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct SecretGuardSettings {
    /// Extra regex patterns the Secret Guard should treat as secrets.
    pub extra_patterns: Vec<String>,
    /// Redact detected secrets instead of refusing the input.
    pub force_redact: bool,
    /// User Config only.
    pub allow_secrets: bool,
}

impl fmt::Debug for SecretGuardSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretGuardSettings")
            .field("extra_patterns", &Redacted)
            .field("force_redact", &self.force_redact)
            .field("allow_secrets", &self.allow_secrets)
            .finish()
    }
}

/// The `[credentials]` table. Allowed in the User Config only (ADR 0003).
///
/// The values stay private: read one with [`FileCredentials::api_key`], which
/// wraps it in a [`Secret`]. `Debug` shows only whether a key is set.
#[derive(Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileCredentials {
    google_api_key: Option<String>,
    anthropic_api_key: Option<String>,
}

impl FileCredentials {
    /// The API key stored for `provider`, as written in the file.
    pub fn api_key(&self, provider: ProviderName) -> Option<Secret> {
        let value = match provider {
            ProviderName::Google => &self.google_api_key,
            ProviderName::Anthropic => &self.anthropic_api_key,
        };
        value.as_ref().map(|text| Secret::new(text.clone()))
    }

    /// True when the table sets no key at all.
    pub(super) fn is_empty(&self) -> bool {
        self.google_api_key.is_none() && self.anthropic_api_key.is_none()
    }
}

impl fmt::Debug for FileCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileCredentials")
            .field("google_api_key", &redact_text(&self.google_api_key))
            .field("anthropic_api_key", &redact_text(&self.anthropic_api_key))
            .finish()
    }
}

/// The validated content of one config file (Project Config or User Config).
///
/// `Debug` hides free-text values (see `Redacted`).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct FileConfig {
    /// The Target Language.
    pub target: Option<LanguageCode>,
    /// The Language Pair.
    pub pair: Option<LanguagePair>,
    /// The Provider to use.
    pub provider: Option<ProviderName>,
    /// `None` = not set in this file; `Some(vec![])` = explicitly cleared.
    pub protected_terms: Option<Vec<String>>,
    /// A free-text style hint for the Provider.
    pub style: Option<String>,
    /// The input limit in characters.
    pub max_chars: Option<usize>,
    /// The Provider timeout in seconds.
    pub timeout_secs: Option<u64>,
    /// The `[secret_guard]` table.
    pub secret_guard: SecretGuardSettings,
    /// The `[providers.<name>]` tables.
    pub providers: HashMap<ProviderName, ProviderSettings>,
    /// User Config only. Always empty in a Project Config.
    pub credentials: FileCredentials,
    /// True when the User Config had a `[credentials]` value but the file is
    /// readable by group or others, so the values were dropped (§6). The CLI
    /// uses this to warn.
    pub credentials_ignored_insecure: bool,
}

impl fmt::Debug for FileConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileConfig")
            .field("target", &self.target)
            .field("pair", &self.pair)
            .field("provider", &self.provider)
            .field("protected_terms", &redact_list(&self.protected_terms))
            .field("style", &redact_text(&self.style))
            .field("max_chars", &self.max_chars)
            .field("timeout_secs", &self.timeout_secs)
            .field("secret_guard", &self.secret_guard)
            .field("providers", &self.providers)
            .field("credentials", &Redacted)
            .field(
                "credentials_ignored_insecure",
                &self.credentials_ignored_insecure,
            )
            .finish()
    }
}

/// Default input limit in characters (requirements §10).
pub const DEFAULT_MAX_CHARS: usize = 10_000;

/// Default Provider timeout in seconds (requirements §10).
pub const DEFAULT_TIMEOUT_SECS: u64 = 15;

/// Settings given on the command line. The CLI Adapter fills this in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overrides {
    /// `--to`: the Target Language.
    pub target: Option<LanguageCode>,
    /// `--provider`.
    pub provider: Option<ProviderName>,
    /// `--max-chars`.
    pub max_chars: Option<usize>,
    /// `--timeout`, in seconds.
    pub timeout_secs: Option<u64>,
    /// `--redact`.
    pub force_redact: bool,
    /// `--allow-secrets`.
    pub allow_secrets: bool,
}

/// The final settings after merging CLI, Project Config, User Config and defaults.
///
/// `Debug` hides free-text values (see `Redacted`).
#[derive(Clone, PartialEq, Eq)]
pub struct EffectiveConfig {
    /// The Target Language, if any layer set one.
    pub target: Option<LanguageCode>,
    /// The Language Pair, if any layer set one.
    pub pair: Option<LanguagePair>,
    /// The Provider to use (Google when no layer chose one).
    pub provider: ProviderName,
    /// Terms the Provider must leave untouched.
    pub protected_terms: Vec<String>,
    /// A free-text style hint for the Provider.
    pub style: Option<String>,
    /// The input limit in characters.
    pub max_chars: usize,
    /// The Provider timeout in seconds.
    pub timeout_secs: u64,
    /// The merged Secret Guard settings.
    pub secret_guard: SecretGuardSettings,
    /// The merged per-Provider settings.
    pub providers: HashMap<ProviderName, ProviderSettings>,
}

impl fmt::Debug for EffectiveConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EffectiveConfig")
            .field("target", &self.target)
            .field("pair", &self.pair)
            .field("provider", &self.provider)
            .field("protected_terms", &Redacted)
            .field("style", &redact_text(&self.style))
            .field("max_chars", &self.max_chars)
            .field("timeout_secs", &self.timeout_secs)
            .field("secret_guard", &self.secret_guard)
            .field("providers", &self.providers)
            .finish()
    }
}

impl EffectiveConfig {
    /// The settings for one Provider (all `None` when nothing is configured).
    pub fn provider_settings(&self, name: ProviderName) -> ProviderSettings {
        self.providers.get(&name).cloned().unwrap_or_default()
    }
}
