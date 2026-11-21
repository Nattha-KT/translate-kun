//! Who owns a discovered Project Config (like git's `safe.directory`).
//!
//! A `.kun.toml` found by walking up from the working directory may sit in a
//! directory another user controls (a shared `/tmp`, say). On Unix `kun` skips
//! such a file. Outside Unix there is no check.

#[cfg(unix)]
use std::path::Path;

/// True when the file's owner is the user running `kun`.
pub fn is_owned_by(file_uid: u32, current_uid: u32) -> bool {
    file_uid == current_uid
}

/// True when the file at `path` is owned by the user running `kun`.
///
/// A file that cannot be inspected counts as owned: reading it will fail next
/// with a proper I/O error, so this check never hides that error.
#[cfg(unix)]
pub fn is_owned_by_current_user(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;

    match std::fs::metadata(path) {
        Ok(metadata) => is_owned_by(metadata.uid(), rustix::process::getuid().as_raw()),
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_uid_is_owned() {
        assert!(is_owned_by(1000, 1000));
        assert!(is_owned_by(0, 0));
    }

    #[test]
    fn different_uid_is_not_owned() {
        assert!(!is_owned_by(0, 1000));
        assert!(!is_owned_by(1000, 0));
    }

    #[cfg(unix)]
    #[test]
    fn a_file_we_just_created_is_owned_by_us() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".kun.toml");
        std::fs::write(&file, "").unwrap();
        assert!(is_owned_by_current_user(&file));
    }

    #[cfg(unix)]
    #[test]
    fn a_missing_file_is_left_to_the_reader_to_report() {
        let dir = tempfile::tempdir().unwrap();
        assert!(is_owned_by_current_user(&dir.path().join("nope.toml")));
    }
}
