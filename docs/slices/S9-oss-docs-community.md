# S9 — Open-source docs and community files

## Goal

Make the repository welcoming and self-explanatory for users and contributors anywhere in the world, following widely recognised open-source conventions. Documentation only: no Rust code changes.

## Read first

- `README.md`, `docs/requirements.md`, `docs/json-output.md`, `docs/adr/*`, `GLOSSARY.md`
- `crates/kun/src/args.rs`, `init.rs`, `auth.rs` and `error.rs`, to describe real behaviour, flags and exit codes accurately. Read them; do not edit them.
- `AGENTS.md` (rules only; **do not edit it**, the coordinator updates it after merge)

## In scope (create or edit)

- `README.md`: full rewrite, in English. Include:
  - a one-paragraph pitch
  - a feature list
  - install (`cargo install translate-kun` once published; `cargo install --path crates/kun` from source)
  - quick start (`kun auth login google`, `kun :th "hello"`, piping, `--json`)
  - a short configuration overview with links
  - a "Security model" section: what is protected and what is out of scope (summarise AGENTS.md "Security in proportion" for users)
  - an exit-code table
  - a "Project status: pre-1.0" note
  - contributing and licence sections

  Keep it scannable. Badges are allowed (CI and licence) but use the placeholder repository `Nattha-KT/translate-kun`.
- `docs/usage.md`: every command and flag with examples. Cover Language Pair, Protected Terms, the Secret Guard behaviour table, and stdin rules.
- `docs/configuration.md`: Project Config vs User Config, precedence, every key with type and default, which keys are User-Config-only and why (link ADR 0003), `[credentials]` and the 0600 rule.
- `docs/architecture.md`, for contributors:
  - the crate layout (`kun` CLI Adapter vs `kun-core`)
  - module map
  - the lifecycle of one request
  - the `Provider` trait and how to add a new Provider (step by step)
  - testing strategy (fake Provider, the `live` feature, no network in tests)
- `CONTRIBUTING.md`:
  - prerequisites (Rust stable via rustup)
  - build, test and lint commands (copy the exact Definition of Done commands from AGENTS.md)
  - branch and Conventional Commits style
  - the PR checklist
  - how to add a Provider (link to architecture.md)
  - "good first issue" guidance
  - a note that AI-assisted contributions follow AGENTS.md
- `CODE_OF_CONDUCT.md`: Contributor Covenant 2.1. For the enforcement contact, refer to the maintainer via GitHub (`@Nattha-KT`, written in backticks) or GitHub's private reporting. **Do not put any email address anywhere.**
- `SECURITY.md`:
  - supported versions (0.x latest only)
  - report vulnerabilities through GitHub private vulnerability reporting (Security tab → "Report a vulnerability"), never in public issues
  - expected response time ("best effort, personal project")
  - the threat model, briefly
- `CHANGELOG.md`: Keep a Changelog format, with an `## [Unreleased]` section summarising v0.1.0 features by area.
- `.github/ISSUE_TEMPLATE/bug_report.yml`, `feature_request.yml`, `config.yml` (issue forms). The bug form asks for the `kun --version` output, OS, Provider, and the steps. It must say in bold: **never paste API keys or private text**.
- `.github/PULL_REQUEST_TEMPLATE.md`: summary, linked issue, and a checklist (fmt, clippy, tests, docs updated, CHANGELOG entry, no secrets in fixtures).
- `.editorconfig`: UTF-8, LF, final newline, 4 spaces for `.rs`/`.toml`, 2 for `.yml`/`.md`.
- **Move** `docs/slices/` → `docs/development/slices/` with `git mv`.
- Add `docs/development/README.md`, explaining that the project was planned with requirement interviews, ADRs and slice specs, built with AI agents under the rules in AGENTS.md, and where each artefact lives.
- Fix every link that pointed to `docs/slices/` in the files you own. List any links in files you do not own (for example AGENTS.md) in your report.

## Out of scope

- Any `.rs` file, `Cargo.toml`, `.github/workflows/*` and `dependabot.yml` (S10 owns these), `AGENTS.md` and `CLAUDE.md`.

## Rules

- English for all files. Clear, plain sentences; no marketing fluff.
- Facts must match the code. If the code and requirements disagree, follow the code and list the difference in your report.
- No personal data: no emails, no local paths such as `/Users/...`.

## Acceptance evidence

- `cargo test --workspace --features kun/fake-provider` still passes. Nothing should change, but run it.
- Report the list of files, any code-vs-docs differences found, and any links outside your scope that need updating.
