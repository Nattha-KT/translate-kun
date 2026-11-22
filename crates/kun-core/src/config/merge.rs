//! Merging the layers into one `EffectiveConfig`.

use std::collections::HashMap;

use super::schema::{
    DEFAULT_MAX_CHARS, DEFAULT_TIMEOUT_SECS, EffectiveConfig, FileConfig, Overrides, ProviderName,
    ProviderSettings, SecretGuardSettings,
};

/// Merge the layers. See the precedence table in the S2 slice spec.
pub fn merge(
    cli: &Overrides,
    project: Option<&FileConfig>,
    user: Option<&FileConfig>,
) -> EffectiveConfig {
    let empty = FileConfig::default();
    let project = project.unwrap_or(&empty);
    let user = user.unwrap_or(&empty);

    // Normal fields: CLI > Project > User > default.
    let target = cli
        .target
        .clone()
        .or_else(|| project.target.clone())
        .or_else(|| user.target.clone());
    let pair = project.pair.clone().or_else(|| user.pair.clone());
    let provider = cli
        .provider
        .or(project.provider)
        .or(user.provider)
        .unwrap_or(ProviderName::Google);
    let style = project.style.clone().or_else(|| user.style.clone());
    let max_chars = cli
        .max_chars
        .or(project.max_chars)
        .or(user.max_chars)
        .unwrap_or(DEFAULT_MAX_CHARS);
    let timeout_secs = cli
        .timeout_secs
        .or(project.timeout_secs)
        .or(user.timeout_secs)
        .unwrap_or(DEFAULT_TIMEOUT_SECS);

    // A normal field: Project wins when it sets the list (even to `[]`).
    let protected_terms = project
        .protected_terms
        .clone()
        .or_else(|| user.protected_terms.clone())
        .unwrap_or_default();

    // Security fields can only get stricter, or come from a trusted layer.
    let secret_guard = SecretGuardSettings {
        extra_patterns: union(
            &user.secret_guard.extra_patterns,
            &project.secret_guard.extra_patterns,
        ),
        force_redact: cli.force_redact
            || project.secret_guard.force_redact
            || user.secret_guard.force_redact,
        // Never from the Project Config, even if one was built by hand.
        allow_secrets: cli.allow_secrets || user.secret_guard.allow_secrets,
    };

    let mut providers = HashMap::new();
    for name in ProviderName::ALL {
        let from_project = project.providers.get(&name).cloned().unwrap_or_default();
        let from_user = user.providers.get(&name).cloned().unwrap_or_default();
        let settings = ProviderSettings {
            model: from_project.model.or(from_user.model),
            // Endpoints come from the User Config only (ADR 0003).
            endpoint: from_user.endpoint,
        };
        if settings != ProviderSettings::default() {
            providers.insert(name, settings);
        }
    }

    EffectiveConfig {
        target,
        pair,
        provider,
        protected_terms,
        style,
        max_chars,
        timeout_secs,
        secret_guard,
        providers,
        // The Usage Budget comes from the User Config only (ADR 0003).
        budget: user.budget,
    }
}

