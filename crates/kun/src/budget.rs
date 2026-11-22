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

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use kun_core::budget::{
    BudgetDecision, Month, UsageError, UsageState, decide, load_usage, save_usage,
    usage_state_path, write_private_file,
};
use kun_core::config::{BudgetSettings, ProviderName, user_config_path};
use kun_core::provider::{Provider, ProviderError, Translation, TranslationRequest};
use toml_edit::{DocumentMut, Item, RawString, Value};

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
///
/// With a limit, every call **reserves before it sends**: it reads the usage
/// file, checks that the call fits, and writes the new total (a real atomic
/// write) *before* the request is made. If that write fails for any reason,
/// nothing is sent, so a counter that cannot be saved can never let the budget
/// be passed. If the call then fails, the reservation is given back. A run
/// that makes no Provider call never touches the file.
pub struct BudgetedProvider {
    inner: Box<dyn Provider>,
    active: Option<Active>,
}

/// The budget of one run: only exists when a limit is set.
struct Active {
    provider: ProviderName,
    limit: u64,
    month: Month,
    /// `None` when there is neither HOME nor XDG_STATE_HOME.
    path: Option<PathBuf>,
    /// Everything that changes during the run, in one place.
    run: RefCell<Run>,
}

/// What changes during a run.
struct Run {
    /// Why a call was stopped before it was sent.
    stopped: Option<Stopped>,
    /// A failed call's reservation could not be given back (the count stays
    /// too high, which is the safe direction).
    refund_failed: bool,
}

/// Why a call was stopped before it was sent.
#[derive(Clone, Copy)]
enum Stopped {
    /// The call would pass the limit. Holds the numbers for the message.
    OverLimit { used: u64, needed: u64, limit: u64 },
    /// There is no place for the usage file.
    NoUsagePath,
    /// The usage file cannot be read or is not valid.
    Unreadable,
    /// The usage file is a symlink, a directory or another non-regular file.
    NotRegular,
    /// The new total could not be written, so nothing was sent.
    NotSaveable,
}

/// A call's reservation: the counters as they were before it.
struct Reservation {
    before: UsageState,
}

impl BudgetedProvider {
    /// Wraps `inner`. Nothing is read or written yet.
    pub fn new(
        inner: Box<dyn Provider>,
        provider: ProviderName,
        limit: Option<u64>,
        usage_path: Option<&Path>,
        month: Month,
    ) -> BudgetedProvider {
        let active = limit.map(|limit| Active {
            provider,
            limit,
            month,
            path: usage_path.map(Path::to_path_buf),
            run: RefCell::new(Run {
                stopped: None,
                refund_failed: false,
            }),
        });
        BudgetedProvider { inner, active }
    }

    /// The error to report when the budget stopped a call before it was sent.
    /// The stopped call comes back from the translation as a generic Provider
    /// error; this is the real reason.
    pub fn failure(&self) -> Option<CliError> {
        let active = self.active.as_ref()?;
        let stopped = active.run.borrow().stopped?;
        Some(match stopped {
            Stopped::OverLimit {
                used,
                needed,
                limit,
            } => CliError::BudgetReached {
                provider: provider_label(active.provider),
                used,
                limit,
                needed,
                month: active.month,
            },
            Stopped::NoUsagePath => CliError::Unexpected(
                "cannot find a place for the usage file: set HOME or XDG_STATE_HOME",
            ),
            Stopped::Unreadable => CliError::UsageFileUnusable,
            Stopped::NotRegular => CliError::UsageFileNotRegular,
            Stopped::NotSaveable => CliError::UsageFileNotSaveable,
        })
    }

    /// True when a failed call's reservation could not be given back, so the
    /// counter is too high. The caller prints a short warning.
    pub fn refund_failed(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| active.run.borrow().refund_failed)
    }
}

impl Active {
    /// Before a call: reads the usage file, checks that the call fits, and
    /// writes the new total. The write is the real operation, so if it fails
    /// the call is stopped.
    fn reserve(&self, request: &TranslationRequest) -> Result<Reservation, Stopped> {
        let Some(path) = &self.path else {
            return Err(Stopped::NoUsagePath);
        };
        let before = match load_usage(path) {
            Ok(Some(state)) => state,
            Ok(None) => UsageState::new(self.month),
            Err(UsageError::NotRegular) => return Err(Stopped::NotRegular),
            Err(_) => return Err(Stopped::Unreadable),
        };

        let used = before.used(self.provider, self.month);
        let needed = char_count(&request.text);
        if let BudgetDecision::Refused {
            used,
            needed,
            limit,
        } = decide(used, needed, Some(self.limit))
        {
            return Err(Stopped::OverLimit {
                used,
                needed,
                limit,
            });
        }

        let mut after = before.clone();
        after.add(self.provider, self.month, needed);
        save_usage(path, &after).map_err(|_| Stopped::NotSaveable)?;
        Ok(Reservation { before })
    }

