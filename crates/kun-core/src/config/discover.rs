//! Finding the Project Config and the User Config path.

use std::path::{Component, Path, PathBuf};

/// File name of a Project Config.
const PROJECT_CONFIG_NAME: &str = ".kun.toml";

/// `path` made absolute, with `.` and `..` components removed.
///
/// `..` is resolved lexically (without looking at symlinks).
fn absolute_normalized(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut result = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}

/// Walk up from `start` to the filesystem root and return the closest `.kun.toml`.
///
/// A relative `start` (such as `.`) is made absolute first, so the walk really
/// reaches the filesystem root. The returned path is absolute.
pub fn find_project_config(start: &Path) -> Option<PathBuf> {
    absolute_normalized(start)
        .ancestors()
        .map(|dir| dir.join(PROJECT_CONFIG_NAME))
        .find(|candidate| candidate.is_file())
}

/// `$XDG_CONFIG_HOME/kun/config.toml`, otherwise `~/.config/kun/config.toml`.
///
/// The caller passes the environment values in, so this stays testable.
pub fn user_config_path(home: Option<&Path>, xdg_config_home: Option<&Path>) -> Option<PathBuf> {
    // The XDG spec says a relative `XDG_CONFIG_HOME` must be ignored.
    if let Some(xdg) = xdg_config_home.filter(|path| path.is_absolute()) {
        return Some(xdg.join("kun").join("config.toml"));
    }
    home.map(|home| home.join(".config").join("kun").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Unix only: on Windows the temp dir and the current directory can be on
    // different drives, and no relative path crosses drives.
    #[cfg(unix)]
    #[test]
    fn relative_start_is_resolved_to_an_absolute_path_first() {
        let root = tempfile::tempdir().unwrap();
        let inner = root.path().join("a/b");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(root.path().join(".kun.toml"), "").unwrap();

        // Build a relative path from the current directory to `inner`:
        // enough `..` to reach the filesystem root, then down to `inner`.
        let cwd = std::env::current_dir().unwrap();
        let mut relative = PathBuf::new();
        for _ in cwd.components().skip(1) {
            relative.push("..");
        }
        relative.extend(inner.components().skip(1));
        assert!(relative.is_relative());

        assert_eq!(
            find_project_config(&relative),
            Some(root.path().join(".kun.toml"))
        );
    }

    #[test]
    fn dot_start_gives_an_absolute_result() {
        // `.` is the usual start. Whatever is found must be an absolute path.
        if let Some(found) = find_project_config(Path::new(".")) {
            assert!(found.is_absolute(), "{found:?}");
        }
    }

    #[test]
    fn find_project_config_returns_the_closest_one() {
        let root = tempfile::tempdir().unwrap();
        let inner = root.path().join("a/b");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(root.path().join(".kun.toml"), "").unwrap();
        std::fs::write(root.path().join("a/.kun.toml"), "").unwrap();

        assert_eq!(
            find_project_config(&inner),
            Some(root.path().join("a/.kun.toml"))
        );
        assert_eq!(
            find_project_config(root.path()),
            Some(root.path().join(".kun.toml"))
        );
    }

    #[test]
    fn find_project_config_walks_up_several_levels() {
        let root = tempfile::tempdir().unwrap();
        let deep = root.path().join("a/b/c");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(root.path().join(".kun.toml"), "").unwrap();

        assert_eq!(
            find_project_config(&deep),
            Some(root.path().join(".kun.toml"))
        );
    }

    #[test]
    fn find_project_config_ignores_a_directory_named_like_the_file() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join(".kun.toml")).unwrap();
        // May find a real one above the temp dir on an unusual machine, but never the directory.
        assert_ne!(
            find_project_config(root.path()),
            Some(root.path().join(".kun.toml"))
        );
    }

    /// An absolute path on every OS (`/home/me` has no drive letter, so it
    /// is not absolute on Windows).
    fn absolute(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    #[test]
    fn user_config_path_prefers_xdg_then_home() {
        let home = absolute("home-me");
        let xdg = absolute("custom-xdg");
        let from_xdg = xdg.join("kun").join("config.toml");
        let from_home = home.join(".config").join("kun").join("config.toml");
        assert_eq!(
            user_config_path(Some(&home), Some(&xdg)),
            Some(from_xdg.clone())
        );
        assert_eq!(user_config_path(Some(&home), None), Some(from_home));
        assert_eq!(user_config_path(None, None), None);
        assert_eq!(user_config_path(None, Some(&xdg)), Some(from_xdg));
    }

    #[test]
    fn user_config_path_ignores_empty_or_relative_xdg() {
        let home = absolute("home-me");
        for xdg in ["", "relative/dir"] {
            assert_eq!(
                user_config_path(Some(&home), Some(Path::new(xdg))),
                Some(home.join(".config").join("kun").join("config.toml")),
                "{xdg:?}"
            );
        }
    }
}
