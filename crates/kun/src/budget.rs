//! `kun budget ...` and the Usage Budget guard around a Provider (S17).
//!
//! The pure rules (does this fit? which month?) and the usage file live in
//! `kun_core::budget`. This module connects them to the CLI:
//!
//! - [`BudgetedProvider`] wraps a Provider. Before each call it checks the
//!   budget, and after each successful call it counts the characters. That
//!   way every call counts, including the second call of a Language Pair.
//! - [`status`], [`set_limit_in`] and [`unset_limit_in`] are the `kun budget`
//!   commands.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

use kun_core::budget::{
    BudgetDecision, Month, UsageState, decide, load_usage, save_usage, usage_state_path,
    write_private_file,
};
use kun_core::config::{BudgetSettings, ProviderName, user_config_path};
use kun_core::provider::{Provider, ProviderError, Translation, TranslationRequest};
use toml_edit::{DocumentMut, Item, Value};

use crate::args::BudgetAction;
use crate::error::{CliError, ConfigFile, ConfigProblem};
use crate::json_output::{BudgetStatusOutput, BudgetStatusProvider, DryRunBudget, SCHEMA_VERSION};
use crate::providers::provider_label;

/// Where the two files live, from the environment. The values are passed in
/// (not read here) so the caller owns all process state.
pub struct Paths {
    /// The User Config (`config.toml`), where `[budget]` is written.
    pub user_config: Option<PathBuf>,
    /// The usage file (`usage.json`).
    pub usage: Option<PathBuf>,
}

impl Paths {
    /// Resolves both paths. `None` when there is neither HOME nor an XDG variable.
    pub fn new(
        home: Option<&Path>,
        xdg_config_home: Option<&Path>,
        xdg_state_home: Option<&Path>,
    ) -> Paths {
        Paths {
            user_config: user_config_path(home, xdg_config_home),
            usage: usage_state_path(home, xdg_state_home),
        }
    }
}

/// The characters in `text`, in the same unit as `max_chars`.
fn char_count(text: &str) -> u64 {
    u64::try_from(text.chars().count()).unwrap_or(u64::MAX)
}

// --- The guard around a Provider ------------------------------------------

/// A Provider that enforces the Usage Budget of one built-in Provider.
///
/// With no limit it passes every call straight through and never reads or
/// writes the usage file, so the default behaviour is unchanged.
pub struct BudgetedProvider {
    inner: Box<dyn Provider>,
    active: Option<Active>,
}

/// The budget of one run: only exists when a limit is set.
struct Active {
    provider: ProviderName,
    limit: u64,
    month: Month,
    path: PathBuf,
    state: RefCell<UsageState>,
    /// Set when a call was refused. Holds the numbers for the message.
    refused: Cell<BudgetDecision>,
    /// Set when a counted call could not be written to the usage file.
    save_failed: Cell<bool>,
}

impl BudgetedProvider {
    /// Wraps `inner`. With a `limit`, reads the usage file now: a missing file
    /// means 0 used, and an unreadable or invalid file fails closed.
    pub fn new(
        inner: Box<dyn Provider>,
        provider: ProviderName,
        limit: Option<u64>,
        usage_path: Option<&Path>,
        month: Month,
    ) -> Result<BudgetedProvider, CliError> {
        let Some(limit) = limit else {
            return Ok(BudgetedProvider {
                inner,
                active: None,
            });
        };
        let path = usage_path.ok_or(CliError::Unexpected(
            "cannot find a place for the usage file: set HOME or XDG_STATE_HOME",
        ))?;
        let state = match load_usage(path) {
            Ok(Some(state)) => state,
            Ok(None) => UsageState::new(month),
            Err(_) => return Err(CliError::UsageFileUnusable),
        };
        Ok(BudgetedProvider {
            inner,
            active: Some(Active {
                provider,
                limit,
                month,
                path: path.to_path_buf(),
                state: RefCell::new(state),
                refused: Cell::new(BudgetDecision::Allowed),
                save_failed: Cell::new(false),
            }),
        })
    }