    /// After a failed call: writes the counters as they were. If that fails
    /// the count stays too high, which is the safe direction for a budget.
    fn refund(&self, reservation: &Reservation) {
        let Some(path) = &self.path else {
            return;
        };
        if save_usage(path, &reservation.before).is_err() {
            self.run.borrow_mut().refund_failed = true;
        }
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

        // Reserve before sending: a stopped call makes no network call.
        let reservation = match active.reserve(request) {
            Ok(reservation) => reservation,
            Err(stopped) => {
                active.run.borrow_mut().stopped = Some(stopped);
                // The caller replaces this with the real reason (`failure`).
                return Err(ProviderError::QuotaExceeded);
            }
        };

        // On success the reservation is the count. On failure, give it back.
        let result = self.inner.translate(request);
        if result.is_err() {
            active.refund(&reservation);
        }
        result
    }
}

// --- Dry run ---------------------------------------------------------------

/// The budget line of a dry run, or `None` when no limit is set for the
/// Provider. Reads the usage file (never writes it).
///
/// `would_send` is the plan: when a real run makes no Provider call (the
/// Source Language is the Target Language) nothing can be refused, and an
/// unreadable usage file does not matter, so it gives `None` then.
pub fn dry_run_budget(
    provider: ProviderName,
    budget: &BudgetSettings,
    usage_path: Option<&Path>,
    month: Month,
    needs: u64,
    would_send: bool,
) -> Result<Option<DryRunBudget>, CliError> {
    let Some(limit) = budget.limit(provider) else {
        return Ok(None);
    };
    let used = match load_state(usage_path) {
        Ok(state) => used_in(state.as_ref(), provider, month),
        Err(_) if !would_send => return Ok(None),
        Err(error) => return Err(error),
    };
    let would_refuse = would_send && decide(used, needs, Some(limit)) != BudgetDecision::Allowed;
    Ok(Some(DryRunBudget {
        limit,
        used,
        needs,
        would_refuse,
    }))
}

/// The usage file, read once. No path or no file means no state; a file that
/// cannot be used is an error.
fn load_state(usage_path: Option<&Path>) -> Result<Option<UsageState>, CliError> {
    let Some(path) = usage_path else {
        return Ok(None);
    };
    load_usage(path).map_err(|error| match error {
        UsageError::NotRegular => CliError::UsageFileNotRegular,
        _ => CliError::UsageFileUnusable,
    })
}

