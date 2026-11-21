# S13b — Privacy doc, published audit report, fuzzing, OpenSSF Scorecard

## Goal

Give users independent ways to judge whether `kun` is trustworthy. This slice adds a plain privacy statement, the security audit summary with honest limits, repeatable fuzzing, and an automated public score of the repository's security practices.

## Read first

- `AGENTS.md`, `SECURITY.md`, `README.md`, `docs/architecture.md`, `docs/configuration.md`
- The audit report (read only; it is outside the repository and may contain local paths that must not be copied):
  a local, untracked copy of the raw audit report (never commit it)
- `docs/development/slices/S12-security-audit-fixes.md` and the `CHANGELOG.md` Security section, which together show what was fixed.

## In scope

- `docs/privacy.md` (new)
- `docs/security/audit-2026-11.md` (new)
- `SECURITY.md` and `README.md` (new sections, badges, links)
- `fuzz/` (new)
- `.github/workflows/scorecard.yml` (new)
- `.github/workflows/fuzz.yml` (new)
- `CONTRIBUTING.md` (a fuzzing section)
- Do **not** touch `crates/**`, `docs/usage.md` or `docs/json-output.md`. S13a owns `--dry-run` and documents it there. You may **link** to `kun --dry-run` from the privacy doc as "see docs/usage.md".

## Tasks

1. **`docs/privacy.md`.** Answer "what data leaves my machine?" in about one page.
   - Include a table with the columns: data | when it leaves | where it goes | can I prevent it.
   - Cover these rows: the text, after the Secret Guard, redaction and Protected Terms; the API key, in an HTTPS header to the chosen Provider only; nothing else (no telemetry, no update checks, no cache).
   - Explain how to verify it yourself: `kun --dry-run`, reading the source, and watching network traffic.
   - Explain what the Providers' own policies govern, with links to the Google Cloud and Anthropic privacy pages (official URLs only).
2. **`docs/security/audit-2026-11.md`.** A public summary of the audit.
   - Scope, method, date and commit.
   - Findings with severity and status (fixed in S12, accepted, or deferred), and the list of areas verified as sound.
   - A clear disclaimer: **the audit was performed by an AI model (Claude Opus 5.5), not by an independent professional security firm, and the project is pre-1.0 personal software.**
   - Remove any local paths or personal data that appear in the raw report.
3. **`SECURITY.md`.** Add a "Security assurance" section that links to the audit and states the same disclaimer. Also list the continuous checks: cargo-deny, dependabot, fuzzing and Scorecard.
4. **`README.md`.** Add links to Privacy and Security, plus an OpenSSF Scorecard badge (`https://api.scorecard.dev/projects/github.com/Nattha-KT/translate-kun/badge`).
5. **Fuzzing (`fuzz/`, cargo-fuzz layout, its own workspace, not a member of the main workspace).** Add three targets:
   - `secret_guard`: scan, then redact, on arbitrary UTF-8. Assert the output contains no byte range of any finding, and that nothing panics.
   - `config_parse`: parse arbitrary bytes as a Project Config and a User Config, and assert nothing panics.
   - `protected_terms`: protect then restore round-trips on arbitrary text and terms.

   Add a seed corpus with a few fake-secret samples (FAKE/EXAMPLE style, built so no single literal looks real). Document how to run it in CONTRIBUTING.md (`cargo +nightly fuzz run <target>`).
6. **`.github/workflows/fuzz.yml`.** Run weekly and on manual dispatch, with nightly Rust; run each target for 60 seconds and upload crash artefacts on failure. Set `permissions: contents: read`.
7. **`.github/workflows/scorecard.yml`.** Use the standard `ossf/scorecard-action` setup: on push to main, weekly, and on branch-protection changes; `publish_results: true`; upload SARIF to code scanning; minimal permissions (`security-events: write`, `id-token: write`, everything else read).

## Acceptance evidence

- The main workspace's Definition of Done still passes. `fuzz/` must not break `cargo test --workspace`.
- If nightly and cargo-fuzz are available, build the fuzz targets; otherwise say so. Do not install nightly without reporting it.
- Report the files created and confirm that no personal data or local paths were copied from the raw report.
