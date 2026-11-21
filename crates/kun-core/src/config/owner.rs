//! Who owns a discovered Project Config (like git's `safe.directory`).
//!
//! A `.kun.toml` found by walking up from the working directory may sit in a
//! directory another user controls (a shared `/tmp`, say). On Unix `kun` skips
//! such a file; see `load_project_config_if_owned`, which reads the owner from
//! the open file. Outside Unix there is no check.

/// True when the file's owner is the user running `kun`.
pub fn is_owned_by(file_uid: u32, current_uid: u32) -> bool {
    file_uid == current_uid
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
}
