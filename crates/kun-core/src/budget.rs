//! Usage Budget: the local monthly limit and the counter file behind it.
//!
//! Pure decisions live here (month arithmetic, "does this fit?") together with
//! the small usage state file. There is no terminal I/O.

use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::ProviderName;

/// A calendar month in UTC, such as `2026-11`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Month {
    year: i64,
    month: u32,
}

impl Month {
    /// The UTC month that contains `secs` seconds since the Unix epoch.
    pub fn from_unix_secs(secs: i64) -> Month {
        let days = secs.div_euclid(86_400);
        // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let day_of_era = z.rem_euclid(146_097);
        let year_of_era =
            (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let shifted_month = (5 * day_of_year + 2) / 153;
        let month = if shifted_month < 10 {
            shifted_month + 3
        } else {
            shifted_month - 9
        };
        let year = year_of_era + era * 400 + i64::from(month <= 2);
        Month {
            year,
            // Always 1..=12 by construction.
            month: u32::try_from(month).unwrap_or(1),
        }
    }

    /// The current UTC month. A clock before 1970 counts as 1970.
    pub fn now() -> Month {
        let secs = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(elapsed) => i64::try_from(elapsed.as_secs()).unwrap_or(0),
            Err(_) => 0,
        };
        Month::from_unix_secs(secs)
    }

    /// Reads `YYYY-MM`. Anything else gives `None`.
    pub fn parse(text: &str) -> Option<Month> {
        let (year, month) = text.split_once('-')?;
        if year.len() != 4 || month.len() != 2 {
            return None;
        }
        if !year.bytes().all(|byte| byte.is_ascii_digit())
            || !month.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        let year: i64 = year.parse().ok()?;
        let month: u32 = month.parse().ok()?;
        if !(1..=12).contains(&month) {
            return None;
        }
        Some(Month { year, month })
    }

    /// The date the counter resets, `YYYY-MM-01` of the next month (UTC).
    pub fn first_of_next(&self) -> String {
        if self.month == 12 {
            format!("{:04}-01-01", self.year + 1)
        } else {
            format!("{:04}-{:02}-01", self.year, self.month + 1)
        }
    }
}

impl fmt::Display for Month {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}", self.year, self.month)
    }
}

/// The answer to "may this text be sent?".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetDecision {
    /// Send it: there is no limit, or the text fits.
    Allowed,
    /// Do not send it. The numbers are for the message.
    Refused {
        /// Characters already counted this month.
        used: u64,
        /// Characters this text would add.
        needed: u64,
        /// The monthly limit.
        limit: u64,
    },
}

/// `used + needed` may reach the limit exactly, but not pass it.
pub fn decide(used: u64, needed: u64, limit: Option<u64>) -> BudgetDecision {
    let Some(limit) = limit else {
        return BudgetDecision::Allowed;
    };
    if used.saturating_add(needed) > limit {
        return BudgetDecision::Refused {
            used,
            needed,
            limit,
        };
    }
    BudgetDecision::Allowed
}

/// The characters counted so far in one month, per Provider. Counters only:
/// never text, hashes or timestamps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageState {
    month: Month,
    google: u64,
    anthropic: u64,
}

impl UsageState {
    /// A state with nothing counted in `month`.
    pub fn new(month: Month) -> UsageState {
        UsageState {
            month,
            google: 0,
            anthropic: 0,
        }
    }

    /// Characters counted for `provider` in `month`. A state that belongs to
    /// another month counts as 0.
    pub fn used(&self, provider: ProviderName, month: Month) -> u64 {
        if self.month != month {
            return 0;
        }
        match provider {
            ProviderName::Google => self.google,
            ProviderName::Anthropic => self.anthropic,
        }
    }

    /// Counts `chars` for `provider` in `month`, starting over first when the
    /// state belongs to another month.
    pub fn add(&mut self, provider: ProviderName, month: Month, chars: u64) {
        if self.month != month {
            *self = UsageState::new(month);
        }
        let counter = match provider {
            ProviderName::Google => &mut self.google,
            ProviderName::Anthropic => &mut self.anthropic,
        };
        *counter = counter.saturating_add(chars);
    }
}

/// Why the usage state file could not be used. Fixed categories only: the
/// file's content and path are never part of an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum UsageError {
    /// The file exists but could not be read.
    #[error("the usage file could not be read")]
    Unreadable,
    /// The file was read but is not a valid usage file.
    #[error("the usage file is not valid")]
    Corrupt,
    /// The file could not be written.
    #[error("the usage file could not be written")]
    WriteFailed,
}

