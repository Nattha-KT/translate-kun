//! The CLI Adapter's main flow: arguments, config, input, Secret Guard,
//! Provider, output. Terminal I/O and exit codes live in the `kun` crate only.

use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

use clap::CommandFactory;
use kun_core::config::Overrides;
use kun_core::language::LanguageCode;
use kun_core::translate::{
    TargetPlan, TranslateError, Translated, resolve_target, translate_with_terms,
};

use crate::args::{AuthAction, Cli, Command, LanguageSpec, split_positionals};
use crate::auth::{self, open_store};
use crate::error::CliError;
use crate::guard::{ask_on_terminal, run_secret_guard};
use crate::init::init_in;
use crate::input::read_input;
use crate::json_output::{JsonOutput, SCHEMA_VERSION};
use crate::providers::{CredentialSources, build_provider, parse_provider_flag, provider_label};
use crate::settings::{Loaded, load_settings, load_user_credentials};

/// Runs one translation. Returns the error to report; `main` turns it into
/// a message and an exit code.
pub fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Some(Command::Init { force }) => {
            let dir = std::env::current_dir()
                .map_err(|_| CliError::Unexpected("cannot find the current directory"))?;
            init_in(&dir, force)?;
            println!("Created .kun.toml");
            return Ok(());
        }
        Some(Command::Completions { shell }) => {
            // Build the script in memory first: `generate` panics if stdout is
            // a closed pipe, while `write_stdout` treats that as success.
            let mut script: Vec<u8> = Vec::new();
            clap_complete::generate(shell, &mut Cli::command(), "kun", &mut script);
            let script = String::from_utf8(script)
                .map_err(|_| CliError::Unexpected("cannot build the completion script"))?;
            return write_stdout(&script);
        }
        Some(Command::Auth { action }) => {
            // Only `status` needs the User Config (for its `[credentials]`),
            // and only that file: a broken Project Config must not matter.
            let user = match action {
                AuthAction::Status => {
                    let home = std::env::var_os("HOME").map(PathBuf::from);
                    let xdg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
                    let user = load_user_credentials(home.as_deref(), xdg.as_deref())?;
                    for warning in &user.warnings {
                        eprintln!("warning: {warning}");
                    }
                    Some(user)
                }
                _ => None,
            };
            return auth::run(action, user.as_ref());
        }
        None => {}
    }

    // 1. Arguments.
    let positionals = split_positionals(&cli.words);
    let (source, explicit_target) = combine_languages(&cli, positionals.spec)?;
    let provider_override = match &cli.provider {
        Some(name) => parse_provider_flag(name)?,
        None => None,
    };

    // 2. Config: Project Config from the current directory, User Config from
    //    HOME / XDG_CONFIG_HOME, then the CLI overrides on top.
    //    The explicit Target Language is not an override: it is passed to
    //    `resolve_target` separately, so it can win over a Language Pair.
    let overrides = Overrides {
        target: None,
        provider: provider_override,
        max_chars: cli.max_chars,
        timeout_secs: None,
        force_redact: cli.redact,
        allow_secrets: cli.allow_secrets,
    };
    let loaded = load_settings_from_env(&overrides)?;
    // Warnings go to stderr at once, or into the JSON object with `--json`.
    let mut warnings = Warnings::new(cli.json);
    for warning in &loaded.warnings {
        warnings.add(warning);
    }
    let user_credentials = loaded.user_credentials;
    let config = loaded.config;

    // 3. Input, and the size limit (characters, not bytes).
    let input = read_input(positionals.text, config.max_chars)?;
    if input.ignored_stdin {
        warnings.add(
            "text was given as arguments and stdin is not a terminal; \
             ignoring stdin and using the arguments",
        );
    }
    // The limit comes first: whitespace-only text that is too long is "too
    // large" (7), not "no text" (2).
    if input.text.chars().count() > config.max_chars {
        return Err(CliError::TooLarge {
            limit: config.max_chars,
        });
    }
    if input.text.trim().is_empty() {
        return Err(CliError::NoText);
    }

    // 4. Target Language. Without one this is a usage error, reported before
    //    the Secret Guard runs or a Provider is built.
    let plan = resolve_target(
        explicit_target.as_ref(),
        config.pair.as_ref(),
        config.target.as_ref(),
    );
    if plan == TargetPlan::Missing {
        return Err(TranslateError::NoTarget.into());
    }

    // 5. Secret Guard. Interactive means a person can answer the prompt:
    //    piped stdin is never interactive.
    let interactive = io::stdin().is_terminal() && io::stderr().is_terminal();
    let guarded = run_secret_guard(
        input.text,
        &config.secret_guard,
        interactive,
        &ask_on_terminal,
    )?;
    if guarded.redacted {
        warnings.add("possible secrets were replaced with [REDACTED] before sending");
    }

    // 6. Provider and translation.
    let provider_name = cli
        .provider
        .as_deref()
        .unwrap_or_else(|| provider_label(config.provider));
    let store = open_store();
    let env = |name: &str| std::env::var(name).ok();
    let sources = CredentialSources {
        env: &env,
        store: &*store,
        user_file: user_credentials.as_ref(),
    };
    let provider = build_provider(provider_name, &config, &sources)?;
    let translated = translate_with_terms(
        &*provider,
        &guarded.text,
        source.clone(),
        plan.clone(),
        &config.protected_terms,
    )?;

    // 7. Output: the translation on stdout, everything else on stderr
    //    (or, with `--json`, one JSON object on stdout).
    add_translation_warnings(&mut warnings, &translated);
    let translation = translated.translation;
    if cli.json {
        let source_used = translation.detected_source.clone().or(source);
        let target = final_target(&plan, source_used.as_ref());
        let output = JsonOutput {
            schema_version: SCHEMA_VERSION,
            text: translation.text,
            source: source_used.map(|code| code.to_string()),
            target,
            provider: translation.provider.to_string(),
            warnings: warnings.into_list(),
        };
        return write_stdout(&output.render());
    }
    match &translation.detected_source {
        Some(code) => eprintln!(
            "provider: {}, detected source: {code}",
            translation.provider
        ),
        None => eprintln!("provider: {}", translation.provider),
    }
    write_stdout(&translation.text)
}

