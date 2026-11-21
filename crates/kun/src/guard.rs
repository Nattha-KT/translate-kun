//! The Secret Guard flow of the CLI Adapter (requirements §7).
//!
//! `kun-core` finds and decides. This module acts on the decision: it asks
//! the user, redacts, or blocks. Findings hold only kinds and positions, so
//! nothing here can print a secret.

use std::io::{BufRead, BufReader, Write};

use kun_core::config::SecretGuardSettings;
use kun_core::secret_guard::{
    Decision, Finding, GuardPolicy, SecretGuard, SecretKind, decide, redact,
};

use crate::error::CliError;

/// The text to send, after the Secret Guard has had its say.
/// No `Debug`: it is user text.
pub struct Guarded {
    pub text: String,
    /// The text was changed by redaction.
    pub redacted: bool,
}

/// Runs the Secret Guard on `text`.
///
/// `ask` is called only for `Decision::AskUser`, with the findings, and says
/// whether the user agreed to send. It is a parameter so tests need no terminal.
pub fn run_secret_guard(
    text: String,
    settings: &SecretGuardSettings,
    interactive: bool,
    ask: &dyn Fn(&[Finding]) -> bool,
) -> Result<Guarded, CliError> {
    let guard = SecretGuard::new(&settings.extra_patterns).map_err(CliError::ExtraPattern)?;
    let findings = guard.scan(&text);
    let policy = GuardPolicy {
        allow_secrets: settings.allow_secrets,
        force_redact: settings.force_redact,
    };

    match decide(&findings, policy, interactive) {
        Decision::Send => Ok(Guarded {
            text,
            redacted: false,
        }),
        Decision::Redact => Ok(Guarded {
            text: redact(&text, &findings),
            redacted: true,
        }),
        Decision::AskUser => {
            if ask(&findings) {
                Ok(Guarded {
                    text,
                    redacted: false,
                })
            } else {
                Err(CliError::SecretDeclined)
            }
        }
        Decision::Block => Err(CliError::SecretBlocked(findings)),
        // `Decision` is `#[non_exhaustive]`. An unknown decision must not send the text.
        _ => Err(CliError::SecretBlocked(findings)),
    }
}

/// The kinds found, each once, e.g. `AWS access key, JWT`.
pub fn kinds_summary(findings: &[Finding]) -> String {
    let mut kinds: Vec<SecretKind> = Vec::new();
    for finding in findings {
        if !kinds.contains(&finding.kind) {
            kinds.push(finding.kind);
        }
    }
    let names: Vec<String> = kinds.iter().map(ToString::to_string).collect();
    names.join(", ")
}

/// Only `y` or `yes` (any case) means yes. Everything else means no.
pub fn is_yes(answer: &str) -> bool {
    let answer = answer.trim().to_ascii_lowercase();
    answer == "y" || answer == "yes"
}

/// Asks on stderr and reads the answer from the terminal (`/dev/tty`), so the
/// question works even when stdin is redirected. If the terminal cannot be
/// opened or read, the answer is no.
pub fn ask_on_terminal(findings: &[Finding]) -> bool {
    ask(findings, &mut std::io::stderr(), &|| {
        let tty = std::fs::File::open("/dev/tty")?;
        Ok(Box::new(BufReader::new(tty)))
    })
}

/// Opens the place the answer is read from (the terminal).
type OpenAnswerSource<'a> = &'a dyn Fn() -> std::io::Result<Box<dyn BufRead>>;

