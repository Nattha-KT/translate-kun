//! Project Config and User Config: loading, trust rules and merging.
//!
//! No terminal I/O here. Credential values from a User Config `[credentials]`
//! table are kept private and only handed out as a `Secret`.
//!
//! The module is split by job; every public item is re-exported here, so
//! callers use `kun_core::config::X`:
//!
//! - `schema`: the config types (`FileConfig`, `EffectiveConfig`, `BudgetSettings`, ...)
//! - `error`: `ConfigError`
//! - `load`: reading files, validation and the `0600` permission check
//! - `owner`: the uid comparison behind skipping a Project Config owned by another user (Unix)
//! - `trust`: keys a Project Config may never contain (ADR 0003)
//! - `merge`: precedence and the tighten-only rules
//! - `discover`: finding the Project Config and the User Config path

mod discover;
mod error;
mod load;
mod merge;
mod owner;
mod schema;
mod trust;

pub use discover::{find_project_config, user_config_path};
pub use error::ConfigError;
#[cfg(unix)]
pub use load::is_private_file;
pub use load::{load_project_config, load_project_config_if_owned, load_user_config};
pub use merge::merge;
pub use owner::is_owned_by;
pub use schema::{
    BudgetSettings, DEFAULT_MAX_CHARS, DEFAULT_TIMEOUT_SECS, EffectiveConfig, FileConfig,
    FileCredentials, LanguagePair, Overrides, ProviderName, ProviderSettings, SecretGuardSettings,
};