/// Loads the config files using HOME, XDG_CONFIG_HOME and the current directory.
fn load_settings_from_env(overrides: &Overrides) -> Result<Loaded, CliError> {
    let start_dir: PathBuf = std::env::current_dir()
        .map_err(|_| CliError::Unexpected("cannot find the current directory"))?;
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let xdg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
    load_settings(&start_dir, home.as_deref(), xdg.as_deref(), overrides)
}

/// Warnings: printed on stderr as they happen, or (with `--json`) collected
/// for the JSON object.
struct Warnings {
    json: bool,
    list: Vec<String>,
}

impl Warnings {
    fn new(json: bool) -> Self {
        Warnings {
            json,
            list: Vec::new(),
        }
    }

    fn add(&mut self, message: &str) {
        if self.json {
            self.list.push(message.to_string());
        } else {
            eprintln!("warning: {message}");
        }
    }

    fn into_list(self) -> Vec<String> {
        self.list
    }
}

/// Warnings that come from the translation itself. The translation still
/// succeeds; the text of a lost term is never put in the message.
fn add_translation_warnings(warnings: &mut Warnings, translated: &Translated) {
    if translated.same_language {
        warnings.add("the text is already in the Target Language; returned unchanged");
    }
    if translated.protected_terms_missing {
        warnings.add(
            "the Provider dropped a Protected Term, so it could not be restored in the translation",
        );
    }
}

/// The Target Language that was used. For a Language Pair this is the
/// language the text was not in (the Provider chose, so it is derived again).
fn final_target(plan: &TargetPlan, source: Option<&LanguageCode>) -> String {
    match plan {
        TargetPlan::Fixed(code) => code.to_string(),
        TargetPlan::Pair(pair) => {
            if source == Some(pair.first()) {
                pair.second().to_string()
            } else {
                pair.first().to_string()
            }
        }
        TargetPlan::Missing => String::new(),
    }
}

/// Merges the language spec (`:xx`, `xx:yy`) with `-s` / `-t`. Giving the
/// same language both ways is a usage error, so nothing is silently ignored.
fn combine_languages(
    cli: &Cli,
    spec: Option<LanguageSpec>,
) -> Result<(Option<LanguageCode>, Option<LanguageCode>), CliError> {
    let Some(spec) = spec else {
        return Ok((cli.source.clone(), cli.target.clone()));
    };
    if cli.target.is_some() || (spec.source.is_some() && cli.source.is_some()) {
        return Err(CliError::Usage(
            "give languages either as `xx:yy` or with -s/-t, not both",
        ));
    }
    Ok((
        spec.source.or_else(|| cli.source.clone()),
        Some(spec.target),
    ))
}

/// Writes the text and a final newline (unless it already ends with one).
/// A closed pipe (`kun ... | head`) is not an error.
fn write_stdout(text: &str) -> Result<(), CliError> {
    let mut out = io::stdout().lock();
    let mut result = out.write_all(text.as_bytes());
    if result.is_ok() && !text.ends_with('\n') {
        result = out.write_all(b"\n");
    }
    let result = result.and_then(|()| out.flush());
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(_) => Err(CliError::Unexpected("cannot write to standard output")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kun_core::provider::Translation;

    fn translated(same_language: bool, protected_terms_missing: bool) -> Translated {
        Translated {
            translation: Translation {
                text: "FAKE text".to_string(),
                detected_source: None,
                provider: "fake",
            },
            same_language,
            protected_terms_missing,
        }
    }

    #[test]
    fn a_lost_protected_term_becomes_a_warning_without_the_term() {
        // `--json` mode collects the warnings, so they can be inspected.
        let mut warnings = Warnings::new(true);
        add_translation_warnings(&mut warnings, &translated(false, true));
        let list = warnings.into_list();
        assert_eq!(list.len(), 1);
        assert!(list[0].contains("Protected Term"));
    }

    #[test]
    fn a_clean_translation_has_no_warnings() {
        let mut warnings = Warnings::new(true);
        add_translation_warnings(&mut warnings, &translated(false, false));
        assert!(warnings.into_list().is_empty());
    }

    #[test]
    fn same_language_still_warns() {
        let mut warnings = Warnings::new(true);
        add_translation_warnings(&mut warnings, &translated(true, false));
        assert_eq!(warnings.into_list().len(), 1);
    }
}