/// `first` followed by the items of `second` not already present. Duplicates
/// inside either list are dropped too.
fn union(first: &[String], second: &[String]) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    for item in first.iter().chain(second) {
        if !result.contains(item) {
            result.push(item.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::{FileCredentials, LanguagePair};
    use crate::config::trust::parse_project_config;
    use crate::language::LanguageCode;
    use std::path::PathBuf;

    fn path() -> PathBuf {
        PathBuf::from("config.toml")
    }

    fn lang(code: &str) -> LanguageCode {
        code.parse().unwrap()
    }

    fn pair(a: &str, b: &str) -> LanguagePair {
        LanguagePair::new(lang(a), lang(b)).unwrap()
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn the_budget_comes_from_the_user_config_only() {
        use crate::config::schema::BudgetSettings;

        let user = FileConfig {
            budget: BudgetSettings {
                google_chars_per_month: Some(450_000),
                anthropic_chars_per_month: None,
            },
            ..FileConfig::default()
        };
        // A hand-built Project layer (the parser would reject it) is ignored.
        let project = FileConfig {
            budget: BudgetSettings {
                google_chars_per_month: Some(u64::MAX),
                anthropic_chars_per_month: Some(1),
            },
            ..FileConfig::default()
        };

        let config = merge(&Overrides::default(), Some(&project), Some(&user));
        assert_eq!(config.budget.limit(ProviderName::Google), Some(450_000));
        assert_eq!(config.budget.limit(ProviderName::Anthropic), None);

        let none = merge(&Overrides::default(), Some(&project), None);
        assert_eq!(none.budget, BudgetSettings::default());
    }

    #[test]
    fn defaults_apply_when_nothing_is_set() {
        let config = merge(&Overrides::default(), None, None);
        assert_eq!(config.max_chars, 10_000);
        assert_eq!(config.timeout_secs, 15);
        assert_eq!(config.provider, ProviderName::Google);
        assert_eq!(config.target, None);
        assert_eq!(config.pair, None);
        assert!(config.protected_terms.is_empty());
        assert_eq!(config.style, None);
        assert_eq!(config.secret_guard, SecretGuardSettings::default());
    }

    #[test]
    fn normal_fields_prefer_cli_then_project_then_user() {
        let user = FileConfig {
            target: Some(lang("fr")),
            pair: Some(pair("en", "fr")),
            provider: Some(ProviderName::Google),
            style: Some("user style".into()),
            max_chars: Some(300),
            timeout_secs: Some(30),
            ..FileConfig::default()
        };
        let project = FileConfig {
            target: Some(lang("de")),
            pair: Some(pair("en", "de")),
            provider: Some(ProviderName::Anthropic),
            style: Some("project style".into()),
            max_chars: Some(200),
            timeout_secs: Some(20),
            ..FileConfig::default()
        };
        let cli = Overrides {
            target: Some(lang("th")),
            provider: Some(ProviderName::Google),
            max_chars: Some(100),
            timeout_secs: Some(10),
            ..Overrides::default()
        };

        let all = merge(&cli, Some(&project), Some(&user));
        assert_eq!(all.target, Some(lang("th")));
        assert_eq!(all.provider, ProviderName::Google);
        assert_eq!(all.max_chars, 100);
        assert_eq!(all.timeout_secs, 10);
        // `pair` and `style` have no CLI override: Project wins.
        assert_eq!(all.pair, Some(pair("en", "de")));
        assert_eq!(all.style.as_deref(), Some("project style"));

        let no_cli = merge(&Overrides::default(), Some(&project), Some(&user));
        assert_eq!(no_cli.target, Some(lang("de")));
        assert_eq!(no_cli.provider, ProviderName::Anthropic);
        assert_eq!(no_cli.max_chars, 200);
        assert_eq!(no_cli.timeout_secs, 20);

        let user_only = merge(&Overrides::default(), None, Some(&user));
        assert_eq!(user_only.target, Some(lang("fr")));
        assert_eq!(user_only.provider, ProviderName::Google);
        assert_eq!(user_only.max_chars, 300);
        assert_eq!(user_only.timeout_secs, 30);
        assert_eq!(user_only.pair, Some(pair("en", "fr")));
    }

    #[test]
    fn project_may_only_tighten_force_redact() {
        let on = FileConfig {
            secret_guard: SecretGuardSettings {
                force_redact: true,
                ..SecretGuardSettings::default()
            },
            ..FileConfig::default()
        };
        let off = FileConfig::default();

        assert!(
            merge(&Overrides::default(), Some(&on), Some(&off))
                .secret_guard
                .force_redact
        );
        assert!(
            merge(&Overrides::default(), Some(&off), Some(&on))
                .secret_guard
                .force_redact
        );
        let cli = Overrides {
            force_redact: true,
            ..Overrides::default()
        };
        assert!(
            merge(&cli, Some(&off), Some(&off))
                .secret_guard
                .force_redact
        );
        assert!(
            !merge(&Overrides::default(), Some(&off), Some(&off))
                .secret_guard
                .force_redact
        );
    }

    #[test]
    fn extra_patterns_are_a_deduplicated_union_in_order() {
        let user = FileConfig {
            secret_guard: SecretGuardSettings {
                extra_patterns: strings(&["A", "B"]),
                ..SecretGuardSettings::default()
            },
            ..FileConfig::default()
        };
        let project = FileConfig {
            secret_guard: SecretGuardSettings {
                extra_patterns: strings(&["C", "B", "C"]),
                ..SecretGuardSettings::default()
            },
            ..FileConfig::default()
        };
        let config = merge(&Overrides::default(), Some(&project), Some(&user));
        assert_eq!(
            config.secret_guard.extra_patterns,
            strings(&["A", "B", "C"])
        );
    }

    #[test]
    fn protected_terms_follow_normal_precedence_project_over_user() {
        let user = FileConfig {
            protected_terms: Some(strings(&["kun", "Rust"])),
            ..FileConfig::default()
        };
        let project = FileConfig {
            protected_terms: Some(strings(&["translate-kun"])),
            ..FileConfig::default()
        };

        let both = merge(&Overrides::default(), Some(&project), Some(&user));
        assert_eq!(both.protected_terms, strings(&["translate-kun"]));

        // A Project Config that does not set them leaves the User Config's alone.
        let unset = FileConfig::default();
        let user_wins = merge(&Overrides::default(), Some(&unset), Some(&user));
        assert_eq!(user_wins.protected_terms, strings(&["kun", "Rust"]));

        let user_only = merge(&Overrides::default(), None, Some(&user));
        assert_eq!(user_only.protected_terms, strings(&["kun", "Rust"]));
    }

    #[test]
    fn an_explicit_empty_protected_terms_list_clears_the_user_list() {
        let user = FileConfig {
            protected_terms: Some(strings(&["kun"])),
            ..FileConfig::default()
        };
        let project = parse_project_config(&path(), "protected_terms = []").unwrap();
        assert_eq!(project.protected_terms, Some(Vec::new()));

        let config = merge(&Overrides::default(), Some(&project), Some(&user));
        assert!(config.protected_terms.is_empty());
    }

    #[test]
    fn debug_output_never_shows_free_text_values() {
        let fake = "FAKE-EXAMPLE-KEY-0123456789";
        let mut providers = HashMap::new();
        providers.insert(
            ProviderName::Anthropic,
            ProviderSettings {
                model: Some(format!("model-{fake}")),
                endpoint: Some(format!("http://{fake}@host")),
            },
        );
        let file = FileConfig {
            target: Some(lang("th")),
            pair: Some(pair("en", "th")),
            provider: Some(ProviderName::Anthropic),
            protected_terms: Some(strings(&[fake])),
            style: Some(format!("style {fake}")),
            max_chars: Some(123),
            timeout_secs: Some(7),
            secret_guard: SecretGuardSettings {
                extra_patterns: strings(&[fake]),
                force_redact: true,
                allow_secrets: false,
            },
            providers,
            credentials: FileCredentials::default(),
            budget: Default::default(),
            credentials_ignored_insecure: false,
        };
        let effective = merge(&Overrides::default(), Some(&file), Some(&file));

        let outputs = [
            format!("{file:?}"),
            format!("{file:#?}"),
            format!("{effective:?}"),
            format!("{effective:#?}"),
            format!("{:?}", effective.secret_guard),
            format!("{:?}", effective.provider_settings(ProviderName::Anthropic)),
        ];
        for output in outputs {
            assert!(!output.contains(fake), "leaked in: {output}");
        }

        // Non-sensitive values stay visible, so the output is still useful.
        let shown = format!("{file:?}");
        assert!(shown.contains("<redacted>"));
        assert!(shown.contains("\"th\""));
        assert!(shown.contains("Anthropic"));
        assert!(shown.contains("123"));
        assert!(shown.contains("force_redact: true"));
    }

    #[test]
    fn allow_secrets_comes_from_cli_or_user_only() {
        let allowing = FileConfig {
            secret_guard: SecretGuardSettings {
                allow_secrets: true,
                ..SecretGuardSettings::default()
            },
            ..FileConfig::default()
        };
        let plain = FileConfig::default();

        // A Project Config value is ignored even if one got into the struct.
        let from_project = merge(&Overrides::default(), Some(&allowing), Some(&plain));
        assert!(!from_project.secret_guard.allow_secrets);

        let from_user = merge(&Overrides::default(), Some(&plain), Some(&allowing));
        assert!(from_user.secret_guard.allow_secrets);

        let cli = Overrides {
            allow_secrets: true,
            ..Overrides::default()
        };
        assert!(merge(&cli, None, None).secret_guard.allow_secrets);
    }

    #[test]
    fn endpoint_comes_from_user_only_and_model_prefers_project() {
        let mut user = FileConfig::default();
        user.providers.insert(
            ProviderName::Anthropic,
            ProviderSettings {
                model: Some("user-model".into()),
                endpoint: Some("http://localhost:11434".into()),
            },
        );
        let mut project = FileConfig::default();
        project.providers.insert(
            ProviderName::Anthropic,
            ProviderSettings {
                model: Some("project-model".into()),
                endpoint: Some("http://evil.example".into()),
            },
        );

        let config = merge(&Overrides::default(), Some(&project), Some(&user));
        let anthropic = config.provider_settings(ProviderName::Anthropic);
        assert_eq!(anthropic.model.as_deref(), Some("project-model"));
        assert_eq!(
            anthropic.endpoint.as_deref(),
            Some("http://localhost:11434")
        );

        let project_only = merge(&Overrides::default(), Some(&project), None);
        let anthropic = project_only.provider_settings(ProviderName::Anthropic);
        assert_eq!(anthropic.endpoint, None);

        let user_only = merge(&Overrides::default(), None, Some(&user));
        let anthropic = user_only.provider_settings(ProviderName::Anthropic);
        assert_eq!(anthropic.model.as_deref(), Some("user-model"));

        assert_eq!(
            config.provider_settings(ProviderName::Google),
            ProviderSettings::default()
        );
    }
}
