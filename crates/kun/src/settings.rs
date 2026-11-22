//! Loading the Project Config and User Config from disk and merging them.

use std::path::Path;

use kun_core::config::{
    BudgetSettings, EffectiveConfig, FileConfig, FileCredentials, Overrides, find_project_config,
    load_project_config_if_owned, load_user_config, merge, user_config_path,
};

use crate::error::{CliError, ConfigFile};

/// The merged settings, plus warnings for the caller to print on stderr.
pub struct Loaded {
    pub config: EffectiveConfig,
    pub warnings: Vec<String>,
    /// The User Config `[credentials]` table (empty if none, or if it was
    /// ignored because the file is readable by others).
    pub user_credentials: Option<FileCredentials>,
}

/// Discovers the Project Config from `start_dir`, loads the User Config from
/// `home` / `xdg_config_home`, and merges them with the CLI `overrides`.
///
/// The environment values are passed in (not read here) so the caller owns
/// all process state.
pub fn load_settings(
    start_dir: &Path,
    home: Option<&Path>,
    xdg_config_home: Option<&Path>,
    overrides: &Overrides,
) -> Result<Loaded, CliError> {
    let mut warnings = Vec::new();

    let project = match find_project_config(start_dir) {
        Some(path) => {
            let loaded = load_project_config_if_owned(&path)
                .map_err(|e| CliError::config(ConfigFile::Project, &e))?;
            skipped_project_warning(loaded, &mut warnings)
        }
        None => None,
    };
    let user = load_user(home, xdg_config_home, &mut warnings)?;

    let config = merge(overrides, project.as_ref(), user.as_ref());
    let user_credentials = user.map(|file| file.credentials);
    Ok(Loaded {
        config,
        warnings,
        user_credentials,
    })
}

/// `None` from `load_project_config_if_owned` means the file belongs to
/// another user and was skipped (like git's `safe.directory`), so a `.kun.toml`
/// planted in a shared directory cannot change `kun`'s behaviour. That is not
/// an error: add a warning and carry on with the User Config and defaults.
///
/// The warning is a fixed text. The path can hold user-chosen text, so it is
/// not printed.
fn skipped_project_warning(
    loaded: Option<FileConfig>,
    warnings: &mut Vec<String>,
) -> Option<FileConfig> {
    if loaded.is_none() {
        warnings.push("ignoring .kun.toml: owned by another user".to_string());
    }
    loaded
}

/// What `kun auth status` needs from the User Config.
pub struct UserCredentials {
    pub credentials: Option<FileCredentials>,
    /// The file has `[credentials]` but is readable by others, so they were dropped.
    pub ignored_insecure: bool,
    pub warnings: Vec<String>,
}

/// Loads only the User Config (never the Project Config), so a broken
/// `.kun.toml` cannot stop `kun auth`.
pub fn load_user_credentials(
    home: Option<&Path>,
    xdg_config_home: Option<&Path>,
) -> Result<UserCredentials, CliError> {
    let mut warnings = Vec::new();
    let user = load_user(home, xdg_config_home, &mut warnings)?;
    let ignored_insecure = user
        .as_ref()
        .is_some_and(|file| file.credentials_ignored_insecure);
    Ok(UserCredentials {
        credentials: user.map(|file| file.credentials),
        ignored_insecure,
        warnings,
    })
}

/// The `[budget]` table of the User Config (empty when there is none). Only
/// the User Config is read, never the Project Config: the Usage Budget cannot
/// come from there, and a broken `.kun.toml` must not stop `kun budget`.
pub fn load_user_budget(
    home: Option<&Path>,
    xdg_config_home: Option<&Path>,
) -> Result<BudgetSettings, CliError> {
    let mut warnings = Vec::new();
    let user = load_user(home, xdg_config_home, &mut warnings)?;
    Ok(user.map(|file| file.budget).unwrap_or_default())
}

/// The User Config, or `None` when there is no file. A file that exists but
/// cannot be used is an error.
fn load_user(
    home: Option<&Path>,
    xdg_config_home: Option<&Path>,
    warnings: &mut Vec<String>,
) -> Result<Option<FileConfig>, CliError> {
    let Some(path) = user_config_path(home, xdg_config_home) else {
        return Ok(None);
    };
    if !path.is_file() {
        return Ok(None);
    }
    let config = load_user_config(&path).map_err(|e| CliError::config(ConfigFile::User, &e))?;
    if config.credentials_ignored_insecure {
        warnings.push(format!(
            "{} is readable by others; its [credentials] were ignored. Run: chmod 600 {}",
            path.display(),
            path.display()
        ));
    } else if !is_private(&path) {
        warnings.push(format!(
            "the User Config {} is readable by other users; run `chmod 600` on it",
            path.display()
        ));
    }
    Ok(Some(config))
}

#[cfg(unix)]
fn is_private(path: &Path) -> bool {
    kun_core::config::is_private_file(path)
}