    /// The error to report when a call was refused by the budget. A refused
    /// call comes back from the translation as a generic Provider error; this
    /// is the real reason.
    pub fn refusal(&self) -> Option<CliError> {
        let active = self.active.as_ref()?;
        match active.refused.get() {
            BudgetDecision::Allowed => None,
            BudgetDecision::Refused {
                used,
                needed,
                limit,
            } => Some(CliError::BudgetReached {
                provider: provider_label(active.provider),
                used,
                limit,
                needed,
                month: active.month,
            }),
        }
    }

    /// True when a successful call could not be added to the usage file.
    pub fn counter_not_saved(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| active.save_failed.get())
    }
}

impl Provider for BudgetedProvider {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn translate(&self, request: &TranslationRequest) -> Result<Translation, ProviderError> {
        let Some(active) = &self.active else {
            return self.inner.translate(request);
        };

        // Check before sending: a refusal makes no network call.
        let needed = char_count(&request.text);
        let used = active.state.borrow().used(active.provider, active.month);
        let decision = decide(used, needed, Some(active.limit));
        if decision != BudgetDecision::Allowed {
            active.refused.set(decision);
            // The caller replaces this with the budget error (`refusal`).
            return Err(ProviderError::QuotaExceeded);
        }

        // Count only after a successful response.
        let translation = self.inner.translate(request)?;
        let mut state = active.state.borrow_mut();
        state.add(active.provider, active.month, needed);
        if save_usage(&active.path, &state).is_err() {
            active.save_failed.set(true);
        }
        Ok(translation)
    }
}

// --- Dry run ---------------------------------------------------------------

/// The budget line of a dry run, or `None` when no limit is set for the
/// Provider. Reads the usage file (never writes it).
pub fn dry_run_budget(
    provider: ProviderName,
    budget: &BudgetSettings,
    usage_path: Option<&Path>,
    month: Month,
    needs: u64,
) -> Result<Option<DryRunBudget>, CliError> {
    let Some(limit) = budget.limit(provider) else {
        return Ok(None);
    };
    let used = used_this_month(provider, usage_path, month)?;
    let would_refuse = decide(used, needs, Some(limit)) != BudgetDecision::Allowed;
    Ok(Some(DryRunBudget {
        limit,
        used,
        needs,
        would_refuse,
    }))
}

/// Characters counted for `provider` this month. No file means 0; a file that
/// cannot be used is an error.
fn used_this_month(
    provider: ProviderName,
    usage_path: Option<&Path>,
    month: Month,
) -> Result<u64, CliError> {
    let Some(path) = usage_path else {
        return Ok(0);
    };
    match load_usage(path) {
        Ok(Some(state)) => Ok(state.used(provider, month)),
        Ok(None) => Ok(0),
        Err(_) => Err(CliError::UsageFileUnusable),
    }
}

// --- kun budget status -----------------------------------------------------

/// What `kun budget status` shows.
pub struct Status {
    month: Month,
    rows: Vec<StatusRow>,
}

struct StatusRow {
    provider: &'static str,
    limit: Option<u64>,
    used: u64,
}

impl StatusRow {
    fn remaining(&self) -> Option<u64> {
        self.limit.map(|limit| limit.saturating_sub(self.used))
    }
}

/// Reads the limits and the counters. Unlike a translation, `status` always
/// reads the usage file: that is what it was asked to show.
pub fn status(
    budget: &BudgetSettings,
    usage_path: Option<&Path>,
    month: Month,
) -> Result<Status, CliError> {
    let mut rows = Vec::new();
    for provider in ProviderName::ALL {
        rows.push(StatusRow {
            provider: provider_label(provider),
            limit: budget.limit(provider),
            used: used_this_month(provider, usage_path, month)?,
        });
    }
    Ok(Status { month, rows })
}

impl Status {
    /// The human report: one line per Provider.
    pub fn render_human(&self) -> String {
        let mut out = format!(
            "Usage budget for {} (resets on {}, UTC):\n",
            self.month,
            self.month.first_of_next()
        );
        for row in &self.rows {
            match (row.limit, row.remaining()) {
                (Some(limit), Some(left)) => out.push_str(&format!(
                    "{}: {} of {limit} characters used, {left} left\n",
                    row.provider, row.used
                )),
                _ => out.push_str(&format!(
                    "{}: no limit, {} characters used\n",
                    row.provider, row.used
                )),
            }
        }
        out
    }