/// The shape of the state file. Counters and the month, nothing else.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UsageFile {
    version: u32,
    month: String,
    google: u64,
    anthropic: u64,
}

/// The only file format version there is.
const USAGE_FILE_VERSION: u32 = 1;

/// `$XDG_STATE_HOME/kun/usage.json`, otherwise `~/.local/state/kun/usage.json`.
///
/// The caller passes the environment values in, so this stays testable.
pub fn usage_state_path(home: Option<&Path>, xdg_state_home: Option<&Path>) -> Option<PathBuf> {
    // The XDG spec says a relative `XDG_STATE_HOME` must be ignored.
    if let Some(xdg) = xdg_state_home.filter(|path| path.is_absolute()) {
        return Some(xdg.join("kun").join("usage.json"));
    }
    home.map(|home| {
        home.join(".local")
            .join("state")
            .join("kun")
            .join("usage.json")
    })
}

/// Reads the usage file. `Ok(None)` when there is no file.
pub fn load_usage(path: &Path) -> Result<Option<UsageState>, UsageError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(UsageError::Unreadable),
    };
    let file: UsageFile = serde_json::from_str(&text).map_err(|_| UsageError::Corrupt)?;
    if file.version != USAGE_FILE_VERSION {
        return Err(UsageError::Corrupt);
    }
    let month = Month::parse(&file.month).ok_or(UsageError::Corrupt)?;
    Ok(Some(UsageState {
        month,
        google: file.google,
        anthropic: file.anthropic,
    }))
}

/// Writes the usage file atomically with mode `0600`. Creates the directory.
pub fn save_usage(path: &Path, state: &UsageState) -> Result<(), UsageError> {
    let file = UsageFile {
        version: USAGE_FILE_VERSION,
        month: state.month.to_string(),
        google: state.google,
        anthropic: state.anthropic,
    };
    let text = serde_json::to_string(&file).map_err(|_| UsageError::WriteFailed)?;
    write_private_file(path, text.as_bytes()).map_err(|_| UsageError::WriteFailed)
}

