<div align="center">

# translate-kun

**`kun` translates text in your terminal and in your pipes, and it checks that text for known API key and token patterns before it leaves your machine.**

[![CI](https://github.com/machngai/translate-kun/actions/workflows/ci.yml/badge.svg)](https://github.com/machngai/translate-kun/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/translate-kun)](https://crates.io/crates/translate-kun)
[![Latest release](https://img.shields.io/github/v/release/machngai/translate-kun)](https://github.com/machngai/translate-kun/releases/latest)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/machngai/translate-kun/badge)](https://scorecard.dev/viewer/?uri=github.com/machngai/translate-kun)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

[Why](#why-kun) · [Install](#install) · [Quick start](#quick-start) · [Recipes](#usage-recipes) · [Configuration](#configuration) · [How it works](#how-it-works) · [Privacy](#privacy-and-security) · [Costs](#costs-and-limits) · [FAQ](#faq-and-troubleshooting)

</div>

Inspired by [Translate Shell](https://github.com/soimort/translate-shell), but built on official, key-authenticated translation services instead of scraped web endpoints, so it does not break when a free endpoint changes. Thai ↔ English is a first-class use case.

> [!NOTE]
> **Pre-1.0.** Flags, config keys and output may still change between `0.x` releases. The `--json` output is versioned (`schema_version`) and changes only in a compatible way within a version.

## Why kun

| Feature | What you get |
|---|---|
| 🖥️ **Terminal and pipes** | Text from arguments, stdin or a file. Translation on stdout, diagnostics on stderr. |
| 🛡️ **Secret Guard** | Looks for known patterns of API keys, tokens and private keys. By default it blocks what it finds, or asks you on a terminal. |
| 🔒 **Protected Terms** | Product and function names come back exactly as you wrote them. |
| 🔀 **Two Providers** | Google Cloud Translation or Anthropic. No silent fallback: your text never goes to the other one on its own. |
| 💰 **Usage Budget** | A monthly character limit. `kun` refuses before it passes it. Opt-in, ships in 0.2.0. |
| ⚙️ **Made for scripts** | `--json` output and stable exit codes (0 to 8). |
| ✅ **Verifiable releases** | Every release archive has a checksum and a build provenance attestation. |

## Install

| Method | How |
|---|---|
| Homebrew (macOS, Linux) | `brew install machngai/tap/kun` |
| crates.io (needs Rust stable, from [rustup](https://rustup.rs)) | `cargo install translate-kun` |
| GitHub Releases | Unpack an archive from the [Releases page](https://github.com/machngai/translate-kun/releases) and put `kun` on your `PATH`. Archives: macOS (Apple silicon and Intel), Linux (x86_64 and aarch64), Windows (x86_64, best effort). They also hold the licence files and `THIRD-PARTY-LICENSES.md`. |
| From source | `git clone https://github.com/machngai/translate-kun`, `cd translate-kun`, `cargo install --path crates/kun` |

Check it with `kun --version`.

<details>
<summary><b>Verify your download</b> (checksum, provenance, SBOM)</summary>

Every archive has a SHA-256 checksum and a build provenance attestation (this repository's release workflow built it). The examples use the macOS Apple silicon archive.

```sh
# Provenance: needs the GitHub CLI (`gh`), signed in with `gh auth login`.
gh attestation verify translate-kun-aarch64-apple-darwin.tar.xz \
  -R machngai/translate-kun \
  --signer-workflow machngai/translate-kun/.github/workflows/release.yml

# Checksum: download the .sha256 file next to the archive. Look for OK.
sha256sum -c translate-kun-aarch64-apple-darwin.tar.xz.sha256       # Linux
shasum -a 256 -c translate-kun-aarch64-apple-darwin.tar.xz.sha256   # macOS
```

Releases also have an SBOM (CycloneDX), and the release binaries embed their dependency list (`cargo audit bin "$(command -v kun)"`). See [SECURITY.md](SECURITY.md#security-assurance).

</details>

## Quick start

1. **Install** `kun`: `brew install machngai/tap/kun` (other ways: [Install](#install)).
2. **Get an API key** for one Provider. It is your account and your bill.
   - **Google Cloud Translation** (the default): follow the [setup guide](https://docs.cloud.google.com/translate/docs/setup), then create a key on the [Credentials page](https://console.cloud.google.com/apis/credentials).
   - **Anthropic**: create a key in the [Anthropic console](https://console.anthropic.com/settings/keys).

> [!IMPORTANT]
> Google needs **billing enabled** on the project, or the API does not work: [Enable billing](https://docs.cloud.google.com/translate/docs/setup#enable_billing).

> [!TIP]
> Only have an Anthropic key? Google stays the default, so add `--provider anthropic` or set `provider = "anthropic"`: [recipe 10](#10-use-anthropic-instead-of-google).

3. **Log in.** A hidden prompt saves the key in your OS keychain. Keys are never accepted as flags, because flags end up in your shell history. For CI and cron, set `KUN_GOOGLE_API_KEY` or `ANTHROPIC_API_KEY` instead.

   ```sh
   kun auth login google        # or: kun auth login anthropic
   ```
4. **Translate.** `:th` means "Target Language: Thai".

   ```sh
   kun :th "hello"
   ```

## Usage recipes

Each recipe has a goal, numbered steps and the output to expect. Every command and flag: [docs/usage.md](docs/usage.md).

### 1. Translate your first sentence

1. Run `kun :th "hello"`. You see:

   ```text
   provider: google, detected source: en      <- stderr
   สวัสดี                                      <- stdout (the words come from the Provider)
   ```
2. Pick the languages with one of these forms:

   | You write | Meaning |
   |---|---|
   | `kun :th "hello"` | Target Language `th`, Source Language detected |
   | `kun en:th "hello"` | Source Language `en`, Target Language `th` |
   | `kun -s en -t th "hello"` | Long form, good for scripts |
   | `kun :th I love it` | Quotes are optional: a bare word is always text |
   | `kun :th -- auth is great` | `--` goes before text that starts with a subcommand: `auth`, `budget`, `completions`, `help`, `init` |

A language is recognised only as the first argument (`:xx` or `xx:yy`) or with `-s` / `-t`. Codes look like `th` or `zh-TW`, and case does not matter. With no `:xx`, Language Pair or `target`, `kun` has no Target Language and exits with 2.

### 2. Translate a file or the clipboard

1. A file: `kun :th < notes.md > notes.th.md`
2. A pipe: `echo "good morning" | kun :th`
3. The clipboard (macOS): `pbpaste | kun :th | pbcopy`

> [!TIP]
> Sensitive text goes through stdin. Arguments are saved in your shell history, and other local users can see them in `ps`.

If you give arguments and piped stdin, the arguments win and `kun` warns you. Input over 10,000 characters exits with 7 (raise it with `--max-chars` or `max_chars`). That limit is per input, not per run: a Language Pair can send the text twice.

### 3. Switch between two languages (Language Pair)

1. Create `~/.config/kun/config.toml`, your User Config (or `$XDG_CONFIG_HOME/kun/config.toml` when that is an absolute path). It does not exist until you create it, or until `kun budget set` does.
2. Add `pair = ["en", "th"]`.
3. Leave out the language: `kun "hello"` goes en → th, and `kun "สวัสดี"` goes th → en.

Any other language goes to the first one. An explicit `:xx` or `-t xx` wins. With a detected Source Language, a Language Pair can send your text twice (once per direction).

### 4. Keep names unchanged (Protected Terms)

1. Add `protected_terms = ["translate-kun", "Cargo.toml"]` to your config.
2. Translate as usual. `kun` swaps each term for a placeholder before sending and puts it back afterwards. Matching is case-sensitive.
3. If a Provider loses a placeholder, you still get the translation, plus a warning.

> [!TIP]
> `style = "casual, developer tone"` is a tone hint. Only Anthropic uses it. Google ignores it.

### 5. Check what would be sent (`--dry-run`)

1. Add `--dry-run`. The key in this text is fake, and `--redact` masks it:

   ```sh
   echo "my key is AKIAFAKEEXAMPLE12345 ok" | kun --dry-run --redact en:th
   ```
2. Read the report. It shows the Provider and host (and the model for Anthropic), the languages, the credential source (never the key), the Secret Guard decision, and the text after redaction and Protected Terms placeholders. Nothing is sent or counted against your Usage Budget. Anthropic also receives your `style` hint, which the report does not list.

<details>
<summary>Show the report</summary>

This run assumes a Google key in `KUN_GOOGLE_API_KEY`, no Language Pair and no Usage Budget.

```text
warning: possible secrets would be replaced with [REDACTED]
Dry run: nothing was sent.
provider: google
host: translation.googleapis.com
source language: en
target language: th
second call: no
credentials: env
Secret Guard: redact (matches become [REDACTED])
  - possible AWS access key at bytes 10..30
----- begin text that would be sent -----
my key is [REDACTED] ok
----- end text that would be sent -----
```

</details>

### 6. Handle a secret in your text

1. Pipe text with a fake key. A pipe is not interactive, so the Secret Guard blocks it (exit 6):

   ```text
   $ echo "my key is AKIAFAKEEXAMPLE12345 ok" | kun en:th
   error: Secret Guard blocked the text; nothing was sent.
     - possible AWS access key at bytes 10..30
   Use --redact to mask them, or --allow-secrets to send them anyway.
   ```
2. Choose how to go on:

   | You want | Do this |
   |---|---|
   | Be asked | Run `kun` on a terminal (stdin and stderr both terminals): it asks `Send anyway? [y/N]`, default no |
   | Mask the match | `--redact`, or `force_redact = true` under `[secret_guard]`: matches become `[REDACTED]` |
   | Send it anyway | `--allow-secrets`. A forced redact wins over it |

> [!WARNING]
> The Secret Guard detects known patterns only: AWS access key IDs (and AWS secret access keys in assignment form, such as `aws_secret_access_key = …`), GitHub and GitLab tokens, Anthropic, OpenAI and Google API keys, Slack tokens, Stripe live keys, npm and Hugging Face tokens, private key blocks, JWTs, and your own `extra_patterns`. It is a safety net, not a guarantee. The message shows the kind and byte range, never the secret.

### 7. Stay inside a free tier (Usage Budget)

> [!IMPORTANT]
> **Ships in 0.2.0.** `kun budget` is on `main`, but the published 0.1.x releases do not have it. If `kun --version` shows 0.1.x, build from source to try it.

1. Set a monthly limit, in characters, for one Provider:

   ```sh
   kun budget set google 450000     # under Google's 500,000-character free tier
   ```
2. Translate as usual. If the next call would pass the limit, `kun` refuses before any network call and exits with 8. No flag overrides it.
3. Check where you stand with `kun budget`:

   ```text
   Usage budget for 2026-11 (resets on 2026-12-01, UTC):
   google: 448210 of 450000 characters used, 1790 left
   anthropic: no limit, 0 characters used
   ```
4. Raise the limit, run `kun budget unset google`, or wait for the 1st of the month (UTC).

<details>
<summary>How the Usage Budget counts</summary>

- It is off by default. It counts the characters actually sent in each Provider call, after redaction and Protected Terms. `kun` reserves them before the call and gives them back if the call fails (if that fails, the count stays too high and `kun` warns you).
- A Language Pair can make two calls, each reserved on its own. If the second is refused, the first stays counted.
- The limit lives in `[budget]` in the User Config. Counters live in `$XDG_STATE_HOME/kun/usage.json` (when absolute), else `~/.local/state/kun/usage.json`: the month and one counter per Provider, no text. If a budget is set and that file cannot be read or saved, `kun` sends nothing and exits with 3.
- It is local and approximate: it counts only this machine, and runs at the same moment can lose an increment, so the count can drift. Also set a quota cap or alert in your Provider's console, because only the Provider enforces its own limits. Anthropic bills tokens, not characters, so a character budget is a rough guide there.

</details>

### 8. Use `kun` in a script

1. Ask for JSON. One object goes to stdout. Errors are never JSON: they go to stderr and stdout stays empty.

   ```sh
   kun --json :th "hello"
   # {"schema_version":1,"text":"สวัสดี","source":"en","target":"th","provider":"google","warnings":[]}
   ```
2. Pick a field: `kun --json :th "hello" | jq -r .text`
3. Check the exit code. The fields are in [docs/json-output.md](docs/json-output.md).

<details>
<summary>Exit codes (0 to 8)</summary>

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Unexpected error, including a keychain failure |
| 2 | Usage error: bad arguments, no text, or no Target Language |
| 3 | Configuration error (including a credential, endpoint or `[budget]` in a Project Config, or a `providers.<name>.endpoint` for the Provider in use), no OS keychain for `kun auth login`, or a usage file that cannot be read or saved while a Usage Budget is set |
| 4 | Missing credentials |
| 5 | Network or Provider error |
| 6 | Blocked by the Secret Guard, or you answered "no" at its prompt |
| 7 | Input longer than the size limit |
| 8 | The Provider rate limited you or its quota, credit or billing limit was reached (wait, or check your Provider account), **or** your local Usage Budget was reached (raise it, or wait for the reset). A script can wait and retry on a rate limit. |

</details>

### 9. Per-project settings (`kun init`)

1. In your project folder, run `kun init`. It prints `Created .kun.toml`. If the file exists it fails: `kun init --force` replaces it.
2. Open `.kun.toml`. Every setting is commented out: remove the `#` to use a line. A team can share a Language Pair, Protected Terms or extra Secret Guard patterns.

> [!NOTE]
> A Project Config is **untrusted**, because it may come from a repository you cloned ([ADR 0003](docs/adr/0003-project-config-is-untrusted.md)). It can choose a Provider, a Language Pair, Protected Terms and limits, and it can only make the Secret Guard stricter. It **cannot** hold credentials, endpoints or a `[budget]` (exit 3), and it can never turn on `allow_secrets`. On Unix, a `.kun.toml` owned by another user is skipped with a warning.

### 10. Use Anthropic instead of Google

1. Create a key in the [Anthropic console](https://console.anthropic.com/settings/keys), then run `kun auth login anthropic`.
2. Use it on one run: `kun --provider anthropic :th "hello"`
3. Or make it your default: add `provider = "anthropic"` to your config.

There is **no automatic fallback**: if a Provider fails, `kun` exits with an error, and some errors suggest `--provider`. Your text never goes to a second company on its own.

> [!TIP]
> Shell completions (`bash`, `zsh`, `fish`, `powershell`, `elvish`): see [docs/usage.md](docs/usage.md#kun-completions).

## Configuration

Two optional TOML files. Precedence: command-line flag > Project Config > User Config > built-in defaults.

| File | Location | Purpose |
|---|---|---|
| User Config | `~/.config/kun/config.toml` (or `$XDG_CONFIG_HOME/kun/config.toml` when absolute) | Personal settings. The only place for credentials and endpoints. |
| Project Config | `.kun.toml`, found by walking up from the current directory | Shared project settings. Untrusted. |

<details>
<summary>An example User Config</summary>

```toml
provider = "google"                 # or "anthropic"
pair = ["en", "th"]                 # Language Pair
protected_terms = ["translate-kun"]
max_chars = 10000                   # longest accepted input, in characters

[secret_guard]
force_redact = false                # true = always redact, never ask
extra_patterns = ["internal-[a-z0-9]{16}"]

[budget]                            # User Config only; kun budget set writes this
google_chars_per_month = 450000
```

</details>

Every key, its default and where it is allowed: [docs/configuration.md](docs/configuration.md).

## How it works

[![How one translation runs: kun CLI, Secret Guard, Protected Terms, Usage Budget, Provider adapter, then HTTPS to Google or Anthropic](docs/assets/architecture.svg)](docs/assets/architecture.svg)

*Click to open it full size. The animation is CSS only, and a still image shows if your system asks for reduced motion.*

The diagram shows the path of a request that reaches a Provider. A run that sends nothing, such as `--dry-run` or a Secret Guard block, stops earlier.

1. **CLI and config.** Parse the arguments, then load and merge the User Config, the Project Config and your flags.
2. **Input.** Read the text from arguments or stdin. Check the size limit (exit 7) and that there is text (exit 2).
3. **Target Language.** Take it from `:xx` or `-t`, a Language Pair, or `target`. None: exit 2.
4. **Secret Guard.** Block, redact or ask (exit 6 if blocked). `--dry-run` stops here and prints what would be sent.
5. **Provider adapter and Credentials.** Build the Provider. A configured `providers.<name>.endpoint` for it stops the run (exit 3). No key: exit 4.
6. **Protected Terms and Usage Budget.** Terms become placeholders. If you set a limit, the call's characters are reserved (exit 8 if they do not fit). With no limit set, the usage file is never touched.
7. **HTTPS.** The text and the key go to Google Cloud Translation or Anthropic, and to no other server.
8. **Restore and output.** Put the terms back, then print the translation, or one JSON object with `--json`.

**Workspace layout.** `crates/kun-core` is the logic, with no terminal I/O. `crates/kun` is the CLI adapter: the terminal, the prompts and the exit codes. More: [docs/architecture.md](docs/architecture.md), and the ADRs [0001 official Providers only](docs/adr/0001-official-providers-no-scraping.md), [0002 Rust](docs/adr/0002-rust.md) and [0003 untrusted Project Config](docs/adr/0003-project-config-is-untrusted.md).

## Privacy and security

`kun` is a personal CLI translator, not a vault. Its security work is in proportion to that.

> [!IMPORTANT]
> The text (after the Secret Guard and Protected Terms) and your API key go to the Provider you chose, over HTTPS. Anthropic also receives your `style` hint (if set), the languages and the model name. A Provider also sees ordinary network metadata, such as your IP address. `kun` contacts no other server. After that, the Provider's privacy terms apply, so do not use `kun` for text your organisation does not allow to be sent there. Details: [docs/privacy.md](docs/privacy.md). Use `--dry-run` to see the destination and the text.

- **Keys.** For each key, `kun` looks in the environment variable (for CI), then the OS keychain, then the `[credentials]` table of the User Config. On Unix that table is used only if the file is `0600` or stricter. Keys are never accepted as flags, and never printed.
- **No telemetry.** No analytics, no update check, no cache, no history.
- **Audit.** Findings: [docs/security/audit-2026-11.md](docs/security/audit-2026-11.md). **An AI model (Claude Opus 5.5) did the audit, not an independent security firm.** Treat it as useful evidence, not a guarantee. CI also runs cargo-deny, CodeQL and fuzzing, and the repository has an OpenSSF Scorecard (see the badge).
- **Report a vulnerability** privately through GitHub: see [SECURITY.md](SECURITY.md).

## Costs and limits

- **Prices are the Provider's.** `kun` is free. See [Google Cloud Translation pricing](https://cloud.google.com/translate/pricing) and [Anthropic pricing](https://www.anthropic.com/pricing). Free tiers change, so check them. Google Cloud Translation has offered 500,000 characters a month. Quota and billing links: [docs/usage.md](docs/usage.md#limits-quotas-and-costs).
- **Guards.** `max_chars` caps one input ([recipe 2](#2-translate-a-file-or-the-clipboard)). The Usage Budget caps a month ([recipe 7](#7-stay-inside-a-free-tier-usage-budget)). `kun` never retries on its own, because a retry could cost money twice (exit 8: [exit codes](#8-use-kun-in-a-script)).

## FAQ and troubleshooting

<details>
<summary>Common problems and fixes</summary>

| Problem | What to do |
|---|---|
| `kun: command not found` | `cargo install` puts `kun` in `~/.cargo/bin`. Add it to your `PATH`, then open a new shell. |
| Exit 4: no credentials | No key for the Provider in use. Google is the default: if you only logged in to Anthropic, add `--provider anthropic` or set `provider = "anthropic"`. Otherwise run `kun auth login`, or set `KUN_GOOGLE_API_KEY` or `ANTHROPIC_API_KEY`. `kun auth status` shows where each key comes from, never the key. |
| The Secret Guard blocked my text (exit 6) | See [recipe 6](#6-handle-a-secret-in-your-text). If it is a real secret, remove it. |
| `ignoring .kun.toml: owned by another user` | A warning, not an error: a `.kun.toml` owned by another user is skipped on purpose, like git's `safe.directory`. Read the file before you trust it. |
| `WARNING: 1 line is improperly formatted` when I verify a checksum | Harmless: the `.sha256` file ends with an extra blank line. Look for `OK`. |
| No keychain on Linux | `kun auth login` exits with 3 without Secret Service. Set the environment variable, or put the key in `[credentials]` in the User Config and run `chmod 600` on it. On Unix, `kun` ignores that table (and warns you) if group or others can read the file. |
| My text started with `auth` and failed | It is a subcommand: put `--` first. See [recipe 1](#1-translate-your-first-sentence). |
| Exit 8 | If the message says "monthly usage budget", it is your Usage Budget: raise it or wait for the reset. Otherwise the Provider limits you: a rate limit passes, so wait and retry. A quota, credit or billing limit needs your Provider account page. |

</details>

## Contributing

Contributions are welcome: bug reports, docs fixes, tests and code. Start with [CONTRIBUTING.md](CONTRIBUTING.md). The normal test suite uses no network, and the `live` tests that call real Providers are opt-in (`--features live`). Before a pull request, run:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --features translate-kun/fake-provider -- -D warnings
cargo test --workspace --features translate-kun/fake-provider
```

Please follow the [Code of Conduct](CODE_OF_CONDUCT.md). How the project was planned: [docs/development](docs/development/README.md).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.

Unless you explicitly state otherwise, any contribution you intentionally submit for inclusion in this project, as defined in the Apache-2.0 license, is dual licensed as above, without any additional terms or conditions.