    /// The `--json` object.
    pub fn to_json(&self) -> BudgetStatusOutput {
        let provider = |name: &str| {
            self.rows
                .iter()
                .find(|row| row.provider == name)
                .map(|row| BudgetStatusProvider {
                    limit: row.limit,
                    used: row.used,
                    remaining: row.remaining(),
                })
                .unwrap_or(BudgetStatusProvider {
                    limit: None,
                    used: 0,
                    remaining: None,
                })
        };
        BudgetStatusOutput {
            schema_version: SCHEMA_VERSION,
            month: self.month.to_string(),
            resets_on: self.month.first_of_next(),
            providers: crate::json_output::BudgetStatusProviders {
                google: provider("google"),
                anthropic: provider("anthropic"),
            },
        }
    }
}

// --- kun budget set | unset --------------------------------------------------

/// The `[budget]` key of a Provider.
fn key_name(provider: ProviderName) -> &'static str {
    match provider {
        ProviderName::Google => "google_chars_per_month",
        ProviderName::Anthropic => "anthropic_chars_per_month",
        // `ProviderName` is `#[non_exhaustive]`.
        _ => "unknown",
    }
}

fn user_config_unparsable() -> CliError {
    CliError::Config {
        file: ConfigFile::User,
        problem: ConfigProblem::Unparsable,
    }
}

/// `text` (the User Config) with `[budget] <provider>_chars_per_month = chars`.
/// Everything else, comments included, stays as it was.
pub fn set_limit_in(text: &str, provider: ProviderName, chars: u64) -> Result<String, CliError> {
    let amount = i64::try_from(chars).map_err(|_| CliError::Usage("the amount is too large"))?;
    let mut doc: DocumentMut = text.parse().map_err(|_| user_config_unparsable())?;

    let had_content = !doc.as_table().is_empty();
    let is_new_table = !doc.contains_key("budget");
    let item = doc.entry("budget").or_insert_with(toml_edit::table);
    if is_new_table && had_content {
        // A blank line between the new table and what came before.
        if let Some(table) = item.as_table_mut() {
            table.decor_mut().set_prefix("\n");
        }
    }
    let table = item
        .as_table_like_mut()
        .ok_or_else(user_config_unparsable)?;

    let key = key_name(provider);
    match table.get_mut(key).and_then(Item::as_value_mut) {
        Some(old) => {
            // Keep a comment that follows the old value.
            let decor = old.decor().clone();
            let mut new = Value::from(amount);
            *new.decor_mut() = decor;
            *old = new;
        }
        None => {
            table.insert(key, toml_edit::value(amount));
        }
    }
    Ok(doc.to_string())
}

/// `text` without the `[budget]` key of `provider`. `None` when it was not set,
/// so the caller leaves the file alone.
pub fn unset_limit_in(text: &str, provider: ProviderName) -> Result<Option<String>, CliError> {
    let mut doc: DocumentMut = text.parse().map_err(|_| user_config_unparsable())?;
    let Some(item) = doc.get_mut("budget") else {
        return Ok(None);
    };
    let table = item
        .as_table_like_mut()
        .ok_or_else(user_config_unparsable)?;
    if table.remove(key_name(provider)).is_none() {
        return Ok(None);
    }
    Ok(Some(doc.to_string()))
}

/// The User Config text, or an empty string when there is no file yet.
fn read_user_config(path: &Path) -> Result<String, CliError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(_) => Err(CliError::Config {
            file: ConfigFile::User,
            problem: ConfigProblem::Unreadable,
        }),
    }
}

fn write_user_config(path: &Path, text: &str) -> Result<(), CliError> {
    write_private_file(path, text.as_bytes())
        .map_err(|_| CliError::Unexpected("cannot write the User Config"))
}

fn no_user_config() -> CliError {
    CliError::Unexpected("cannot find the User Config: set HOME or XDG_CONFIG_HOME")
}

