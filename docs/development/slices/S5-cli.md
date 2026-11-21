# S5 — CLI: arguments, stdin, Language Pair, Secret Guard flow, output, exit codes

## Goal

Make `kun` a working CLI end-to-end against the `Provider` trait. It parses the agreed syntax, loads and merges config, runs the Secret Guard, resolves the Target Language (including the Language Pair), translates, and writes output with the agreed exit codes. Real Providers are wired in a later slice. Until then the CLI tests use the fake Provider.

## Read first

- `AGENTS.md` (all)
- `docs/requirements.md` §3 (CLI), §7 (Secret Guard behaviour table), §9 (output and exit codes), §10 (limits)
- Existing code: `crates/kun-core/src/{config.rs,secret_guard.rs,provider.rs,provider/fake.rs,language.rs}`

## Skills

Use `tdd`. Use `codebase-design` for the new core module in item 1.

## In scope

- `crates/kun-core/src/translate.rs` (new) and one `pub mod translate;` line in `lib.rs`
- `crates/kun/**`: main.rs, new modules, and tests
- `crates/kun/Cargo.toml`, `Cargo.lock`. Allowed new dependencies in `kun`: `is-terminal` (or `std::io::IsTerminal`, preferred, no dependency), plus `assert_cmd` and `predicates` as dev-dependencies (already present).
- Do not modify `secret_guard.rs`, `config.rs`, `provider.rs` or `language.rs`. If you need a change there, stop and report it.

## Behaviour

1. **Core orchestration, in `kun-core/src/translate.rs` (no I/O).**
   - `resolve_target(explicit: Option<&LanguageCode>, pair: Option<&LanguagePair>, configured: Option<&LanguageCode>) -> TargetPlan`
   - `TargetPlan` is one of `Fixed(code)`, `Pair(pair)` or `Missing`. The order of precedence follows §3.
   - `translate(provider: &dyn Provider, text, source: Option<LanguageCode>, plan: TargetPlan) -> Result<Translation, TranslateError>`
   - Language Pair rule, which works for any pair:
     1. Translate to `pair.first`.
     2. If the Provider reports `detected_source == pair.first`, translate **again** to `pair.second` and return that result.
     3. Otherwise return the first result.
     4. If the source was given explicitly, decide the direction from it without a second call.
   - If there is no pair and the detected source equals the target, return the input unchanged with a warning flag set (§3).
   - Unit-test all of this with `FakeProvider`. The fake already detects `th` from Thai script, otherwise `en`.

2. **Argument parsing (`kun`, clap derive).** Implement the §3 rules exactly:
   - The first positional argument is a language spec only if it matches `:xx` or `xx:yy`, where `xx` and `yy` are valid `LanguageCode`s. All remaining positional arguments are joined with single spaces into the text.
   - `-s` / `--source`, `-t` / `--target`, `--provider`, `--json` (reserved: if passed, exit 2 with "not implemented yet"), `--max-chars`, `--allow-secrets`, `--redact`.
   - `kun I love it` translates "I love it". `it` is never a language.

3. **Input.** If positional text is given, use it. Otherwise read stdin when stdin is not a terminal. If both are present, the arguments win and a warning goes to stderr. With no text at all, print help to stderr and exit 2.

4. **Config.**
   - Discover the Project Config from the current directory.
   - Load the User Config from the `HOME` and `XDG_CONFIG_HOME` env vars.
   - Merge with the CLI overrides.
   - If the User Config file is not private (unix), print a warning to stderr.
   - Config errors exit with code 3.

5. **Size limit.** If the text is longer than `max_chars` (count `chars()`, not bytes), print a clear message to stderr and exit 7.

6. **Secret Guard flow.** Build `SecretGuard::new(&effective.extra_patterns)`, then `scan`, then `decide(findings, policy, interactive)`.
   - `interactive` = stdin **and** stderr are terminals. When stdin is piped, it is never interactive.
   - `AskUser`: prompt on stderr `Possible secret detected (<kinds>). Send anyway? [y/N] ` and read the answer from the terminal (`/dev/tty`). Anything other than `y` or `yes` means block.
   - `Redact`: replace the text with `redact()` output before sending, and note this on stderr.
   - `Block`: list the finding kinds and positions on stderr (never the values) and exit 6.

7. **Provider selection.** `fn build_provider(name, &EffectiveConfig) -> Result<Box<dyn Provider>, ...>`
   - Supported names in this slice: `fake`, only when the cargo feature `fake-provider` is enabled. The integration tests enable it. It must **not** be in default features.
   - `google` and `anthropic` return a clear "provider not wired yet" error with exit 1. A later slice replaces this.
   - The default provider name is `google`.

8. **Output.**
   - stdout: the translated text only, with a trailing newline.
   - stderr: metadata such as the detected source and the provider, and all warnings.

9. **Exit codes (§9).**

   | Code | Meaning |
   |---|---|
   | 0 | ok |
   | 1 | unexpected or provider-not-wired |
   | 2 | usage |
   | 3 | config |
   | 4 | missing credentials |
   | 5 | network or provider |
   | 6 | secret blocked |
   | 7 | too large |

   Map them in one place, from a `CliError` enum. `ProviderError::MissingCredentials` maps to 4. Every other `ProviderError` maps to 5.

## Tests (integration, `assert_cmd`, `--features fake-provider`)

- `kun --provider fake :th hello` prints `[th] hello` to stdout and exits 0.
- `kun --provider fake I love it` with config target `it`: the text stays "I love it".
- stdin piped input; args plus stdin gives the stderr warning.
- Language Pair `["en","th"]` in a temp `.kun.toml`:
  - English input gives `[th] ...`
  - Thai input gives `[en] ...`
- Over-long input exits 7.
- A fake AWS key in piped input exits 6, and the key does not appear in stderr.
- The same input with `--redact` succeeds, and stdout contains `[REDACTED]`, not the key.
- A Project Config containing `allow_secrets` exits 3.
- Isolate HOME and XDG in every test (point them at temp dirs) so the developer's real config is never read.

## Out of scope

- Real Providers, `kun init`, `kun auth` and `--json` (S6/S7).

## Acceptance evidence

- The Definition of Done in `AGENTS.md` passes, with tests run as `cargo test --workspace --features kun/fake-provider` (or document the exact command).
- Report the CLI help output, the test count, and any requirement you could not meet.
