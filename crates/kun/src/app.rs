//! The CLI Adapter's main flow: arguments, config, input, Secret Guard,
//! Provider, output. Terminal I/O and exit codes live in the `kun` crate only.

use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

use kun_core::config::Overrides;
use kun_core::language::LanguageCode;
use kun_core::translate::{TargetPlan, TranslateError, resolve_target, translate};

use crate::args::{Cli, LanguageSpec, split_positionals};
use crate::error::CliError;
use crate::guard::{ask_on_terminal, run_secret_guard};
use crate::input::read_input;
use crate::providers::{build_provider, parse_provider_flag, provider_label};
use crate::settings::load_settings;

/// Runs one translation. Returns the error to report; `main` turns it into
/// a message and an exit code.
pub fn run(cli: Cli) -> Result<(), CliError> {
    if cli.json {
        return Err(CliError::Usage("--json is not implemented yet"));
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
    let start_dir: PathBuf = std::env::current_dir()
        .map_err(|_| CliError::Unexpected("cannot find the current directory"))?;
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let xdg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
    let loaded = load_settings(&start_dir, home.as_deref(), xdg.as_deref(), &overrides)?;
    for warning in &loaded.warnings {
        eprintln!("warning: {warning}");
    }
    let config = loaded.config;

    // 3. Input, and the size limit (characters, not bytes).
    let input = read_input(positionals.text, config.max_chars)?;
    if input.ignored_stdin {
        eprintln!(
            "warning: text was given as arguments and stdin is not a terminal; \
             ignoring stdin and using the arguments"
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
        eprintln!("note: possible secrets were replaced with [REDACTED] before sending");
    }

    // 6. Provider and translation.
    let provider_name = cli
        .provider
        .as_deref()
        .unwrap_or_else(|| provider_label(config.provider));
    let provider = build_provider(provider_name, &config)?;
    let translated = translate(&*provider, &guarded.text, source, plan)?;

    // 7. Output: the translation on stdout, everything else on stderr.
    let translation = translated.translation;
    match &translation.detected_source {
        Some(code) => eprintln!(
            "provider: {}, detected source: {code}",
            translation.provider
        ),
        None => eprintln!("provider: {}", translation.provider),
    }
    if translated.same_language {
        eprintln!("warning: the text is already in the Target Language; returned unchanged");
    }
    write_stdout(&translation.text)
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
