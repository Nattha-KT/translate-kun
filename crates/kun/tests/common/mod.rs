//! Shared helpers for the integration tests.

#![allow(dead_code)] // each test file uses a different part

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use tempfile::TempDir;

/// An isolated place to run `kun`: its own HOME, XDG_CONFIG_HOME and working
/// directory, all temporary, so the developer's real config is never read.
pub struct Sandbox {
    home: TempDir,
    xdg: TempDir,
    work: TempDir,
}

impl Sandbox {
    pub fn new() -> Self {
        Sandbox {
            home: tempfile::tempdir().unwrap(),
            xdg: tempfile::tempdir().unwrap(),
            work: tempfile::tempdir().unwrap(),
        }
    }

    pub fn work_dir(&self) -> &Path {
        self.work.path()
    }

    /// Writes a Project Config (`.kun.toml`) in the working directory.
    pub fn write_project_config(&self, text: &str) {
        std::fs::write(self.work.path().join(".kun.toml"), text).unwrap();
    }

    /// Where the User Config lives in this sandbox.
    pub fn user_config_path(&self) -> PathBuf {
        self.xdg.path().join("kun/config.toml")
    }

    /// Writes a User Config under XDG_CONFIG_HOME, with mode 0600.
    pub fn write_user_config(&self, text: &str) {
        let path = self.user_config_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
    }

    /// A plain `std::process::Command` for `kun` with the sandbox environment,
    /// for tests that need to spawn it and feed stdin slowly.
    pub fn std_command(&self) -> std::process::Command {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_kun"));
        command
            .env("HOME", self.home.path())
            .env("XDG_CONFIG_HOME", self.xdg.path())
            .env_remove("KUN_GOOGLE_API_KEY")
            .env_remove("ANTHROPIC_API_KEY")
            .current_dir(self.work.path());
        command
    }

    /// A `kun` command with the sandbox environment. Stdin is empty unless
    /// the test calls `write_stdin`.
    pub fn kun(&self) -> Command {
        let mut command = Command::cargo_bin("kun").unwrap();
        command
            .env("HOME", self.home.path())
            .env("XDG_CONFIG_HOME", self.xdg.path())
            .env_remove("KUN_GOOGLE_API_KEY")
            .env_remove("ANTHROPIC_API_KEY")
            .current_dir(self.work.path());
        command
    }
}