/// The message after `kun budget set`.
fn set_message(provider: ProviderName, chars: u64) -> String {
    format!(
        "Set the monthly usage budget for {} to {chars} characters.",
        provider_label(provider)
    )
}

/// Runs `kun budget [status | set | unset]`. `limits` is the `[budget]` table
/// of the User Config (read by the caller, for `status`).
pub fn run(
    action: Option<BudgetAction>,
    json: bool,
    limits: &BudgetSettings,
    paths: &Paths,
    month: Month,
) -> Result<(), CliError> {
    match action {
        None | Some(BudgetAction::Status) => {
            let report = status(limits, paths.usage.as_deref(), month)?;
            if json {
                crate::app::write_stdout(&report.to_json().render())
            } else {
                crate::app::write_stdout(report.render_human().trim_end())
            }
        }
        Some(BudgetAction::Set { provider, chars }) => {
            let path = paths.user_config.as_deref().ok_or_else(no_user_config)?;
            let text = set_limit_in(&read_user_config(path)?, provider, chars)?;
            write_user_config(path, &text)?;
            crate::app::write_stdout(&set_message(provider, chars))
        }
        Some(BudgetAction::Unset { provider }) => {
            let path = paths.user_config.as_deref().ok_or_else(no_user_config)?;
            let name = provider_label(provider);
            match unset_limit_in(&read_user_config(path)?, provider)? {
                Some(text) => {
                    write_user_config(path, &text)?;
                    crate::app::write_stdout(&format!(
                        "Removed the monthly usage budget for {name}."
                    ))
                }
                None => crate::app::write_stdout(&format!(
                    "No monthly usage budget was set for {name}."
                )),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;
    use kun_core::language::LanguageCode;
    use kun_core::provider::fake::FakeProvider;

    fn month() -> Month {
        Month::parse("2026-11").unwrap()
    }

    fn request(text: &str) -> TranslationRequest {
        TranslationRequest {
            text: text.to_string(),
            source: None,
            target: "th".parse::<LanguageCode>().unwrap(),
        }
    }

    /// Counts how many times the wrapped Provider is really called.
    struct Counting {
        calls: Rc<Cell<usize>>,
        inner: FakeProvider,
    }

    impl Provider for Counting {
        fn name(&self) -> &'static str {
            "fake"
        }

        fn translate(&self, request: &TranslationRequest) -> Result<Translation, ProviderError> {
            self.calls.set(self.calls.get() + 1);
            self.inner.translate(request)
        }
    }

    fn counting(inner: FakeProvider) -> (Box<dyn Provider>, Rc<Cell<usize>>) {
        let calls = Rc::new(Cell::new(0));
        let provider = Counting {
            calls: Rc::clone(&calls),
            inner,
        };
        (Box::new(provider), calls)
    }

    fn guarded(
        inner: Box<dyn Provider>,
        limit: Option<u64>,
        path: &Path,
    ) -> Result<BudgetedProvider, CliError> {
        BudgetedProvider::new(inner, ProviderName::Google, limit, Some(path), month())
    }

    #[test]
    fn a_successful_call_is_counted_and_saved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        let (inner, calls) = counting(FakeProvider::new());
        let provider = guarded(inner, Some(100), &path).unwrap();

        provider.translate(&request("hello")).unwrap();
        provider.translate(&request("สวัสดี")).unwrap();

        assert_eq!(calls.get(), 2);
        let state = load_usage(&path).unwrap().unwrap();
        assert_eq!(state.used(ProviderName::Google, month()), 11);
        assert_eq!(state.used(ProviderName::Anthropic, month()), 0);
        assert!(provider.refusal().is_none());
    }

    #[test]
    fn a_failed_call_is_not_counted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        let (inner, calls) = counting(FakeProvider::failing(ProviderError::Authentication));
        let provider = guarded(inner, Some(100), &path).unwrap();

        let error = provider.translate(&request("hello")).unwrap_err();

        assert_eq!(error, ProviderError::Authentication);
        assert_eq!(calls.get(), 1);
        assert!(!path.exists(), "nothing succeeded, so nothing is saved");
        assert!(provider.refusal().is_none());
    }

    #[test]
    fn a_call_over_the_limit_never_reaches_the_provider() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        let (inner, calls) = counting(FakeProvider::new());
        let provider = guarded(inner, Some(7), &path).unwrap();

        provider.translate(&request("hello")).unwrap();
        provider.translate(&request("hello")).unwrap_err();

        assert_eq!(calls.get(), 1, "the refused call must not be sent");
        let refusal = provider.refusal().unwrap();
        assert_eq!(refusal.exit_code(), 8);
        assert!(refusal.to_string().contains("5 of 7 characters used"));
        // The first call stays counted.
        let state = load_usage(&path).unwrap().unwrap();
        assert_eq!(state.used(ProviderName::Google, month()), 5);
    }

    #[test]
    fn without_a_limit_the_usage_file_is_never_read_or_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        std::fs::write(&path, "{ corrupt").unwrap();
        let (inner, _) = counting(FakeProvider::new());
        let provider = guarded(inner, None, &path).unwrap();

        provider.translate(&request("hello")).unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ corrupt");
        assert!(!provider.counter_not_saved());
    }

    #[test]
    fn a_corrupt_usage_file_with_a_limit_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        std::fs::write(&path, "{ corrupt").unwrap();
        let (inner, _) = counting(FakeProvider::new());

        let error = guarded(inner, Some(100), &path).err().unwrap();

        assert_eq!(error.exit_code(), 3);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ corrupt");
    }

    #[test]
    fn a_state_from_another_month_counts_as_zero() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        let old = Month::parse("2026-10").unwrap();
        let mut state = UsageState::new(old);
        state.add(ProviderName::Google, old, 1_000_000);
        save_usage(&path, &state).unwrap();
        let (inner, _) = counting(FakeProvider::new());
        let provider = guarded(inner, Some(10), &path).unwrap();

        provider.translate(&request("hello")).unwrap();

        let state = load_usage(&path).unwrap().unwrap();
        assert_eq!(state.used(ProviderName::Google, month()), 5);
    }

    #[cfg(unix)]
    #[test]
    fn a_counter_that_cannot_be_saved_is_flagged_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        // A dangling symlink where the directory should be: the file is
        // simply missing (0 used), but the directory cannot be created.
        let link = dir.path().join("state");
        std::os::unix::fs::symlink(dir.path().join("nowhere"), &link).unwrap();
        let path = link.join("usage.json");
        let (inner, _) = counting(FakeProvider::new());
        let provider = guarded(inner, Some(100), &path).unwrap();

        assert!(provider.translate(&request("hello")).is_ok());
        assert!(provider.counter_not_saved());
    }

    #[test]
    fn a_limit_without_a_usage_path_is_an_error() {
        let (inner, _) = counting(FakeProvider::new());
        let error =
            BudgetedProvider::new(inner, ProviderName::Google, Some(5), None, month()).err();
        assert_eq!(error.unwrap().exit_code(), 1);
    }

    #[test]
    fn the_dry_run_budget_reads_but_never_writes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        let budget = BudgetSettings {
            google_chars_per_month: Some(10),
            anthropic_chars_per_month: None,
        };

        let none = dry_run_budget(ProviderName::Anthropic, &budget, Some(&path), month(), 5);
        assert!(none.unwrap().is_none());

        let report = dry_run_budget(ProviderName::Google, &budget, Some(&path), month(), 11)
            .unwrap()
            .unwrap();
        assert_eq!((report.limit, report.used, report.needs), (10, 0, 11));
        assert!(report.would_refuse);

        let report = dry_run_budget(ProviderName::Google, &budget, Some(&path), month(), 10)
            .unwrap()
            .unwrap();
        assert!(!report.would_refuse);
        assert!(!path.exists());
    }

    // --- status ---------------------------------------------------------

    #[test]
    fn status_lists_every_provider_with_what_is_left() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        let mut state = UsageState::new(month());
        state.add(ProviderName::Google, month(), 448_210);
        save_usage(&path, &state).unwrap();
        let budget = BudgetSettings {
            google_chars_per_month: Some(450_000),
            anthropic_chars_per_month: None,
        };

        let report = status(&budget, Some(&path), month()).unwrap();

        assert_eq!(
            report.render_human(),
            "Usage budget for 2026-11 (resets on 2026-12-01, UTC):\n\
             google: 448210 of 450000 characters used, 1790 left\n\
             anthropic: no limit, 0 characters used\n"
        );
        assert_eq!(
            report.to_json().render(),
            r#"{"schema_version":1,"month":"2026-11","resets_on":"2026-12-01","providers":{"google":{"limit":450000,"used":448210,"remaining":1790},"anthropic":{"limit":null,"used":0,"remaining":null}}}"#
        );
    }

    #[test]
    fn nothing_is_left_when_the_limit_was_lowered_below_the_usage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        let mut state = UsageState::new(month());
        state.add(ProviderName::Google, month(), 50);
        save_usage(&path, &state).unwrap();
        let budget = BudgetSettings {
            google_chars_per_month: Some(10),
            anthropic_chars_per_month: None,
        };

        let report = status(&budget, Some(&path), month()).unwrap();

        assert!(
            report
                .render_human()
                .contains("50 of 10 characters used, 0 left")
        );
    }

    #[test]
    fn status_fails_closed_on_a_corrupt_usage_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        std::fs::write(&path, "nope").unwrap();
        let error = status(&BudgetSettings::default(), Some(&path), month())
            .err()
            .unwrap();
        assert_eq!(error.exit_code(), 3);
    }

    // --- set / unset ------------------------------------------------------

    #[test]
    fn set_on_an_empty_file_adds_the_table() {
        let text = set_limit_in("", ProviderName::Google, 450_000).unwrap();
        assert_eq!(text, "[budget]\ngoogle_chars_per_month = 450000\n");
    }

    #[test]
    fn set_keeps_comments_layout_and_other_tables() {
        let original =
            "# keep me\ntarget = \"th\" # and me\n\n[secret_guard]\nforce_redact = true\n";
        let text = set_limit_in(original, ProviderName::Anthropic, 50_000).unwrap();
        assert!(text.starts_with(original), "{text}");
        assert!(
            text.contains("[budget]\nanthropic_chars_per_month = 50000"),
            "{text}"
        );
    }

    #[test]
    fn set_changes_an_existing_value_and_keeps_its_comment() {
        let original =
            "[budget]\ngoogle_chars_per_month = 10 # my free tier\nanthropic_chars_per_month = 5\n";
        let text = set_limit_in(original, ProviderName::Google, 99).unwrap();
        assert_eq!(
            text,
            "[budget]\ngoogle_chars_per_month = 99 # my free tier\nanthropic_chars_per_month = 5\n"
        );
    }

    #[test]
    fn set_works_with_an_inline_budget_table() {
        let text = set_limit_in(
            "budget = { anthropic_chars_per_month = 5 }\n",
            ProviderName::Google,
            7,
        )
        .unwrap();
        assert!(text.contains("google_chars_per_month = 7"), "{text}");
        assert!(text.contains("anthropic_chars_per_month = 5"), "{text}");
    }

    #[test]
    fn set_refuses_what_it_cannot_edit_safely() {
        for text in ["not = = toml", "budget = 5\n"] {
            let error = set_limit_in(text, ProviderName::Google, 1).unwrap_err();
            assert_eq!(error.exit_code(), 3, "{text}");
        }
        let error = set_limit_in("", ProviderName::Google, u64::MAX).unwrap_err();
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn unset_removes_only_that_key() {
        let original =
            "# top\n[budget]\ngoogle_chars_per_month = 10\nanthropic_chars_per_month = 5\n";
        let text = unset_limit_in(original, ProviderName::Google)
            .unwrap()
            .unwrap();
        assert_eq!(text, "# top\n[budget]\nanthropic_chars_per_month = 5\n");
    }

    #[test]
    fn unset_of_a_missing_key_changes_nothing() {
        for text in [
            "",
            "target = \"th\"\n",
            "[budget]\nanthropic_chars_per_month = 5\n",
        ] {
            assert_eq!(
                unset_limit_in(text, ProviderName::Google).unwrap(),
                None,
                "{text}"
            );
        }
    }
}
