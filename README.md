# translate-kun

[![CI](https://github.com/machngai/translate-kun/actions/workflows/ci.yml/badge.svg)](https://github.com/machngai/translate-kun/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/machngai/translate-kun/badge)](https://scorecard.dev/viewer/?uri=github.com/machngai/translate-kun)

`kun` is a command-line translator for developers who work in the terminal. It is inspired by [Translate Shell](https://github.com/soimort/translate-shell), but it uses official, key-authenticated translation services instead of scraped web endpoints, so it does not break when a free endpoint changes. It reads text from arguments or stdin, detects the source language, and prints the translation on stdout, which makes it easy to use in pipes and scripts. Thai ↔ English is a first-class use case.

> **Project status: pre-1.0.** The CLI works, but flags, config keys and output may still change between `0.x` releases. The `--json` output is versioned (`schema_version`) and changes only in a compatible way within a version.

## Features

- Translate text from arguments or stdin: `kun :th "hello"`, `echo hello | kun :th`.
- Automatic Source Language detection.
- **Language Pair**: configure two languages (for example `en` and `th`) and `kun` picks the direction from the detected Source Language.
- Two Providers: Google Cloud Translation (default) and Anthropic. No automatic fallback: text never goes to a second company on its own.
- **Secret Guard**: scans outgoing text for API keys, tokens and private keys before anything is sent.
- API keys are stored in the OS keychain (`kun auth login`), or read from environment variables for CI. Keys are never accepted as command-line flags.
- Project Config (`.kun.toml`, created by `kun init`) and User Config, with strict trust rules for the project file.
- `--json` output for scripts and editor integrations.
- Stable exit codes. Translation on stdout, diagnostics on stderr.
- No telemetry, no cache, no history. The only network traffic is to the Provider you choose.

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

```sh
# Provenance: needs the GitHub CLI (`gh`)
gh attestation verify <file> -R machngai/translate-kun

# Checksum: download <file>.sha256 next to the archive
sha256sum -c <file>.sha256          # Linux
shasum -a 256 -c <file>.sha256      # macOS
```

Each release also has a software bill of materials (SBOM, CycloneDX), and the dependency list is embedded in the binary: `cargo install cargo-audit --locked`, then `cargo audit bin $(which kun)`. See [SECURITY.md](SECURITY.md#security-assurance).

## Quick start

```sh
# 1. Store an API key (hidden prompt; saved in the OS keychain)
kun auth login google

# 2. Translate. ":th" means "Target Language: Thai"
kun :th "hello"

# Source and Target Language
kun en:th "hello"

# Pipe text in
echo "good morning" | kun :th

# Translate a file
kun :th < notes.md > notes.th.md

# Machine-readable output
kun --json :th "hello"
# {"schema_version":1,"text":"สวัสดี","source":"en","target":"th","provider":"google","warnings":[]}

# Use the other Provider
kun --provider anthropic :th "hello"
```

For CI, set `KUN_GOOGLE_API_KEY` or `ANTHROPIC_API_KEY` in the environment instead of using the keychain.

A bare word is always text: `kun I love it` translates "I love it". A language is only recognised as `:xx` or `xx:yy` in the first position, or with `-s` / `-t`.

See [docs/usage.md](docs/usage.md) for every command and flag.

### Where your text goes

After the Secret Guard has checked it, your text is sent to the Provider you chose (Google Cloud Translation or Anthropic). From that point it is handled under that Provider's terms and privacy policy. Do not use `kun` for text that your organisation does not allow to be sent to that Provider. See [docs/privacy.md](docs/privacy.md) for exactly what leaves your machine.

## Configuration

`kun` reads two optional TOML files:

| File | Location | Purpose |
|---|---|---|
| Project Config | `.kun.toml`, found by walking up from the current directory | Shared settings for a project. Treated as **untrusted**. |
| User Config | `~/.config/kun/config.toml` (or `$XDG_CONFIG_HOME/kun/config.toml`) | Personal settings. The only place for credentials and endpoints. |

Precedence: command-line flag > Project Config > User Config > built-in defaults. Run `kun init` to create a commented `.kun.toml`.

Details, every key and its default: [docs/configuration.md](docs/configuration.md).

## Security model

`kun` is a personal CLI translator, not a vault. Its security work is in proportion to that.

**What `kun` protects against**

- Your API keys leaking to third parties, logs, terminal output, shell history or other local users. Keys are never accepted as flags, never printed, and never included in `--json` output or error messages. A key in a User Config file is used only if the file is `0600`.
- Your text leaking to a service you did not choose. There is no automatic Provider fallback, no telemetry, no cache and no history.
- Secrets in the text you translate. The Secret Guard checks the text before it leaves your machine. On a terminal it asks; in a script it blocks.
- A malicious repository. A cloned `.kun.toml` cannot hold credentials, cannot point `kun` at another endpoint, and cannot turn off the Secret Guard ([ADR 0003](docs/adr/0003-project-config-is-untrusted.md)).

**Out of scope**

- Malware already running as your user, a hostile operating system, debugger or memory access, and timing side channels.
- The Secret Guard is a safety net, not a guarantee. It uses patterns, so it can miss a secret that has an unusual shape.
- What the Provider does with your text. Read the privacy terms of the Provider you use.

**Trust, and how to check it yourself**

- [Privacy](docs/privacy.md): exactly what data leaves your machine (your text and your API key, to the Provider you chose, and nothing else), and how to verify it.
- [Security](SECURITY.md): threat model, how to report a vulnerability, and the continuous checks (cargo-deny, Dependabot, fuzzing, OpenSSF Scorecard).
- [Security audit](docs/security/audit-2026-11.md): the findings and their status. **The audit was performed by an AI model (Claude Opus 5.5), not by an independent professional security firm, and the project is pre-1.0 personal software.**

To report a vulnerability, see [SECURITY.md](SECURITY.md).

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Unexpected error (including a keychain read or write failure) |
| 2 | Usage error: bad arguments, no text, or no Target Language |
| 3 | Configuration error, including a credential in a Project Config, or no OS keychain for `kun auth login` |
| 4 | Missing credentials |
| 5 | Network or Provider error |
| 6 | Blocked by the Secret Guard (or you answered "no" at its prompt) |
| 7 | Input longer than the size limit (10,000 characters by default) |
| 8 | Rate limited or quota exceeded: waiting, or checking your Provider account, may help (a script can retry on 8) |

## Contributing

Contributions are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md). For how the code is organised, see [docs/architecture.md](docs/architecture.md). Everyone taking part is expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md). The history of how the project was planned is in [docs/development](docs/development/README.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution you intentionally submit for inclusion in this project, as defined in the Apache-2.0 license, is dual licensed as above, without any additional terms or conditions.
