//! `kun init`: writes a commented `.kun.toml` template.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use crate::error::CliError;

/// The file name of a Project Config.
const FILE_NAME: &str = ".kun.toml";

/// Prose comments start with `# `. Example settings start with `#` and no
/// space, so removing that one `#` gives a valid setting.
const TEMPLATE: &str = r#"# kun Project Config. Commit this file to share settings with your team.
#
# Credentials and endpoints are NOT allowed in this file (API keys, endpoint
# URLs, allow_secrets). Put them in the keychain (`kun auth login <provider>`),
# an environment variable, or the User Config (~/.config/kun/config.toml).
#
# Remove the leading `#` of a line to use it.

# Default Target Language.
#target = "th"

# Language Pair: text in the first language goes to the second, anything else
# goes to the first.
#pair = ["th", "en"]

# Which Provider to use: "google" or "anthropic".
#provider = "google"

# Terms that must stay untranslated.
#protected_terms = ["kun", "Cargo.toml"]

# A short style hint for the Provider.
#style = "formal"

# Longest accepted input, in characters (default 10000).
#max_chars = 10000

# Provider timeout in seconds (default 15).
#timeout_secs = 15

# Secret Guard: extra regex patterns to treat as secrets, and whether to always
# replace possible secrets with [REDACTED].
#[secret_guard]
#extra_patterns = ["internal-[a-z0-9]{16}"]
#force_redact = true
"#;

/// Writes the template into `dir`. Without `force`, an existing file is never
/// touched.
pub fn init_in(dir: &Path, force: bool) -> Result<(), CliError> {
    let write_failed = CliError::Unexpected("cannot write `.kun.toml` in the current directory");
    let path = dir.join(FILE_NAME);

    if !force {
        // Fails if the file exists (even a dangling symlink), with no race
        // between the check and the create.
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    CliError::Usage(
                        "`.kun.toml` already exists here; use `kun init --force` to overwrite it",
                    )
                } else {
                    write_failed
                }
            })?;
        return file.write_all(TEMPLATE.as_bytes()).map_err(|_| {
            CliError::Unexpected("cannot write `.kun.toml` in the current directory")
        });
    }

    // --force: write a temporary file next to it and rename it over
    // `.kun.toml`. Renaming replaces a symlink itself, so the file the
    // symlink points to is never overwritten.
    let temp = dir.join(format!(".kun.toml.tmp-{}", std::process::id()));
    let result = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .and_then(|mut file| file.write_all(TEMPLATE.as_bytes()))
        .and_then(|()| std::fs::rename(&temp, &path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
        return Err(write_failed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_the_file_and_refuses_to_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        init_in(dir.path(), false).unwrap();
        let path = dir.path().join(".kun.toml");
        assert!(path.is_file());
        kun_core::config::load_project_config(&path).unwrap();

        let error = init_in(dir.path(), false).unwrap_err();
        assert_eq!(error.exit_code(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn force_replaces_a_symlink_instead_of_writing_through_it() {
        let dir = tempfile::tempdir().unwrap();
        let other = dir.path().join("dotfiles-original");
        std::fs::write(&other, "precious").unwrap();
        let link = dir.path().join(".kun.toml");
        std::os::unix::fs::symlink(&other, &link).unwrap();

        init_in(dir.path(), true).unwrap();

        assert_eq!(std::fs::read_to_string(&other).unwrap(), "precious");
        assert!(!link.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&link).unwrap(), TEMPLATE);
    }

    #[test]
    fn force_leaves_no_temporary_file_behind() {
        let dir = tempfile::tempdir().unwrap();
        init_in(dir.path(), true).unwrap();
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, [std::ffi::OsString::from(".kun.toml")]);
    }

    #[test]
    fn force_replaces_the_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".kun.toml");
        std::fs::write(&path, "x".repeat(5000)).unwrap();
        init_in(dir.path(), true).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), TEMPLATE);
    }
}
