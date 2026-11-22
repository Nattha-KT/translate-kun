# translate-kun

**`kun` translates text in your terminal and in your pipes, and it checks that text for known API key and token patterns before it leaves your machine.**

[![CI](https://github.com/machngai/translate-kun/actions/workflows/ci.yml/badge.svg)](https://github.com/machngai/translate-kun/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/translate-kun)](https://crates.io/crates/translate-kun)
[![Latest release](https://img.shields.io/github/v/release/machngai/translate-kun)](https://github.com/machngai/translate-kun/releases/latest)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/machngai/translate-kun/badge)](https://scorecard.dev/viewer/?uri=github.com/machngai/translate-kun)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

`kun` is a command-line translator for developers. It is inspired by [Translate Shell](https://github.com/soimort/translate-shell), but it uses official, key-authenticated translation services instead of scraped web endpoints, so it does not break when a free endpoint changes. Thai ↔ English is a first-class use case.

> **Project status: pre-1.0.** The CLI works, but flags, config keys and output may still change between `0.x` releases. The `--json` output is versioned (`schema_version`) and changes only in a compatible way within a version.

## Contents

- [Why kun](#why-kun)
- [Demo](#demo)
- [Install](#install)
- [Quick start (5 minutes)](#quick-start-5-minutes)
- [Usage guide](#usage-guide)
- [Configuration at a glance](#configuration-at-a-glance)
- [How it works](#how-it-works)
- [Privacy and security](#privacy-and-security)
- [Costs and limits](#costs-and-limits)
- [FAQ and troubleshooting](#faq-and-troubleshooting)
- [Contributing](#contributing)
- [License](#license)

## Why kun

- **A translator for the terminal and for pipes.** Text comes from arguments, stdin or a file. The translation goes to stdout, and diagnostics go to stderr.
- **The Secret Guard checks your text first.** It looks for known patterns of API keys, tokens and private keys. By default it blocks what it finds, or asks you on a terminal. It is a safety net, not a guarantee.
- **Protected Terms stay exact.** Product and function names come back exactly as you wrote them.
- **Two Providers, and no silent fallback.** Choose Google Cloud Translation or Anthropic. If one fails, `kun` never sends your text to the other on its own.
- **A Usage Budget for free tiers.** Set a monthly character limit and `kun` refuses before it passes it. It is opt-in and ships in 0.2.0.
- **Made for scripts.** `--json` output and stable exit codes (0 to 8).
- **Verifiable releases.** Every release archive has a checksum and a build provenance attestation.

## Demo

This session assumes a Google key in the environment (`KUN_GOOGLE_API_KEY`), no Language Pair and no Usage Budget. The translated words come from the Provider, so your output may differ a little. The key in the examples is obviously fake.

```text
$ kun :th "hello"
provider: google, detected source: en
สวัสดี

$ echo "good morning" | kun :th
provider: google, detected source: en
สวัสดีตอนเช้า

$ pbpaste | kun :th | pbcopy          # translate the clipboard (macOS)

$ echo "my key is AKIAFAKEEXAMPLE12345 ok" | kun en:th
error: Secret Guard blocked the text; nothing was sent.
  - possible AWS access key at bytes 10..30
Use --redact to mask them, or --allow-secrets to send them anyway.

$ echo "my key is AKIAFAKEEXAMPLE12345 ok" | kun --dry-run --redact en:th
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

$ kun --json :th "hello"
{"schema_version":1,"text":"สวัสดี","source":"en","target":"th","provider":"google","warnings":[]}
```

The blocked text exits with code 6 and nothing is sent. On a terminal (stdin and stderr are both terminals), the Secret Guard asks instead. When `kun` is not interactive, such as in a pipe or a script, it blocks by default. `--redact` masks the match, and `--allow-secrets` sends the text anyway. The `warning:` line goes to stderr before the report.

## Install

**Homebrew** (macOS and Linux):

```sh
brew install machngai/tap/kun
```

**crates.io** (needs Rust stable, installed with [rustup](https://rustup.rs)):

```sh
cargo install translate-kun
```

**GitHub Releases**: download the archive for your system from the [Releases page](https://github.com/machngai/translate-kun/releases), unpack it and put `kun` on your `PATH`. Archives exist for macOS (Apple silicon and Intel), Linux (x86_64 and aarch64) and Windows (x86_64, best effort). Each archive also holds the licence files and `THIRD-PARTY-LICENSES.md`.

**From source**:

```sh
git clone https://github.com/machngai/translate-kun
cd translate-kun
cargo install --path crates/kun
```

All of these give you the `kun` command. Check it with `kun --version`.

### Verify your download

Every release archive has a SHA-256 checksum and a build provenance attestation, so you can check that the file was built by this repository's release workflow and was not changed.

The examples use the macOS Apple silicon archive: replace the file name with the one you downloaded.

```sh
# Provenance: needs the GitHub CLI (`gh`), signed in with `gh auth login`.
# --signer-workflow checks that the file was built by this repository's
# release workflow.
gh attestation verify translate-kun-aarch64-apple-darwin.tar.xz \
  -R machngai/translate-kun \
  --signer-workflow machngai/translate-kun/.github/workflows/release.yml

# Checksum: download the .sha256 file next to the archive
sha256sum -c translate-kun-aarch64-apple-darwin.tar.xz.sha256       # Linux
shasum -a 256 -c translate-kun-aarch64-apple-darwin.tar.xz.sha256   # macOS
```

Look for `OK`. A line such as `WARNING: 1 line is improperly formatted` is harmless: the `.sha256` file ends with an extra blank line.

Each release also has a software bill of materials (SBOM, CycloneDX), and the dependency list is embedded in the release binaries: `cargo install cargo-audit --locked`, then `cargo audit bin "$(command -v kun)"`. See [SECURITY.md](SECURITY.md#security-assurance).

## Quick start (5 minutes)

**1. Get an API key.** You need a key for one Provider. Your own key means your own account and your own bill.

- **Google Cloud Translation** (the default). Follow Google's [setup guide](https://docs.cloud.google.com/translate/docs/setup): enable the Cloud Translation API for a project, and [enable billing](https://docs.cloud.google.com/translate/docs/setup#enable_billing) on that project, because the API does not work without it. Then create an API key in the [Credentials page](https://console.cloud.google.com/apis/credentials).
- **Anthropic.** Create a key in the [Anthropic console](https://console.anthropic.com/settings/keys).

Google is the default Provider. If you only have an Anthropic key, say so on every run with `--provider anthropic`, or set `provider = "anthropic"` in your User Config (see step 4).

**2. Store the key.** `kun` asks for it with a hidden prompt and saves it in your OS keychain. Keys are never accepted as command-line flags, because flags end up in your shell history.

```sh
kun auth login google        # or: kun auth login anthropic
```

**3. Translate something.** `:th` means "Target Language: Thai".

```sh
kun :th "hello"                         # Google, the default
kun --provider anthropic :th "hello"    # if you logged in to Anthropic only
```

**4. Set a Language Pair.** A Language Pair lets `kun` pick the direction for you: English becomes Thai, and Thai becomes English. Put it in your User Config, `~/.config/kun/config.toml` (or `$XDG_CONFIG_HOME/kun/config.toml` when that is an absolute path):

```toml
pair = ["en", "th"]
# provider = "anthropic"  # uncomment if Anthropic is your Provider (Google is the default)
```

This file does not exist until you create it, or until `kun budget set` creates it for you. Without a Language Pair, a `target` or a `:xx`, `kun` has no Target Language and exits with 2. With the pair above you can leave out the language:

```sh
kun "hello"      # en -> th
kun "สวัสดี"       # th -> en
```

For CI and cron, set `KUN_GOOGLE_API_KEY` or `ANTHROPIC_API_KEY` in the environment instead of using the keychain.

## Usage guide

Each part below is short. [docs/usage.md](docs/usage.md) lists every command and flag.

### Languages

```sh
kun :th "hello"             # Target Language th, Source Language detected
kun en:th "hello"           # Source Language en, Target Language th
kun -s en -t th "hello"     # long form, good for scripts
kun :th I love it           # translates "I love it" into Thai
```

A language is recognised only as the first argument written `:xx` or `xx:yy`, or with `-s` / `-t`. **A bare word is always text**, so `kun :th I love it` never reads `it` as Italian. Codes look like `th` or `zh-TW`, and case does not matter. Without a Language Pair or a `target` in a config file, you must give the Target Language yourself, or `kun` exits with 2.

`auth`, `budget`, `completions`, `help` and `init` are subcommands. If your text starts with one of them, put `--` before it:

```sh
kun :th -- auth is great
```

### Input and output

- **Sensitive text goes through stdin.** Text given as arguments is saved in your shell history, and other local users can see it in `ps` while `kun` runs.
- **Pipes and files:**

  ```sh
  echo "good morning" | kun :th
  kun :th < notes.md > notes.th.md
  ```

- **The clipboard (macOS):** `pbpaste | kun :th | pbcopy`.
- If you give both text arguments and piped stdin, the arguments win and `kun` warns you on stderr.
- The default size limit is 10,000 characters. Longer input exits with 7. Raise it with `--max-chars` or `max_chars`.

### Language Pair

Set `pair = ["en", "th"]` and `kun` picks the direction from the detected Source Language. Text in the first language goes to the second, text in the second goes to the first, and any other language goes to the first. An explicit `:xx` or `-t xx` always wins.

A Language Pair with a detected Source Language can send your text to the Provider twice (once per direction). `--dry-run` tells you when that can happen.

### Protected Terms and style

Names that must not change go in `protected_terms`:

```toml
protected_terms = ["translate-kun", "Cargo.toml"]
```

`kun` swaps each term for a placeholder before sending and puts it back afterwards. Matching is case-sensitive. If a Provider loses a placeholder, you still get the translation and a warning.

`style` is a tone hint such as `style = "casual, developer tone"`. Only the Anthropic Provider uses it. Google ignores it.

### Choosing a Provider

```sh
kun --provider anthropic :th "hello"
```

You can also set `provider = "anthropic"` in a config file. There is **no automatic fallback**: if a Provider fails, `kun` exits with an error, and some errors suggest trying `--provider`. It never sends your text to a second company on its own.

### Secret Guard

The Secret Guard scans your text before it is sent. It detects known patterns: AWS access key IDs (and AWS secret access keys, only when written as an assignment such as `aws_secret_access_key = …`), GitHub and GitLab tokens, Anthropic, OpenAI and Google API keys, Slack tokens, Stripe live keys, npm and Hugging Face tokens, private key blocks and JWTs, plus any `extra_patterns` you add.

| Situation | What happens |
|---|---|
| Nothing found | The text is sent. |
| Found, on a terminal (stdin and stderr are both terminals) | `kun` asks `Send anyway? [y/N]`. The default is no. |
| Found, not interactive (pipe, script, cron) | **Blocked** with exit 6. Nothing is sent. |
| `--redact` | Each match becomes `[REDACTED]`, and the rest is sent. |
| `--allow-secrets` | The text is sent as it is, unless a redact is forced (`--redact` or `force_redact`), which wins. |
| `force_redact = true` | Same as `--redact` every time. |

```toml
[secret_guard]
force_redact = true
extra_patterns = ["internal-[a-z0-9]{16}"]
```

The error message shows the kind of secret and its byte range, never the secret itself. The Secret Guard uses patterns, so it is a safety net and not a guarantee.

### Dry run

`--dry-run` shows where your text would go and the exact text that would be sent, and it sends nothing. It reports the Provider and host (and the model for Anthropic), the languages, the credential source (never the key), the Secret Guard decision, and the text after redaction and Protected Terms placeholders. The [Demo](#demo) shows one. It does not list everything a request carries: Anthropic also receives your `style` hint. A dry run never builds a network client, and it never counts against your Usage Budget.

### Usage Budget

> **Ships in 0.2.0.** `kun budget` is on the `main` branch, but the published 0.1.x releases do not have it. If `kun --version` shows 0.1.x, build from source to try it.

A Usage Budget is a monthly limit on the characters `kun` sends to one Provider. It is off by default.

```sh
kun budget set google 450000     # stay under Google's 500,000-character free tier
kun budget                       # show limits, usage and the reset date
kun budget unset google          # remove the limit
```

```text
$ kun budget status
Usage budget for 2026-11 (resets on 2026-12-01, UTC):
google: 448210 of 450000 characters used, 1790 left
anthropic: no limit, 0 characters used
```

- **What it counts.** The characters of the text actually sent in each Provider call, after redaction and Protected Terms. `kun` reserves them before each call and gives them back if the call fails. If giving them back fails, the count stays too high and `kun` warns you.
- **A Language Pair can make two calls.** Each call reserves on its own. If the second call is refused, the first one stays counted.
- **When it stops you.** If the next call would pass the limit, `kun` refuses before any network call and exits with 8. No flag overrides this. Raise the limit or wait for the 1st of the month (UTC).
- **Where it lives.** The limit is in the `[budget]` table of the User Config. The counters are in `$XDG_STATE_HOME/kun/usage.json` (when that is an absolute path), otherwise `~/.local/state/kun/usage.json`. The file holds the month and one counter per Provider, and no text. If a budget is set and the file cannot be read or saved, `kun` sends nothing and exits with 3.
- **It is approximate and local.** It counts only what this machine sends through `kun`, and two runs at the same moment can lose an increment, so the count can drift. Pair it with a quota cap or budget alert in your Provider's console, because only the Provider enforces its own limits. Anthropic bills tokens, not characters, so a character budget is only a rough guide there.

### Project Config

`kun init` writes a commented `.kun.toml` in the current directory, so a team can share a Language Pair, Protected Terms or extra Secret Guard patterns:

```sh
kun init            # fails if .kun.toml already exists
kun init --force    # replace it
```

A Project Config is **untrusted**, because it may come from a repository you just cloned ([ADR 0003](docs/adr/0003-project-config-is-untrusted.md)):

- It can choose a Provider, a Language Pair, Protected Terms and limits.
- It can only make the Secret Guard stricter. It can never turn on `allow_secrets`.
- It **cannot** hold credentials, endpoints or a `[budget]`. These are errors (exit 3).
- On Unix, a `.kun.toml` owned by another user is skipped with a warning.

### Shell completions

```sh
mkdir -p ~/.local/share/bash-completion/completions
kun completions bash > ~/.local/share/bash-completion/completions/kun
```

Shells: `bash`, `zsh`, `fish`, `powershell` and `elvish`. [docs/usage.md](docs/usage.md#kun-completions) has the install line for each.

### Scripting

`--json` prints one object on stdout. Errors are never JSON: they go to stderr and stdout stays empty.

```sh
kun --json :th "hello" | jq -r .text
```

The fields are listed in [docs/json-output.md](docs/json-output.md). Exit codes:

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

## Configuration at a glance

`kun` reads two optional TOML files:

| File | Location | Purpose |
|---|---|---|
| User Config | `~/.config/kun/config.toml` (or `$XDG_CONFIG_HOME/kun/config.toml` when that is an absolute path) | Personal settings. The only place for credentials and endpoints. |
| Project Config | `.kun.toml`, found by walking up from the current directory | Shared settings for a project. Untrusted. |

Precedence: command-line flag > Project Config > User Config > built-in defaults. This User Config uses the main keys:

```toml
# ~/.config/kun/config.toml
provider = "google"                 # or "anthropic"
pair = ["en", "th"]                 # Language Pair
protected_terms = ["translate-kun"]
max_chars = 10000                   # longest accepted input, in characters

[secret_guard]
force_redact = false                # true = always redact, never ask
extra_patterns = []                 # your own regexes

[budget]                            # User Config only; kun budget set writes this
google_chars_per_month = 450000
```

Every key, its default and where it is allowed: [docs/configuration.md](docs/configuration.md).

## How it works

[![How one translation runs: kun CLI, Secret Guard, Protected Terms, Usage Budget, Provider adapter, then HTTPS to Google or Anthropic](docs/assets/architecture.svg)](docs/assets/architecture.svg)

*Click the diagram to open it full size. It is animated with CSS only, and shows a still image if your system asks for reduced motion.*

The diagram shows the path of a request that reaches a Provider. A run that sends nothing, such as `--dry-run` or a Secret Guard block, stops earlier. One translation, `kun :th "hello"`, goes through these steps:

1. **CLI and config.** `kun` parses the arguments and the language, such as `:th`. It loads the User Config and the Project Config, and merges them with your flags. The Project Config is untrusted.
2. **Input.** It reads the text from arguments or stdin, and checks the size limit (exit 7) and that there is text (exit 2).
3. **Target Language.** It picks the Target Language from `:xx` or `-t`, a Language Pair, or `target`. If there is none, it exits with 2 before anything else happens.
4. **Secret Guard.** It scans the text. It blocks it, redacts it or asks you (exit 6 if blocked). `--dry-run` stops here and prints what would be sent.
5. **Provider adapter and Credentials.** `kun` builds the Provider you chose. A configured `providers.<name>.endpoint` for that Provider stops the run with exit 3. For each key, `kun` looks in the environment variable first, then the OS keychain, then the `[credentials]` table of the User Config (on Unix, only if that file is `0600` or stricter). No key means exit 4.
6. **Protected Terms and Usage Budget.** Your terms become placeholders. Then, if you set a limit, the Usage Budget reserves the characters of this call. If they do not fit, `kun` stops (exit 8). With no limit set, nothing is reserved and the usage file is never touched.
7. **HTTPS.** The text and the key go to Google Cloud Translation or Anthropic. Anthropic also gets your `style` hint, the languages and the model name. `kun` contacts no other server.
8. **Restore and output.** `kun` puts the Protected Terms back, then prints the translation on stdout, or one JSON object with `--json`.

**Workspace layout.** `kun` is two crates. `crates/kun-core` holds the logic and has no terminal I/O. `crates/kun` is the CLI adapter: it owns the terminal, the prompts and the exit codes. This keeps the logic easy to test and lets another adapter reuse it.

More: [docs/architecture.md](docs/architecture.md) for contributors, and the decisions in the ADRs: [0001 official Providers only](docs/adr/0001-official-providers-no-scraping.md), [0002 Rust](docs/adr/0002-rust.md) and [0003 untrusted Project Config](docs/adr/0003-project-config-is-untrusted.md).

## Privacy and security

`kun` is a personal CLI translator, not a vault. Its security work is in proportion to that.

- **What leaves your machine.** The text to translate (after the Secret Guard and Protected Terms) and your API key go to the Provider you chose, over HTTPS. Anthropic also receives your `style` hint if you set one, the languages and the model name. A Provider also sees ordinary network metadata, such as your IP address. `kun` contacts no other server. Once the text reaches the Provider, its privacy terms apply. Do not use `kun` for text that your organisation does not allow to be sent there. See [docs/privacy.md](docs/privacy.md), and use `kun --dry-run` to see the destination and the text that would be sent.
- **Where keys are stored.** In the OS keychain (`kun auth login`), or in an environment variable for CI. On a machine with no keychain, a User Config `[credentials]` table works as a fallback. On Unix it is used only if the file is `0600` or stricter. Keys are never accepted as flags, and never printed.
- **No telemetry.** There is no analytics, no update check, no cache and no history.
- **The audit.** The code had a security audit, and the findings are in [docs/security/audit-2026-11.md](docs/security/audit-2026-11.md). **The audit was done by an AI model (Claude Opus 5.5), not by an independent security firm.** Treat it as useful evidence, not a guarantee.
- **Continuous checks.** CI runs cargo-deny, CodeQL and fuzzing, and the repository has an OpenSSF Scorecard (see the badge above).
- **Report a vulnerability** privately, through GitHub's private reporting. The steps are in [SECURITY.md](SECURITY.md).

## Costs and limits

- **Provider prices are the Provider's.** `kun` is free, and you pay the Provider you use. See [Google Cloud Translation pricing](https://cloud.google.com/translate/pricing) and [Anthropic pricing](https://www.anthropic.com/pricing). Free tiers exist, but they change, so check the current terms. Google Cloud Translation has offered 500,000 characters a month.
- **`max_chars` limits the size of one input.** The default is 10,000 characters. Longer text exits with 7 before anything is sent. It is not a limit on what a run sends: a Language Pair can send the text twice.
- **The Usage Budget limits a month** (ships in 0.2.0): `kun budget set google 450000`. It is local and approximate, so also set a quota cap or budget alert in the Provider's console.
- **Exit 8 has two causes.** Either the Provider rate limited you or its quota, credit or billing limit was reached (wait, or check your Provider account), or your local Usage Budget was reached (raise it, or wait for the reset). `kun` does not retry on its own, because a retry could cost money twice.

[docs/usage.md](docs/usage.md#limits-quotas-and-costs) has the links to each Provider's quota and billing pages.

## FAQ and troubleshooting

**`kun: command not found`**
After `cargo install`, `kun` lives in `~/.cargo/bin`. Add that folder to your `PATH`, then open a new shell. `kun --version` should work.

**Exit 4: no credentials**
`kun` has no API key for the Provider in use. Google is the default, so if you only logged in to Anthropic, add `--provider anthropic` or set `provider = "anthropic"`. Otherwise run `kun auth login google` (or `anthropic`), or set `KUN_GOOGLE_API_KEY` or `ANTHROPIC_API_KEY`. `kun auth status` shows where each key comes from, and never prints a key.

**The Secret Guard blocked my text (exit 6)**
The message names the kind of secret and its byte range. If it is a real secret, remove it. If you accept the risk, use `--redact` to mask it or `--allow-secrets` to send it. Use `--dry-run` to see what would be sent.

**`ignoring .kun.toml: owned by another user`**
A `.kun.toml` owned by a different user is skipped on purpose, like git's `safe.directory`. This is a warning, not an error. If the file is yours, check its owner. If it is not, read it before you trust it.

**`WARNING: 1 line is improperly formatted` when I verify a checksum**
This is harmless. The `.sha256` file ends with an extra blank line. Look for `OK`.

**No keychain on Linux**
`kun auth login` exits with 3 where there is no Secret Service. Set `KUN_GOOGLE_API_KEY` or `ANTHROPIC_API_KEY`, or add the key to the `[credentials]` table of the User Config and run `chmod 600` on that file. On Unix, `kun` ignores the table (and warns you) if group or others can read the file.

**My text started with `auth` and failed**
`auth`, `budget`, `completions`, `help` and `init` are subcommands. Put `--` before text that starts with one: `kun :th -- auth is great`.

**Exit 8**
If the message says "monthly usage budget", it is your local Usage Budget: raise it with `kun budget set` or wait for the reset. Otherwise the Provider is limiting you. A rate limit passes on its own, so wait and try again. A quota, credit or billing limit needs a visit to your Provider account page.

## Contributing

Contributions are welcome: bug reports, docs fixes, tests and code. Start with [CONTRIBUTING.md](CONTRIBUTING.md). The normal test suite uses no network, and the `live` tests that call real Providers are opt-in (`--features live`). Before you open a pull request, run:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --features translate-kun/fake-provider -- -D warnings
cargo test --workspace --features translate-kun/fake-provider
```

Everyone taking part is expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md). The history of how the project was planned is in [docs/development](docs/development/README.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution you intentionally submit for inclusion in this project, as defined in the Apache-2.0 license, is dual licensed as above, without any additional terms or conditions.
