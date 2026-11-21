# S2 — Config loading + Project Config trust rules

## Goal

Load the Project Config and the User Config, merge them with CLI overrides using the agreed precedence, and enforce the rule that the Project Config is untrusted. No terminal I/O and no reading of credential values; both come in later slices.

## Read first

- `AGENTS.md` (all)
- `docs/requirements.md` §5 (Configuration), §6 (the first rule only), §7 (the Project Config and Secret Guard interaction), §10 (the defaults for limits and timeout)
- `docs/adr/0003-project-config-is-untrusted.md`
- `GLOSSARY.md`: Project Config, User Config, Language Pair, Protected Term, Provider

## Skills

Use `codebase-design` for the public types, so they stay small. Use `tdd` for every behaviour below.

## In scope

- `crates/kun-core/src/config.rs`, or a `config/` module directory
- `crates/kun-core/src/lib.rs`: add **only** `pub mod config;`
- `crates/kun-core/Cargo.toml`: add `serde` (derive) and `toml`. You may add `tempfile` as a dev-dependency.
- `Cargo.lock`
- `crates/kun-core/tests/config.rs` (optional)

Reuse `crate::language::LanguageCode` (merged from S3). Do not touch `secret_guard` or `provider`.

## Behaviour

1. **File shape (TOML).** Both files share one schema. Use `#[serde(deny_unknown_fields)]` so unknown keys are errors.

   ```toml
   target = "th"
   pair = ["en", "th"]
   provider = "google"            # built-in names only: "google" | "anthropic"
   protected_terms = ["translate-kun"]
   style = "casual, developer tone"
   max_chars = 10000
   timeout_secs = 15

   [secret_guard]
   extra_patterns = ["ACME-[0-9]{6}"]
   force_redact = false
   allow_secrets = false          # User Config only

   [providers.anthropic]
   model = "claude-haiku-5-5"
   endpoint = "http://localhost:11434"   # User Config only
   ```

2. **Discovery.** `find_project_config(start: &Path) -> Option<PathBuf>` walks up from `start` to the filesystem root, looking for `.kun.toml`. It returns the closest one.

3. **User Config location.** `user_config_path(home: Option<&Path>, xdg_config_home: Option<&Path>) -> Option<PathBuf>` returns `$XDG_CONFIG_HOME/kun/config.toml`, otherwise `~/.config/kun/config.toml`.
   - Pass these values in as arguments. Do not read the process environment inside `kun-core`, so the function stays testable.

4. **Project Config is untrusted (ADR 0003).** Loading a Project Config is a **hard error** if it contains any of the following:
   - `secret_guard.allow_secrets` (any value)
   - any `endpoint`
   - any key whose name looks like a credential: it contains `key`, `token`, `secret`, `password`, or `credential`, case-insensitive. Allow-list the legitimate names above, such as `extra_patterns`.

   The error names the file path and the key path. **It never includes the value.**

5. **Merge.** `merge(cli: &Overrides, project: Option<&FileConfig>, user: Option<&FileConfig>) -> EffectiveConfig`
   - Normal fields: CLI > Project > User > defaults (`max_chars` 10000, `timeout_secs` 15).
   - **Tighten-only security fields:**
     - `force_redact` = OR of all layers.
     - `extra_patterns` = union, de-duplicated, order kept.
     - `allow_secrets` comes from CLI or User only.
     - `endpoint` comes from User only.
   - `provider` is an enum, so an unknown name is a parse error.
   - `pair` must be exactly 2 different valid language codes, otherwise it is a parse or validation error.

6. **Errors.** Use `ConfigError` with `thiserror`. Variants: `Io { path }`, `Parse { path }`, `ForbiddenInProjectConfig { path, key }`, `Invalid { path, key }`.
   - Messages contain only paths and key names, never config values.
   - A test must prove that a credential value present in a Project Config does not appear in the error's `Display` or `Debug`.

7. **User Config privacy (unix only).** `is_private_file(path) -> bool` is true when the file mode is `0o600` or stricter. It goes behind `#[cfg(unix)]`. Warning the user is the CLI's job.

## Out of scope

- Reading credential values, and the keychain (S4).
- CLI flag parsing (S5). Define `Overrides` as a plain struct; S5 fills it.
- The `kun init` template (S7).
- Running the Secret Guard on config content.

## Acceptance evidence

- The Definition of Done in `AGENTS.md` passes.
- Report the public API (signatures only), the test count, and a precedence table showing which layer wins for each field.