/// Characters counted for `provider` in `month` (0 without a state).
fn used_in(state: Option<&UsageState>, provider: ProviderName, month: Month) -> u64 {
    state.map_or(0, |state| state.used(provider, month))
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
    // One read, so every row comes from the same version of the file.
    let state = load_state(usage_path)?;
    let rows = ProviderName::ALL
        .into_iter()
        .map(|provider| StatusRow {
            provider: provider_label(provider),
            limit: budget.limit(provider),
            used: used_in(state.as_ref(), provider, month),
        })
        .collect();
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

    let key = key_name(provider);
    let keys: Vec<String> = table.iter().map(|(name, _)| name.to_string()).collect();
    let Some(position) = keys.iter().position(|name| name == key) else {
        return Ok(None);
    };

    // In TOML the comment lines above a key belong to that key. They are the
    // user's text, so they move to what follows instead of being dropped.
    let comments = table
        .get_key_value(key)
        .and_then(|(key, _)| key.leaf_decor().prefix())
        .and_then(RawString::as_str)
        .filter(|prefix| prefix.contains('#'))
        .map(str::to_string);
    table.remove(key);

    let mut homeless = None;
    if let Some(comments) = comments {
        match keys.get(position + 1) {
            Some(next) => {
                if let Some((mut next_key, _)) = table.get_key_value_mut(next) {
                    let decor = next_key.leaf_decor_mut();
                    let old = decor.prefix().and_then(RawString::as_str).unwrap_or("");
                    let merged = format!("{comments}{old}");
                    decor.set_prefix(merged);
                }
            }
            None => homeless = Some(comments),
        }
    }
    // The removed key was the last one: keep its comments above the table.
    if let Some(comments) = homeless {
        match item.as_table_mut() {
            Some(table) => {
                let old = table
                    .decor()
                    .prefix()
                    .and_then(RawString::as_str)
                    .unwrap_or("");
                let merged = format!("{old}{comments}");
                table.decor_mut().set_prefix(merged);
            }
            None => {
                let merged = format!("{}{comments}", doc.trailing().as_str().unwrap_or(""));
                doc.set_trailing(merged);
            }
        }
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
    use std::cell::Cell;
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

    fn guarded(inner: Box<dyn Provider>, limit: Option<u64>, path: &Path) -> BudgetedProvider {
        BudgetedProvider::new(inner, ProviderName::Google, limit, Some(path), month())
    }

    #[test]
    fn a_successful_call_is_counted_and_saved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        let (inner, calls) = counting(FakeProvider::new());
        let provider = guarded(inner, Some(100), &path);

        provider.translate(&request("hello")).unwrap();
        provider.translate(&request("สวัสดี")).unwrap();

        assert_eq!(calls.get(), 2);
        let state = load_usage(&path).unwrap().unwrap();
        assert_eq!(state.used(ProviderName::Google, month()), 11);
        assert_eq!(state.used(ProviderName::Anthropic, month()), 0);
        assert!(provider.failure().is_none());
        assert!(!provider.refund_failed());
    }

    /// Records what the usage file says at the moment the request is made.
    struct SeesTheFile {
        path: PathBuf,
        seen: Rc<Cell<u64>>,
    }

    impl Provider for SeesTheFile {
        fn name(&self) -> &'static str {
            "fake"
        }

        fn translate(&self, request: &TranslationRequest) -> Result<Translation, ProviderError> {
            let state = load_usage(&self.path).unwrap().unwrap();
            self.seen.set(state.used(ProviderName::Google, month()));
            FakeProvider::new().translate(request)
        }
    }

    #[test]
    fn the_characters_are_reserved_before_the_request_is_made() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        let seen = Rc::new(Cell::new(0));
        let inner = Box::new(SeesTheFile {
            path: path.clone(),
            seen: Rc::clone(&seen),
        });
        let provider = guarded(inner, Some(100), &path);

        provider.translate(&request("hello")).unwrap();

        assert_eq!(seen.get(), 5, "the count must be saved before sending");
        let state = load_usage(&path).unwrap().unwrap();
        assert_eq!(state.used(ProviderName::Google, month()), 5);
    }

    #[test]
    fn a_failed_call_gives_its_reservation_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        let mut before = UsageState::new(month());
        before.add(ProviderName::Google, month(), 3);
        save_usage(&path, &before).unwrap();
        let (inner, calls) = counting(FakeProvider::failing(ProviderError::Authentication));
        let provider = guarded(inner, Some(100), &path);

        let error = provider.translate(&request("hello")).unwrap_err();

        // The Provider's own error comes back, and the count is as before.
        assert_eq!(error, ProviderError::Authentication);
        assert_eq!(calls.get(), 1);
        assert_eq!(load_usage(&path).unwrap(), Some(before));
        assert!(provider.failure().is_none());
        assert!(!provider.refund_failed());
    }

    /// Breaks the state directory, then fails: the refund cannot be written.
    struct BreaksTheStateDirThenFails {
        dir: PathBuf,
    }

    impl Provider for BreaksTheStateDirThenFails {
        fn name(&self) -> &'static str {
            "fake"
        }

        fn translate(&self, _: &TranslationRequest) -> Result<Translation, ProviderError> {
            std::fs::remove_dir_all(&self.dir).unwrap();
            std::fs::write(&self.dir, "now a file").unwrap();
            Err(ProviderError::Authentication)
        }
    }

    #[test]
    fn a_refund_that_cannot_be_written_keeps_the_over_count_and_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let state_dir = dir.path().join("state");
        let path = state_dir.join("usage.json");
        let inner = Box::new(BreaksTheStateDirThenFails {
            dir: state_dir.clone(),
        });
        let provider = guarded(inner, Some(100), &path);

        let error = provider.translate(&request("hello")).unwrap_err();

        // Still the Provider's own error, not a budget error.
        assert_eq!(error, ProviderError::Authentication);
        assert!(provider.failure().is_none());
        assert!(provider.refund_failed());
    }

    #[test]
    fn a_language_pair_refunds_only_the_call_that_failed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        // The first call works, the second fails.
        struct FailsSecond(Cell<u32>);
        impl Provider for FailsSecond {
            fn name(&self) -> &'static str {
                "fake"
            }
            fn translate(
                &self,
                request: &TranslationRequest,
            ) -> Result<Translation, ProviderError> {
                self.0.set(self.0.get() + 1);
                if self.0.get() == 2 {
                    return Err(ProviderError::Authentication);
                }
                FakeProvider::new().translate(request)
            }
        }
        let provider = guarded(Box::new(FailsSecond(Cell::new(0))), Some(100), &path);

        provider.translate(&request("hello")).unwrap();
        provider.translate(&request("hello")).unwrap_err();

        let state = load_usage(&path).unwrap().unwrap();
        assert_eq!(state.used(ProviderName::Google, month()), 5);
    }

    #[test]
    fn a_call_over_the_limit_never_reaches_the_provider() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        let (inner, calls) = counting(FakeProvider::new());
        let provider = guarded(inner, Some(7), &path);

        provider.translate(&request("hello")).unwrap();
        provider.translate(&request("hello")).unwrap_err();

        assert_eq!(calls.get(), 1, "the refused call must not be sent");
        let refusal = provider.failure().unwrap();
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
        let provider = guarded(inner, None, &path);

        provider.translate(&request("hello")).unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ corrupt");
        assert!(!provider.refund_failed());
    }

    #[test]
    fn a_corrupt_usage_file_with_a_limit_fails_closed_at_the_first_call() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        std::fs::write(&path, "{ corrupt").unwrap();
        let (inner, calls) = counting(FakeProvider::new());
        // Nothing is read when the wrapper is built: a run that never calls
        // the Provider (Source Language equals Target Language) must not fail.
        let provider = guarded(inner, Some(100), &path);
        assert!(provider.failure().is_none());

        provider.translate(&request("hello")).unwrap_err();

        assert_eq!(calls.get(), 0);
        assert_eq!(provider.failure().unwrap().exit_code(), 3);
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
        let provider = guarded(inner, Some(10), &path);

        provider.translate(&request("hello")).unwrap();

        let state = load_usage(&path).unwrap().unwrap();
        assert_eq!(state.used(ProviderName::Google, month()), 5);
    }

    #[cfg(unix)]
    #[test]
    fn a_counter_that_cannot_be_saved_stops_the_run_before_any_call() {
        let dir = tempfile::tempdir().unwrap();
        // A dangling symlink where the directory should be: the file is
        // simply missing (0 used), but nothing can ever be saved there.
        let link = dir.path().join("state");
        std::os::unix::fs::symlink(dir.path().join("nowhere"), &link).unwrap();
        let path = link.join("usage.json");
        let (inner, calls) = counting(FakeProvider::new());
        let provider = guarded(inner, Some(100), &path);

        provider.translate(&request("hello")).unwrap_err();
        provider.translate(&request("hello")).unwrap_err();

        assert_eq!(calls.get(), 0, "no request may be sent");
        let error = provider.failure().unwrap();
        assert_eq!(error.exit_code(), 3);
        assert!(error.to_string().contains("cannot be saved"));
    }

    #[test]
    fn a_usage_file_that_is_not_a_regular_file_stops_every_run_before_any_call() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        std::fs::create_dir(&path).unwrap();

        // Each run is a fresh wrapper, like a fresh `kun` process.
        for _ in 0..3 {
            let (inner, calls) = counting(FakeProvider::new());
            let provider = guarded(inner, Some(100), &path);

            provider.translate(&request("hello")).unwrap_err();

            assert_eq!(calls.get(), 0, "no request may be sent");
            let error = provider.failure().unwrap();
            assert_eq!(error.exit_code(), 3);
            assert!(error.to_string().contains("not a regular file"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_symlink_as_the_usage_file_stops_the_run_before_any_call() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        std::os::unix::fs::symlink(dir.path().join("nowhere"), &path).unwrap();
        let (inner, calls) = counting(FakeProvider::new());
        let provider = guarded(inner, Some(100), &path);

        provider.translate(&request("hello")).unwrap_err();

        assert_eq!(calls.get(), 0);
        assert_eq!(provider.failure().unwrap().exit_code(), 3);
        // The symlink was not followed, so its target was never created.
        assert!(!dir.path().join("nowhere").exists());
    }

    #[test]
    fn a_limit_without_a_usage_path_fails_at_the_first_call() {
        let (inner, calls) = counting(FakeProvider::new());
        let provider = BudgetedProvider::new(inner, ProviderName::Google, Some(5), None, month());

        provider.translate(&request("hello")).unwrap_err();

        assert_eq!(calls.get(), 0);
        assert_eq!(provider.failure().unwrap().exit_code(), 1);
    }

    #[test]
    fn repeated_runs_never_pass_the_budget() {
        // Each run is a fresh wrapper, like a fresh `kun` process.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        let mut sent = 0;
        for _ in 0..10 {
            let (inner, calls) = counting(FakeProvider::new());
            let provider = guarded(inner, Some(12), &path);
            let _ = provider.translate(&request("hello"));
            sent += calls.get() * 5;
        }
        assert_eq!(sent, 10);
        let state = load_usage(&path).unwrap().unwrap();
        assert_eq!(state.used(ProviderName::Google, month()), 10);
    }

    #[test]
    fn the_dry_run_budget_reads_but_never_writes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        let budget = BudgetSettings {
            google_chars_per_month: Some(10),
            anthropic_chars_per_month: None,
        };

        let dry = |provider, needs, would_send| {
            dry_run_budget(provider, &budget, Some(&path), month(), needs, would_send)
        };

        assert!(dry(ProviderName::Anthropic, 5, true).unwrap().is_none());

        let report = dry(ProviderName::Google, 11, true).unwrap().unwrap();
        assert_eq!((report.limit, report.used, report.needs), (10, 0, 11));
        assert!(report.would_refuse);

        let report = dry(ProviderName::Google, 10, true).unwrap().unwrap();
        assert!(!report.would_refuse);
        assert!(!path.exists());
    }

    #[test]
    fn a_dry_run_that_sends_nothing_is_never_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        let mut state = UsageState::new(month());
        state.add(ProviderName::Google, month(), 10);
        save_usage(&path, &state).unwrap();
        // The limit was lowered below the usage, but no Provider call is planned.
        let budget = BudgetSettings {
            google_chars_per_month: Some(5),
            anthropic_chars_per_month: None,
        };

        let report = dry_run_budget(
            ProviderName::Google,
            &budget,
            Some(&path),
            month(),
            0,
            false,
        )
        .unwrap()
        .unwrap();

        assert_eq!((report.limit, report.used, report.needs), (5, 10, 0));
        assert!(!report.would_refuse);
    }

    #[test]
    fn a_dry_run_that_sends_nothing_ignores_an_unreadable_usage_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        std::fs::write(&path, "{ corrupt").unwrap();
        let budget = BudgetSettings {
            google_chars_per_month: Some(5),
            anthropic_chars_per_month: None,
        };

        let nothing_sent = dry_run_budget(
            ProviderName::Google,
            &budget,
            Some(&path),
            month(),
            0,
            false,
        );
        assert!(nothing_sent.unwrap().is_none());

        let sent = dry_run_budget(ProviderName::Google, &budget, Some(&path), month(), 3, true);
        assert_eq!(sent.err().unwrap().exit_code(), 3);
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
    fn unset_keeps_the_comments_above_the_removed_key() {
        // The comment above a key belongs to that key in TOML. It is the
        // user's text: it moves to what follows instead of being dropped.
        let original = "[budget]\n# my free tier\ngoogle_chars_per_month = 10\n# paid, small\nanthropic_chars_per_month = 5\n";
        let text = unset_limit_in(original, ProviderName::Google)
            .unwrap()
            .unwrap();
        assert_eq!(
            text,
            "[budget]\n# my free tier\n# paid, small\nanthropic_chars_per_month = 5\n"
        );
    }

    #[test]
    fn unset_keeps_the_comments_of_the_last_key_too() {
        let original = "[budget]\ngoogle_chars_per_month = 10\n# paid, small\nanthropic_chars_per_month = 5\n\n[secret_guard]\nforce_redact = true\n";
        let text = unset_limit_in(original, ProviderName::Anthropic)
            .unwrap()
            .unwrap();
        assert!(text.contains("# paid, small"), "{text}");
        assert!(!text.contains("anthropic_chars_per_month"), "{text}");
        assert!(text.contains("google_chars_per_month = 10"), "{text}");
        assert!(text.contains("force_redact = true"), "{text}");
        // Still valid TOML.
        text.parse::<DocumentMut>().unwrap();
    }

    #[test]
    fn unset_keeps_the_comments_of_the_only_key() {
        let original = "# top\n[budget]\n# my free tier\ngoogle_chars_per_month = 10\n";
        let text = unset_limit_in(original, ProviderName::Google)
            .unwrap()
            .unwrap();
        assert!(text.contains("# my free tier"), "{text}");
        assert!(text.contains("# top"), "{text}");
        text.parse::<DocumentMut>().unwrap();
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