/// File modes are not checked off Unix.
#[cfg(not(unix))]
fn is_private(_path: &Path) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lang(code: &str) -> kun_core::language::LanguageCode {
        code.parse().unwrap()
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn user_config_is_found_under_home() {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let file = home.path().join(".config/kun/config.toml");
        write(&file, "target = \"fr\"");
        set_private(&file);

        let loaded =
            load_settings(work.path(), Some(home.path()), None, &Overrides::default()).unwrap();
        assert_eq!(loaded.config.target, Some(lang("fr")));
        assert!(loaded.warnings.is_empty());
    }

    #[test]
    fn xdg_config_home_wins_over_home() {
        let home = tempfile::tempdir().unwrap();
        let xdg = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        write(
            &home.path().join(".config/kun/config.toml"),
            "target = \"fr\"",
        );
        write(&xdg.path().join("kun/config.toml"), "target = \"de\"");

        let loaded = load_settings(
            work.path(),
            Some(home.path()),
            Some(xdg.path()),
            &Overrides::default(),
        )
        .unwrap();
        assert_eq!(loaded.config.target, Some(lang("de")));
    }

    #[test]
    fn project_config_beats_user_config_and_cli_beats_both() {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        write(
            &home.path().join(".config/kun/config.toml"),
            "max_chars = 300",
        );
        write(&work.path().join(".kun.toml"), "max_chars = 200");

        let loaded =
            load_settings(work.path(), Some(home.path()), None, &Overrides::default()).unwrap();
        assert_eq!(loaded.config.max_chars, 200);

        let overrides = Overrides {
            max_chars: Some(100),
            ..Overrides::default()
        };
        let loaded = load_settings(work.path(), Some(home.path()), None, &overrides).unwrap();
        assert_eq!(loaded.config.max_chars, 100);
    }

    #[test]
    fn the_budget_comes_from_the_user_config_and_a_project_config_cannot_set_it() {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        write(
            &home.path().join(".config/kun/config.toml"),
            "[budget]\ngoogle_chars_per_month = 450000",
        );

        let loaded =
            load_settings(work.path(), Some(home.path()), None, &Overrides::default()).unwrap();
        assert_eq!(
            loaded
                .config
                .budget
                .limit(kun_core::config::ProviderName::Google),
            Some(450_000)
        );
        let only_budget = load_user_budget(Some(home.path()), None).unwrap();
        assert_eq!(only_budget, loaded.config.budget);

        write(
            &work.path().join(".kun.toml"),
            "[budget]\ngoogle_chars_per_month = 999999999",
        );
        let error = load_settings(work.path(), Some(home.path()), None, &Overrides::default())
            .err()
            .unwrap();
        assert_eq!(error.exit_code(), 3);
        // `kun budget status` ignores the Project Config altogether.
        assert!(load_user_budget(Some(home.path()), None).is_ok());
    }

    #[test]
    fn no_config_files_gives_defaults() {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let loaded =
            load_settings(work.path(), Some(home.path()), None, &Overrides::default()).unwrap();
        assert_eq!(loaded.config.max_chars, 10_000);
    }

    #[test]
    fn forbidden_project_config_is_a_config_error() {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        write(
            &work.path().join(".kun.toml"),
            "[secret_guard]\nallow_secrets = true",
        );
        let error = load_settings(work.path(), Some(home.path()), None, &Overrides::default())
            .err()
            .unwrap();
        assert_eq!(error.exit_code(), 3);
    }

    #[test]
    fn a_skipped_project_config_gives_a_fixed_warning_without_a_path() {
        let mut warnings = Vec::new();

        let loaded = skipped_project_warning(None, &mut warnings);

        assert!(loaded.is_none());
        assert_eq!(
            warnings,
            vec!["ignoring .kun.toml: owned by another user".to_string()]
        );
    }

    #[test]
    fn a_loaded_project_config_gives_no_warning() {
        let mut warnings = Vec::new();

        let loaded = skipped_project_warning(Some(FileConfig::default()), &mut warnings);

        assert!(loaded.is_some());
        assert!(warnings.is_empty());
    }

    #[test]
    fn a_project_config_we_own_is_loaded_by_load_settings() {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        write(&work.path().join(".kun.toml"), "max_chars = 200");

        let loaded =
            load_settings(work.path(), Some(home.path()), None, &Overrides::default()).unwrap();

        assert_eq!(loaded.config.max_chars, 200);
        assert!(loaded.warnings.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_world_readable_user_config_gives_a_warning() {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let file = home.path().join(".config/kun/config.toml");
        write(&file, "target = \"fr\"");
        set_mode(&file, 0o644);

        let loaded =
            load_settings(work.path(), Some(home.path()), None, &Overrides::default()).unwrap();
        assert_eq!(loaded.warnings.len(), 1);
        assert!(loaded.warnings[0].contains("config.toml"));
    }

    #[cfg(unix)]
    fn set_mode(path: &Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    }

    fn set_private(path: &Path) {
        #[cfg(unix)]
        set_mode(path, 0o600);
        #[cfg(not(unix))]
        let _ = path;
    }
}
