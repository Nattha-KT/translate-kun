//! The error type for loading config files.

use std::path::PathBuf;

use thiserror::Error;

/// Why a config file could not be used.
///
/// Messages hold only paths and key names, never config values.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// The file could not be read.
    #[error("cannot read config file {}", path.display())]
    Io {
        /// The file that could not be read.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The file is not valid TOML, or has an unknown key or a wrong type.
    #[error("cannot parse config file {}", path.display())]
    Parse {
        /// The file that could not be parsed.
        path: PathBuf,
    },
    // `{key:?}` escapes control characters, so a hostile key name cannot
    // inject terminal escape sequences into the message.
    /// A Project Config holds a key that only the User Config may hold.
    #[error("{key:?} is not allowed in a Project Config ({})", path.display())]
    ForbiddenInProjectConfig {
        /// The Project Config file.
        path: PathBuf,
        /// The dotted path of the forbidden key.
        key: String,
    },
    /// A setting has a value that is not allowed.
    #[error("invalid setting {} in {}", key, path.display())]
    Invalid {
        /// The config file.
        path: PathBuf,
        /// The name of the invalid setting.
        key: String,
    },
}
