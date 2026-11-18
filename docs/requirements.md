# translate-kun — v1 Requirements

Status: **draft, pending sign-off** (discovery session 2026-11-18).
Vocabulary follows [`GLOSSARY.md`](../GLOSSARY.md). Key decisions are recorded in [`docs/adr/`](./adr/).

## 1. Purpose

`kun` is a command-line translator for developers who live in the terminal. It translates short-to-medium text (sentences, error messages, snippets, short docs) and is pipe-friendly enough for scripts. It is inspired by Translate Shell (`trans`) but fixes its main weakness: `trans` scrapes unofficial web endpoints and breaks; `kun` uses official, key-authenticated Providers only ([ADR 0001](./adr/0001-official-providers-no-scraping.md)). Thai ↔ English is a first-class use case.

### Users (priority order)

1. Developers translating ad-hoc text in a terminal.
2. Scripts and automation consuming stable output.
3. Future Adapters (e.g. a VS Code extension) that call `kun --json`.

### Goals beyond the tool itself

- The author is learning Rust through this project; prefer simple designs over clever ones.

## 2. Scope

### In v1

- Translate text given as arguments or via stdin.
- Automatic Source Language detection.
- Language Pair (auto-direction, e.g. en ↔ th).
- Project Config (`kun init`) and User Config.
- Protected Terms.
- Secret Guard.
- Credential storage in the OS keychain (`kun auth login`).
- Two Providers: Google Cloud Translation (default) and Anthropic.
- `--json` output for scripts and Adapters.
- Distribution as a prebuilt binary for macOS and Linux.

### Later (not v1)

- Interactive REPL mode.
- Dictionary mode (definitions, alternatives, examples).
- Text-to-speech / pronunciation.
- Format-preserving file translation (Markdown, code comments) and chunking of long input.
- Additional Providers: DeepL, Ollama/local models, offline models.
- VS Code Adapter (v2).
- Opt-in translation cache.

### Never

- Scraping unofficial/free web endpoints.
- GUI or web UI.
- Telemetry, analytics, or automatic update checks.
- Accepting API keys as CLI flags.

## 3. Command-line interface

Command name: `kun`. Project name: `translate-kun`.

```sh
kun "hello"                 # Target Language from config / Language Pair
kun :th "hello"             # ":" prefix = Target Language
kun en:th "hello"           # Source and Target Language
kun -s en -t th "hello"     # long form for scripts
echo "hello" | kun :th      # stdin
kun :th < README.md > README.th.md

kun init                    # create .kun.toml in the current directory
kun auth login google       # store a credential in the OS keychain (hidden input)
kun auth login anthropic
kun auth logout <provider>
kun auth status

kun --provider anthropic :th "hello"
kun --json :th "hello"
```

### Rules

- A language code is recognised **only** with the `:` prefix form (`:th`, `en:th`) or with `-s`/`-t`. A bare trailing word is always text (so `kun I love it` translates "I love it"; `it` is never read as Italian).
- If both arguments and stdin are present, arguments win, and a warning is printed to stderr.

### Target Language resolution (first match wins)

1. Explicit `:xx` / `-t xx`.
2. Language Pair, if configured: detected Source = first language → translate to the second; detected Source = second → translate to the first; any other language → translate to the first.
3. `target` from config (following the precedence in §5).
4. Error: no Target Language (exit code 2).

If the detected Source Language equals the Target Language, and no Language Pair applies, print the input unchanged and add a warning on stderr.

## 4. Providers

| Provider | Role in v1 | Notes |
|---|---|---|
| Google Cloud Translation | Default | Built-in language detection; strong Thai support |
| Anthropic | Second Provider | Default model `claude-haiku-5-5`, configurable in config |

- Providers sit behind a single abstraction so new ones can be added without touching the CLI.
- **No automatic fallback** between Providers. If one fails, `kun` exits with an error and suggests `--provider <other>`; it never sends the text to a second company on its own.
- Custom endpoints (e.g. local Ollama, later) can be configured **only in the User Config**.
- For the LLM Provider, the text to translate is passed as data, clearly separated from instructions. A style or tone from the Project Config can affect the output only.

## 5. Configuration

| | Project Config | User Config |
|---|---|---|
| Location | `.kun.toml`, found by walking up from the current directory | `~/.config/kun/config.toml` (XDG) |
| Created by | `kun init` (commented template) | user / `kun auth login` |
| Trust | **Untrusted** ([ADR 0003](./adr/0003-project-config-is-untrusted.md)) | Trusted |

**Precedence**: CLI flag > Project Config > User Config > built-in defaults. One exception: a **security setting** in the Project Config may only make the setting stricter (§7).

### Project Config may set