/// The prompt, with its output and its input passed in so tests need no terminal.
///
/// If the question cannot be shown (the write or the flush fails), nobody
/// can have answered it: the result is no, and the answer is not even read.
fn ask(
    findings: &[Finding],
    prompt_out: &mut dyn Write,
    open_answer_source: OpenAnswerSource,
) -> bool {
    let shown = write!(
        prompt_out,
        "Possible secret detected ({}). Send anyway? [y/N] ",
        kinds_summary(findings)
    )
    .and_then(|()| prompt_out.flush());
    if shown.is_err() {
        return false;
    }

    let Ok(mut source) = open_answer_source() else {
        return false;
    };
    let mut answer = String::new();
    if source.read_line(&mut answer).is_err() {
        return false;
    }
    is_yes(&answer)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    /// Obviously fake, but shaped like an AWS access key.
    const KEY: &str = "AKIAFAKEEXAMPLE12345";

    fn text() -> String {
        format!("my key is {KEY} ok")
    }

    fn settings(allow_secrets: bool, force_redact: bool) -> SecretGuardSettings {
        SecretGuardSettings {
            allow_secrets,
            force_redact,
            ..SecretGuardSettings::default()
        }
    }

    fn never_asked(_: &[Finding]) -> bool {
        panic!("the user must not be asked here");
    }

    #[test]
    fn clean_text_is_sent_as_it_is() {
        let guarded = run_secret_guard(
            "hello".to_string(),
            &settings(false, false),
            false,
            &never_asked,
        )
        .unwrap();
        assert_eq!(guarded.text, "hello");
        assert!(!guarded.redacted);
    }

    #[test]
    fn piped_secret_is_blocked_with_positions_only() {
        let error = run_secret_guard(text(), &settings(false, false), false, &never_asked)
            .err()
            .unwrap();
        assert_eq!(error.exit_code(), 6);
        assert!(!error.to_string().contains(KEY));
        assert!(error.to_string().contains("bytes 10..30"));
    }

    #[test]
    fn redact_replaces_the_secret() {
        let guarded =
            run_secret_guard(text(), &settings(false, true), false, &never_asked).unwrap();
        assert_eq!(guarded.text, "my key is [REDACTED] ok");
        assert!(guarded.redacted);
    }

    #[test]
    fn allow_secrets_sends_the_text_unchanged() {
        let guarded =
            run_secret_guard(text(), &settings(true, false), false, &never_asked).unwrap();
        assert_eq!(guarded.text, text());
        assert!(!guarded.redacted);
    }

    #[test]
    fn interactive_yes_sends_and_no_blocks() {
        let asked = Cell::new(false);
        let yes = |findings: &[Finding]| {
            asked.set(true);
            assert_eq!(kinds_summary(findings), "AWS access key");
            true
        };
        let guarded = run_secret_guard(text(), &settings(false, false), true, &yes).unwrap();
        assert!(asked.get());
        assert_eq!(guarded.text, text());

        let error = run_secret_guard(text(), &settings(false, false), true, &|_| false)
            .err()
            .unwrap();
        assert_eq!(error.exit_code(), 6);
    }

    #[test]
    fn invalid_extra_pattern_is_a_config_error() {
        let bad = SecretGuardSettings {
            extra_patterns: vec!["(unclosed".to_string()],
            ..SecretGuardSettings::default()
        };
        let error = run_secret_guard("hello".to_string(), &bad, false, &never_asked)
            .err()
            .unwrap();
        assert_eq!(error.exit_code(), 3);
        assert!(!error.to_string().contains("unclosed"));
    }

    fn findings() -> Vec<Finding> {
        SecretGuard::new(&[]).unwrap().scan(KEY)
    }

    /// A reader over fixed bytes, as the "terminal".
    fn answers(text: &'static str) -> impl Fn() -> std::io::Result<Box<dyn BufRead>> {
        move || Ok(Box::new(std::io::Cursor::new(text.as_bytes())))
    }

    /// Fails the test if the answer source is opened.
    fn must_not_be_opened() -> std::io::Result<Box<dyn BufRead>> {
        panic!("the terminal must not be read when the prompt was not shown");
    }

    struct FailingWrite;

    impl Write for FailingWrite {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("closed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    struct FailingFlush;

    impl Write for FailingFlush {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("closed"))
        }
    }

    #[test]
    fn prompt_shows_the_kinds_and_reads_the_answer() {
        let mut shown = Vec::new();
        assert!(ask(&findings(), &mut shown, &answers("y\n")));
        let shown = String::from_utf8(shown).unwrap();
        assert_eq!(
            shown,
            "Possible secret detected (AWS access key). Send anyway? [y/N] "
        );
        assert!(!shown.contains(KEY));

        assert!(!ask(&findings(), &mut Vec::new(), &answers("n\n")));
        assert!(!ask(&findings(), &mut Vec::new(), &answers("")));
    }

    #[test]
    fn a_failed_prompt_write_means_no_and_reads_nothing() {
        assert!(!ask(&findings(), &mut FailingWrite, &must_not_be_opened));
    }

    #[test]
    fn a_failed_prompt_flush_means_no_and_reads_nothing() {
        assert!(!ask(&findings(), &mut FailingFlush, &must_not_be_opened));
    }

    #[test]
    fn an_unavailable_terminal_means_no() {
        let unavailable =
            || -> std::io::Result<Box<dyn BufRead>> { Err(std::io::Error::other("no tty")) };
        assert!(!ask(&findings(), &mut Vec::new(), &unavailable));
    }

    #[test]
    fn only_y_or_yes_means_yes() {
        for answer in ["y", "Y", "yes", "YES", " yes \n", "y\n"] {
            assert!(is_yes(answer), "{answer:?}");
        }
        for answer in ["", "\n", "n", "no", "yep", "ok", "yy", "true"] {
            assert!(!is_yes(answer), "{answer:?}");
        }
    }

    #[test]
    fn kinds_summary_lists_each_kind_once() {
        let findings = SecretGuard::new(&[])
            .unwrap()
            .scan(&format!("{KEY} and AKIAFAKEEXAMPLE67890"));
        assert_eq!(findings.len(), 2);
        assert_eq!(kinds_summary(&findings), "AWS access key");
    }
}
