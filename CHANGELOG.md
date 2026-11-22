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
- Stable exit codes 0 to 8. Exit 8 means "rate limited or quota exceeded": a Google quota or billing error and an Anthropic credit-balance error are no longer reported as a rejected key or a plain HTTP status. A `Retry-After` header (seconds form) is shown in the message. Scripts can retry on 8.
- A translation that does not fit in one request now fails with a clear message (exit 5) instead of "response could not be understood". The Anthropic output limit is raised from 4,096 to 16,000 tokens, so long English-to-Thai input works.
- `--dry-run`: runs the whole pipeline up to the point of sending, prints the host, the model, the credential source, the Secret Guard decision and the exact text that would be sent, and sends nothing. `--dry-run --json` returns one object (`dry_run: true`, `schema_version` 1).

**Providers**

- Google Cloud Translation (default) and Anthropic (default model `claude-haiku-5-5`), chosen with `--provider` or `provider` in a config file.
- No automatic fallback between Providers.
- `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY` and `NO_PROXY` are honoured.
- A `Provider` trait in `kun-core` for adding more Providers, and an offline fake Provider for tests.

**Configuration**

- Project Config (`.kun.toml`, created by `kun init`, found by walking up from the current directory) and User Config (`~/.config/kun/config.toml`).
- Precedence: command-line flag, Project Config, User Config, built-in defaults.
- Project Config is untrusted: it may not hold credentials, endpoints or `allow_secrets`, and may only make the Secret Guard stricter (ADR 0003).
- `protected_terms` is applied to every translation, including the Language Pair path. A lost term is a warning, not an error. `style` is passed to the Anthropic Provider (Google ignores it).
- `kun completions <bash|zsh|fish|powershell|elvish>` prints a shell completion script.

**Credentials**

- `kun auth login | logout | status` with hidden key input and storage in the OS keychain.
- Lookup order: environment variable (`KUN_GOOGLE_API_KEY`, `ANTHROPIC_API_KEY`), OS keychain, then the User Config `[credentials]` table.
- The `[credentials]` table is used only when the User Config file is `0600` or stricter.
- API keys are never accepted as flags and never appear in output, errors or `--json`.

**Secret Guard**

- Built-in patterns for AWS access key IDs and secret access keys, GitHub and GitLab tokens, Anthropic, OpenAI and Google API keys, Slack tokens, Stripe live keys, npm and Hugging Face tokens, private key blocks and JWTs, plus extra patterns from config.
- Prompt on a terminal, block in scripts (exit 6), `--redact` to mask, `--allow-secrets` to override.

**Project**

- Dual licence: MIT OR Apache-2.0.
- Documentation: README, usage, configuration and architecture guides, contributing guide, code of conduct and security policy.
- Issue forms and a pull request template.
- CI on Linux and macOS (format, clippy, tests).

**Release tooling (S8)**

- `cargo-dist` builds release archives for macOS (Apple silicon, Intel), Linux (x86_64, aarch64) and Windows (x86_64, best effort) with SHA-256 checksums (`.github/workflows/release.yml`). Each archive holds `kun`, the licence files, `NOTICE`, `README.md` and `THIRD-PARTY-LICENSES.md` (made with `cargo-about`).
- Verifiable artefacts: build provenance attestations (`gh attestation verify`), an SBOM (CycloneDX), and the dependency list embedded in the binary (`cargo-auditable`, `cargo audit bin`).
- Homebrew tap: `brew install machngai/tap/kun`. Pre-releases do not update the tap.
- crates.io: `cargo install translate-kun`. A workflow publishes `translate-kun-core` and then `translate-kun` through crates.io Trusted Publishing when a non-pre-release GitHub Release is published (`.github/workflows/publish-crates.yml`).
- Publishing metadata for both crates, and a maintainer checklist in `docs/development/releasing.md`.

### Fixed

- Windows: the test job now gates CI. On Windows the "ignoring stdin" warning (also in the `--json` `warnings`) still appears whenever stdin is redirected, even from an empty file or `NUL`: std cannot tell a pipe from an empty file there without `unsafe`, and guessing wrong would silently drop piped text. On Unix, `/dev/null` and an empty file on stdin still give no warning.

### Added (development)

- Property-based tests (`proptest`) for the Secret Guard and Protected Terms. A dev-dependency only: nothing is added to the shipped binary.
- CI: a `coverage` report (`cargo-llvm-cov`, report only, not required), `cargo-machete` (unused dependencies) and `typos` (spell check). The Windows test job now gates the workflow, and all `dtolnay/rust-toolchain` pins use the same `master` SHA.
- A one-off mutation testing run (`cargo-mutants`) on the Secret Guard, with new tests for the mutants that survived. See `CONTRIBUTING.md`.

### Security

Fixes from the pre-publish security audit:

- Secret Guard: new pattern for the AWS secret access key in assignment form (`aws_secret_access_key = …`), so `--redact` hides both halves of an AWS key pair.
- Secret Guard: keys right after `_` or a JSON-escaped `\n` / `\t` are no longer missed.
- Secret Guard: new patterns for Stripe live keys, GitLab personal access tokens, npm tokens and Hugging Face tokens.
- A `providers.<name>.endpoint` for the Provider in use now fails with exit 3 instead of being silently ignored (which sent the text to the public API).
- On a terminal, control characters other than newline and tab are stripped from the translation, so a prompt-injected model cannot emit escape sequences. Pipes and `--json` are unchanged.
- The Anthropic Provider's `Debug` output no longer shows the model name.
- Docs: text given as arguments lands in shell history and `ps` (use stdin for sensitive text); all proxy variables are honoured and HTTPS always goes through `CONNECT`; the Secret Guard is the Adapter's job, not `kun-core`'s.

Pre-publish hardening (S15):

- Every GitHub Action in the workflows is pinned by full commit SHA, with a version comment. Dependabot keeps the pins current.
- A discovered Project Config (`.kun.toml`) owned by a different user is skipped with a warning instead of loaded (Unix only, like git's `safe.directory`).
- CodeQL static analysis (SAST) for the Rust code runs on push and pull requests to `main`, and weekly.
- `.github/CODEOWNERS` names the maintainer as owner of the whole repository and of the security-sensitive paths.

### Documentation

- README: new "Where your text goes" note. Your text, after the Secret Guard, goes to the Provider you chose and is handled under that Provider's terms. `docs/privacy.md` says the same.

[Unreleased]: https://github.com/machngai/translate-kun/commits/main