- `target`: Target Language.
- `pair`: Language Pair, e.g. `["en", "th"]`.
- `provider`: one of the built-in Provider names only.
- `protected_terms`: list of Protected Terms.
- `style`: tone or style hint (used by the LLM Provider only).
- Extra Secret Guard patterns, and forcing `redact` on.

### Project Config must not set

- Credentials of any kind → **hard error** (exit 3); `kun` refuses to run.
- Endpoint or base URLs.
- Anything that weakens the Secret Guard (`allow_secrets`, disabling the guard, removing patterns).

## 6. Credentials

Lookup order (first found wins):

1. Environment variable (`KUN_GOOGLE_API_KEY`, `ANTHROPIC_API_KEY`). Intended for CI.
2. OS keychain: macOS Keychain, Linux Secret Service/libsecret. Windows Credential Manager on a best-effort basis.
3. User Config file. Allowed only if its permissions are `0600`; otherwise `kun` warns.

Rules:

- `kun auth login <provider>` reads the key with hidden input and stores it in the keychain.
- Keys are never accepted as CLI flags (they would leak into shell history and `ps`).
- Keys are never printed, logged, or included in `--json` output or error messages.

Limitation (stated, not hidden): no option protects against malware running as the same user. The keychain is the strongest of the three.

## 7. Secret Guard

Runs **before** any text leaves the machine.

| Situation | Behaviour |
|---|---|
| Interactive TTY, secret detected | Prompt `Possible secret detected. Send anyway? [y/N]`; the default is No |
| Non-interactive (pipe, script), secret detected | **Block** (exit 6) unless `--allow-secrets` |
| `--redact` | Replace each match with `[REDACTED]` and continue |

- Ships with built-in patterns for common API keys, tokens, private keys and similar credentials.
- The Project Config may **add** patterns or force `redact`. It may never remove patterns or allow secrets.
- `--allow-secrets` is available as a CLI flag or in the User Config only.
- Developed test-first. The test suite includes both must-detect and must-not-detect samples.

## 8. Protected Terms

- A Protected Term appears in the translation exactly as written in the source.
- Configured in the Project Config (and optionally in the User Config).
- Example: `translate-kun` must never come back translated.

## 9. Output

- **stdout**: the translated text only, nothing else.
- **stderr**: metadata and diagnostics (detected Source Language, Provider used, Secret Guard warnings).
- **`--json`**: structured output to stdout with a `schema_version` field. This is a **public contract** for Adapters and is versioned ([ADR 0002](./adr/0002-rust.md)).

### Exit codes (proposed; finalise during implementation)

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Unexpected error |
| 2 | Usage error (bad arguments, no Target Language) |
| 3 | Configuration error (including a credential in the Project Config) |
| 4 | Missing credentials |
| 5 | Network or Provider error |
| 6 | Blocked by Secret Guard |
| 7 | Input exceeds the size limit |

## 10. Limits, privacy, network

- **Input limit**: 10,000 characters by default. Over the limit, `kun` stops (exit 7). Override with `--max-chars` or config. No chunking in v1.
- **No persistence of text**: no cache, no history, no logging of source or translated text.
- **No telemetry**: the only network traffic is to the chosen Provider.
- **Proxy**: `kun` honours `HTTPS_PROXY` and `NO_PROXY`.
- **Timeout**: 15 seconds by default, configurable.

## 11. Technical decisions

- **Language**: Rust ([ADR 0002](./adr/0002-rust.md)).
- **Layout**: Cargo workspace.
  - `kun-core`: Providers, Secret Guard, Protected Terms, config loading. No terminal I/O.
  - `kun`: the CLI Adapter (argument parsing, rendering, prompts).
- **HTTP**: blocking client (`ureq`); no async runtime in v1.
- **Keychain**: the `keyring` crate.
- **Config format**: TOML.

## 12. Testing

- Unit and integration tests use a fake Provider: no network, no cost, safe for CI.
- Live Provider tests sit behind `cargo test --features live`. They run only when credentials are present.
- The Secret Guard is developed test-first (TDD).

## 13. Distribution & licensing

- Prebuilt binaries for macOS and Linux on GitHub Releases, built with `cargo-dist`. Windows on a best-effort basis.
- Homebrew tap. `cargo install` for Rust users.
- Public open source, licensed `MIT OR Apache-2.0` (`LICENSE-MIT` and `LICENSE-APACHE`).

## 14. Open items to verify before/while implementing

- Current pricing and free tiers of Google Cloud Translation and DeepL (figures in the session came from memory).
- DeepL's Thai support (relevant only when DeepL is added).
- How well Source Language detection works on very short input (one or two words), and whether the Language Pair needs a fallback rule for low-confidence detections.
- Exact `--json` schema and final exit-code numbers.