/// Writes `bytes` to `path` through a temporary file in the same directory,
/// then renames it over `path`. The file has mode `0600` on Unix and the
/// directory is created when missing. Renaming replaces a symlink itself, so
/// the file a symlink points to is never overwritten.
pub fn write_private_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("no file name"))?;
    let temp = path.with_file_name(format!(
        ".{}.tmp-{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    // A leftover from a crashed run of the same process id.
    let _ = std::fs::remove_file(&temp);

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = options
        .open(&temp)
        .and_then(|mut file| file.write_all(bytes))
        .and_then(|()| std::fs::rename(&temp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProviderName;

    #[test]
    fn a_text_under_the_limit_is_allowed() {
        assert_eq!(decide(100, 50, Some(1000)), BudgetDecision::Allowed);
    }

    #[test]
    fn a_text_that_exactly_reaches_the_limit_is_allowed() {
        assert_eq!(decide(950, 50, Some(1000)), BudgetDecision::Allowed);
    }

    #[test]
    fn a_text_one_over_the_limit_is_refused_with_the_numbers() {
        assert_eq!(
            decide(951, 50, Some(1000)),
            BudgetDecision::Refused {
                used: 951,
                needed: 50,
                limit: 1000
            }
        );
    }

    #[test]
    fn no_limit_means_always_allowed() {
        assert_eq!(decide(u64::MAX, u64::MAX, None), BudgetDecision::Allowed);
    }

    #[test]
    fn huge_numbers_do_not_overflow() {
        assert!(matches!(
            decide(u64::MAX, 1, Some(10)),
            BudgetDecision::Refused { .. }
        ));
    }

    #[test]
    fn months_come_from_unix_seconds_in_utc() {
        for (secs, expected) in [
            (0, "1970-01"),
            (1_000_000_000, "2001-09"),
            (1_709_164_800, "2024-02"), // 2024-02-29T00:00:00Z, a leap day
            (1_767_225_599, "2025-12"), // one second before 2026-01-01
            (1_767_225_600, "2026-01"),
        ] {
            assert_eq!(Month::from_unix_secs(secs).to_string(), expected, "{secs}");
        }
    }

    #[test]
    fn the_reset_date_is_the_first_of_the_next_month() {
        let nov = Month::parse("2026-11").unwrap();
        assert_eq!(nov.first_of_next(), "2026-12-01");
        let dec = Month::parse("2026-12").unwrap();
        assert_eq!(dec.first_of_next(), "2027-01-01");
    }

    #[test]
    fn month_text_must_be_year_dash_month() {
        for bad in ["", "2026", "2026-13", "2026-00", "26-11", "2026-1", "x-y"] {
            assert_eq!(Month::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn counters_add_up_per_provider() {
        let month = Month::parse("2026-11").unwrap();
        let mut state = UsageState::new(month);
        state.add(ProviderName::Google, month, 10);
        state.add(ProviderName::Google, month, 5);
        state.add(ProviderName::Anthropic, month, 7);
        assert_eq!(state.used(ProviderName::Google, month), 15);
        assert_eq!(state.used(ProviderName::Anthropic, month), 7);
    }

    fn absolute(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(name)
    }

    #[test]
    fn the_state_path_prefers_xdg_state_home_then_home() {
        let home = absolute("home-me");
        let xdg = absolute("custom-state");
        assert_eq!(
            usage_state_path(Some(&home), Some(&xdg)),
            Some(xdg.join("kun").join("usage.json"))
        );
        assert_eq!(
            usage_state_path(Some(&home), None),
            Some(home.join(".local/state/kun/usage.json"))
        );
        assert_eq!(usage_state_path(None, None), None);
        // A relative XDG_STATE_HOME must be ignored (XDG spec).
        assert_eq!(
            usage_state_path(Some(&home), Some(std::path::Path::new("relative"))),
            Some(home.join(".local/state/kun/usage.json"))
        );
    }

    #[test]
    fn a_missing_file_is_no_state() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_usage(&dir.path().join("usage.json")), Ok(None));
    }

    #[test]
    fn saving_creates_the_directory_and_loading_reads_it_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b/usage.json");
        let month = Month::parse("2026-11").unwrap();
        let mut state = UsageState::new(month);
        state.add(ProviderName::Google, month, 12_345);

        save_usage(&path, &state).unwrap();

        assert_eq!(load_usage(&path), Ok(Some(state)));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            r#"{"version":1,"month":"2026-11","google":12345,"anthropic":0}"#
        );
    }

    #[test]
    fn broken_files_are_errors_not_zero() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        for text in [
            "",
            "not json",
            "{}",
            r#"{"version":2,"month":"2026-11","google":1,"anthropic":0}"#,
            r#"{"version":1,"month":"soon","google":1,"anthropic":0}"#,
            r#"{"version":1,"month":"2026-11","google":-1,"anthropic":0}"#,
            r#"{"version":1,"month":"2026-11","google":1,"anthropic":0,"text":"hello"}"#,
        ] {
            std::fs::write(&path, text).unwrap();
            assert_eq!(load_usage(&path), Err(UsageError::Corrupt), "{text}");
        }
    }

    #[test]
    fn a_directory_in_place_of_the_file_is_unreadable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        std::fs::create_dir(&path).unwrap();
        assert_eq!(load_usage(&path), Err(UsageError::Unreadable));
    }

    #[cfg(unix)]
    #[test]
    fn the_state_file_is_private_and_leaves_no_temporary_file() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.json");
        let state = UsageState::new(Month::parse("2026-11").unwrap());

        save_usage(&path, &state).unwrap();
        save_usage(&path, &state).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, [std::ffi::OsString::from("usage.json")]);
    }

    #[cfg(unix)]
    #[test]
    fn writing_replaces_a_symlink_instead_of_writing_through_it() {
        let dir = tempfile::tempdir().unwrap();
        let other = dir.path().join("dotfiles-original");
        std::fs::write(&other, "precious").unwrap();
        let link = dir.path().join("config.toml");
        std::os::unix::fs::symlink(&other, &link).unwrap();

        write_private_file(&link, b"new").unwrap();

        assert_eq!(std::fs::read_to_string(&other).unwrap(), "precious");
        assert!(!link.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&link).unwrap(), "new");
    }

    #[test]
    fn errors_say_nothing_but_a_fixed_message() {
        for error in [UsageError::Unreadable, UsageError::Corrupt] {
            assert!(!error.to_string().is_empty());
            assert!(!format!("{error:?}").contains('/'));
        }
    }

    #[test]
    fn a_new_month_starts_from_zero() {
        let nov = Month::parse("2026-11").unwrap();
        let dec = Month::parse("2026-12").unwrap();
        let mut state = UsageState::new(nov);
        state.add(ProviderName::Google, nov, 400);
        // Reading in December sees 0, and the first December call resets the file.
        assert_eq!(state.used(ProviderName::Google, dec), 0);
        state.add(ProviderName::Google, dec, 3);
        assert_eq!(state.used(ProviderName::Google, dec), 3);
        assert_eq!(state.used(ProviderName::Anthropic, dec), 0);
    }
}
