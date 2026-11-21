# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Until 1.0, minor versions may contain breaking changes.

## [Unreleased]

First public version, planned as 0.1.0.

### Added

**Translation**

- `kun` command: translate text from arguments or stdin.
- Language syntax `:xx`, `xx:yy`, and `-s` / `-t`. A bare word is always text.
- Automatic Source Language detection.
- Language Pair (`pair = ["en", "th"]`): the direction follows the detected Source Language.
- Input limit of 10,000 characters by default (`--max-chars`, `max_chars`) and a 15-second Provider timeout (`timeout_secs`).
- Translation on stdout, diagnostics and warnings on stderr.
- `--json` output with a versioned schema (`schema_version` 1). See `docs/json-output.md`.
- Stable exit codes 0 to 7.

**Providers**

- Google Cloud Translation (default) and Anthropic (default model `claude-haiku-5-5`), chosen with `--provider` or `provider` in a config file.
- No automatic fallback between Providers.
- `HTTPS_PROXY` and `NO_PROXY` are honoured.
- A `Provider` trait in `kun-core` for adding more Providers, and an offline fake Provider for tests.

**Configuration**

- Project Config (`.kun.toml`, created by `kun init`, found by walking up from the current directory) and User Config (`~/.config/kun/config.toml`).
- Precedence: command-line flag, Project Config, User Config, built-in defaults.
- Project Config is untrusted: it may not hold credentials, endpoints or `allow_secrets`, and may only make the Secret Guard stricter (ADR 0003).
- Protected Terms and `style` settings are parsed and validated. The logic is in `kun-core`; the CLI does not apply them yet.

**Credentials**

- `kun auth login | logout | status` with hidden key input and storage in the OS keychain.
- Lookup order: environment variable (`KUN_GOOGLE_API_KEY`, `ANTHROPIC_API_KEY`), OS keychain, then the User Config `[credentials]` table.
- The `[credentials]` table is used only when the User Config file is `0600` or stricter.
- API keys are never accepted as flags and never appear in output, errors or `--json`.

**Secret Guard**

- Built-in patterns for AWS access keys, GitHub tokens, Anthropic, OpenAI and Google API keys, Slack tokens, private key blocks and JWTs, plus extra patterns from config.
- Prompt on a terminal, block in scripts (exit 6), `--redact` to mask, `--allow-secrets` to override.

**Project**

- Dual licence: MIT OR Apache-2.0.
- Documentation: README, usage, configuration and architecture guides, contributing guide, code of conduct and security policy.
- Issue forms and a pull request template.
- CI on Linux and macOS (format, clippy, tests).

[Unreleased]: https://github.com/Nattha-KT/translate-kun/commits/main
