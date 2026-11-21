# Architecture

This page is for contributors. It explains how the code is organised, what happens during one request, and how to add a Provider. Terms such as Provider, Target Language and Secret Guard are defined in [GLOSSARY.md](../GLOSSARY.md).

## Crate layout

`translate-kun` is a Cargo workspace with two crates:

| Crate | Path | Role |
|---|---|---|
| `kun-core` | `crates/kun-core` | All logic: config, credentials, Secret Guard, Protected Terms, language codes, Providers. **No terminal I/O.** |
| `kun` | `crates/kun` | The CLI Adapter: argument parsing, reading stdin, prompts, printing, exit codes. Builds the `kun` binary. |

The rule is simple: terminal I/O, prompts and process exit codes live only in `crates/kun`. That keeps `kun-core` easy to test and lets other Adapters (for example a future editor extension) reuse it.

Other constraints (see [AGENTS.md](../AGENTS.md)): no async (the HTTP client is blocking `ureq`), `thiserror` in `kun-core`, simple idiomatic Rust, and no secret may appear in `Debug`, `Display` or error text.

## Module map

### `kun-core`

| Module | Responsibility |
|---|---|
| `language` | `LanguageCode`: a validated `xx` or `xx-YY` code. |
| `config` | A directory of small modules. Public paths stay `kun_core::config::X` (everything is re-exported from `config/mod.rs`). It loads the Project Config and User Config, enforces the trust rules ([ADR 0003](adr/0003-project-config-is-untrusted.md)), and merges layers into an `EffectiveConfig`. |
| `config/schema.rs` | The config types: `FileConfig`, `EffectiveConfig`, `Overrides`, `ProviderName`, `LanguagePair`, `ProviderSettings`, `SecretGuardSettings`, `FileCredentials`. `Debug` hides free-text values. |
| `config/error.rs` | `ConfigError`. Messages hold paths and key names, never values. |
| `config/load.rs` | Reads and validates files (`load_project_config`, `load_user_config`) and checks the `0600` permission for `[credentials]`. |
| `config/trust.rs` | The Project Config forbidden keys (ADR 0003): `endpoint`, `allow_secrets`, credential-like names. |
| `config/merge.rs` | `merge`: CLI > Project > User > defaults, and the tighten-only security fields. |
| `config/discover.rs` | `find_project_config` (walk up for `.kun.toml`) and `user_config_path` (XDG or `~/.config`). |
| `credentials` | `Secret` (a value that never prints), the `CredentialStore` trait with `KeyringStore` (OS keychain) and `MemoryStore` (tests), and `resolve_credential` (env → keychain → User Config file). |
| `secret_guard` | Finds possible secrets (`SecretGuard::scan`), decides what to do (`decide`), and redacts (`redact`). Findings hold a kind and a byte range, never the value. |
| `protected_terms` | `protect` swaps terms for placeholders; `restore` puts them back after translation. |
| `translate` | Target Language resolution (`resolve_target`), the Language Pair rule, and the call into a Provider (`translate`, `translate_with_terms`). |
| `provider` | The `Provider` trait, `TranslationRequest`, `Translation`, `ProviderError`. Submodules: `google`, `anthropic`, `fake`. |

### `kun`

| Module | Responsibility |
|---|---|
| `main` | Entry point. Parses arguments, calls `app::run`, prints the error, returns the exit code. |
| `args` | The `clap` definitions, and the rule that only `:xx` / `xx:yy` in first position is a language. |
| `app` | The main flow (below). |
| `settings` | Finds and loads the config files from disk and merges them with the CLI overrides. Produces warnings. |
| `input` | Reads the text from arguments or stdin, with a size cap on reading. |
| `guard` | Runs the Secret Guard and acts on its decision: prompt, redact or block. |
| `providers` | Builds the chosen Provider from the config and the credential sources. |
| `auth` | `kun auth login / logout / status`. |
| `init` | `kun init`: writes the `.kun.toml` template. |
| `json_output` | The `--json` object. A public contract: see [json-output.md](json-output.md). |
| `error` | `CliError`, and the one place where errors become exit codes. |

## Lifecycle of one request

`app::run` for `kun :th "hello"`:

1. **Arguments.** `clap` parses the flags. `split_positionals` takes an optional `:xx` / `xx:yy` from the first word, and the rest is the text.
2. **Config.** `load_settings` finds the Project Config (walking up from the current directory) and the User Config, validates them, and merges them with the CLI overrides into an `EffectiveConfig`.
3. **Input.** `read_input` takes the text from the arguments or, if there are none, from stdin. The size limit (`max_chars`) is checked in characters. Empty text is a usage error.
4. **Target Language.** `resolve_target` picks one Target Language, a Language Pair, or nothing (a usage error). This happens before the Secret Guard and before any Provider is built, so a missing Target Language never costs a prompt or a network call.
5. **Secret Guard.** `run_secret_guard` scans the text and gets a decision: send, redact, ask the user, or block.
6. **Provider.** `build_provider` refuses a configured `providers.<name>.endpoint` for the chosen Provider (exit 3, since custom endpoints are not built yet), looks up the credential (env → keychain → User Config file) and builds the Provider. `translate` calls it. For a Language Pair, the direction is chosen from the detected Source Language.
7. **Output.** The translation goes to stdout; diagnostics go to stderr. On a terminal, control characters other than `\n` and `\t` are stripped from the translation first. With `--json`, one JSON object goes to stdout and warnings go inside it.

