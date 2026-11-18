//! Loading the Project Config and User Config from disk and merging them.

use std::path::Path;

use kun_core::config::{
    EffectiveConfig, FileConfig, Overrides, find_project_config, load_project_config,
    load_user_config, merge, user_config_path,
};

use crate::error::CliError;

/// The merged settings, plus warnings for the caller to print on stderr.
pub struct Loaded {
    pub config: EffectiveConfig,
    pub warnings: Vec<String>,
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
        Some(path) => Some(load_project_config(&path).map_err(CliError::Config)?),
        None => None,
    };
    let user = load_user(home, xdg_config_home, &mut warnings)?;

    let config = merge(overrides, project.as_ref(), user.as_ref());
    Ok(Loaded { config, warnings })
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
    if !is_private(&path) {
        warnings.push(format!(
            "the User Config {} is readable by other users; run `chmod 600` on it",
            path.display()
        ));
    }
    load_user_config(&path).map(Some).map_err(CliError::Config)
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