Any step can return a `CliError`. `main` prints its fixed message to stderr and exits with `CliError::exit_code()`.

> **The Secret Guard is the Adapter's job.** `kun-core` does not enforce it: `translate` and `Provider::translate` send whatever text they are given. Every Adapter (the CLI today, any other front end later) must run `SecretGuard::scan` and act on `decide` before it makes any Provider call.

## The `Provider` trait

A Provider is a translation service. The trait is small, synchronous and object-safe, so the CLI can hold a `Box<dyn Provider>`:

```rust
pub trait Provider {
    /// A short stable name such as "google", used for `--provider` and output.
    fn name(&self) -> &'static str;

    /// Translates `request.text` into `request.target`.
    fn translate(&self, request: &TranslationRequest) -> Result<Translation, ProviderError>;
}
```

- `TranslationRequest` holds the text, an optional Source Language (`None` means detect) and the Target Language.
- `Translation` holds the translated text, the detected Source Language (if the Provider reports one) and the Provider's name.
- `ProviderError` has only fixed variants (`MissingCredentials`, `Authentication`, `Network(reason)`, `RateLimited`, `UnsupportedLanguage`, `InvalidResponse`, `Refused`). It never carries free text, so an error can never leak a key, a URL or the user's text.

## How to add a new Provider

Say you are adding a Provider called `deepl`.

1. **Implement it in `kun-core`.** Create `crates/kun-core/src/provider/deepl.rs` and add `pub mod deepl;` in `provider.rs`. Write a struct that holds the credential (a `Secret`) and the timeout, and implement `Provider` for it. Use `ureq` with the timeout, and honour the proxy variables (`ureq::Proxy::try_from_env()`) like the existing Providers.
2. **Map every failure to a `ProviderError`.** Turn transport errors and HTTP statuses into fixed categories. Never put a URL, header or response body in an error. Look at `google.rs` for the pattern.
3. **Keep secrets out of output.** The credential must not appear in a URL query, a log line, or `Debug` output. Write a manual `Debug` that prints `<redacted>` for anything that may hold user text or a key.
4. **Register the name.** In `config/schema.rs`, add a `ProviderName` variant and include it in `ProviderName::ALL`. In `credentials.rs`, add its environment variable to `env_var_name`, and make the credential lookup cover it.
5. **Wire it into the CLI.** In `crates/kun/src/providers.rs`, add it to `provider_label` and add a match arm in `build_provider` that resolves the credential and builds your Provider. Add the matching "no key found" `CliError` variant (exit code 4) in `error.rs` if it needs its own message.
6. **Test it without the network** (see below): parse a recorded sample response, and check error mapping and that a fake key never appears in `{:?}` or `{:#?}`.
7. **Document it.** Update [usage.md](usage.md) (the Providers table), [configuration.md](configuration.md) and the `CHANGELOG.md`. A new Provider that sends text to a new company should also be mentioned in the README's security section.

Per [ADR 0001](adr/0001-official-providers-no-scraping.md), only official, key-authenticated APIs are accepted. No scraping of unofficial endpoints.

## Testing strategy

- **No network in tests.** Unit and integration tests never call a real service.
- **The fake Provider.** `kun-core::provider::fake::FakeProvider` is a deterministic, offline Provider. The `kun` crate exposes it with the `fake-provider` Cargo feature (`--provider fake`). With that feature, `kun auth` uses an in-memory store, so tests can never touch the real keychain. The feature must never be a default feature.
- **Integration tests** in `crates/kun/tests` run the real binary with `assert_cmd` against the fake Provider, with `HOME` and `XDG_CONFIG_HOME` pointed at temporary directories.
- **Live tests.** Tests that call real Provider APIs sit behind `--features live` in `kun-core` (`cargo test -p kun-core --features live`). They skip themselves when no credential is set, and CI never runs them.
- **Secret Guard** is developed test-first, with both must-detect and must-not-detect samples.
- **Leak tests.** Any type or error that can hold user text or a key has a test that formats it with `{:?}` and `{:#?}` and checks that a fake key is absent. Test keys must look obviously fake (for example contain `EXAMPLE` or `FAKE`) but still exercise the detector.

The commands to run everything are in [CONTRIBUTING.md](../CONTRIBUTING.md#build-test-and-lint).
