# Slices

Each slice is one unit of work for one implementer, reviewed by one reviewer, approved by the owner before merge to `main`.

| Slice | Title | Depends on | Implementer model | Status |
|---|---|---|---|---|
| S0 | Workspace skeleton + CI + `kun --version` | — | Sonnet 5.5 / medium | merged |
| S1 | Secret Guard (TDD) | S0 | Opus 5.5 / high | merged |
| S2 | Config loading + Project Config trust rules | S0 | Sonnet 5.5 / high | merged |
| S3 | Provider trait + fake Provider | S0 | Sonnet 5.5 / high | merged |
| S4 | Credentials + Google Provider | S2, S3 | Opus 5.5 / high | merged |
| S5 | CLI: syntax, stdin, Language Pair, Secret Guard flow, output, exit codes | S1, S2, S3 | Sonnet 5.5 / high | merged |
| S6 | Anthropic Provider + Protected Terms (core) | S4, S5 | Sonnet 5.5 / high | merged |
| S7 | Google wiring, `kun auth`, `kun init`, `--json` (CLI) | S4, S5 | Sonnet 5.5 / medium | merged |
| S7b | Anthropic CLI wiring | S6, S7 | Sonnet 5.5 / low | merged |
| S8 | Release: cargo-dist, Homebrew tap, crates.io, verifiable artefacts | S7 | Sonnet 5.5 / high | merged (PR #9); released as v0.1.0 |
| S9 | Open-source docs and community files | S7b | Sonnet 5.5 / medium | merged |
| S10 | Packaging, CI and quality gates | S7b | Sonnet 5.5 / high | merged |
| S11a | CLI: apply Protected Terms and style, shell completions | S10 | Sonnet 5.5 / high | merged |
| S11b | Core structure: split config, `non_exhaustive`, docs, licence files | S10 | Sonnet 5.5 / medium | merged |
| S12 | Security audit fixes | S11 | Opus 5.5 / high | merged |
| S13 | Trust and transparency: `--dry-run`, privacy doc, audit report, fuzzing, Scorecard | S12 | Sonnet 5.5 / high | merged |
| S14 | Rate limits, quotas and output limits (exit code 8) | S13 | Sonnet 5.5 / high | merged |
| S15 | Pre-publish hardening: pinned actions, CODEOWNERS, Project Config ownership check, CodeQL, provider data notice | S14 | Sonnet 5.5 / high | merged |
| S16 | Test depth and CI quality tools: Windows stdin fix, `proptest`, coverage, `cargo-machete`, `typos`, mutation testing, toolchain pins | S15 | Sonnet 5.5 / high | merged (PR #5, the first slice delivered as a PR) |

At most 2 implementers run at once, and there is 1 reviewer (Codex).

## Follow-ups

- **Unmaintained transitive crates (from `cargo deny`):** `zvariant`, `zvariant_derive` and `zbus_names` are flagged unmaintained (RUSTSEC-2026-0370, RUSTSEC-2026-0371, RUSTSEC-2026-0372). They are transitive on Linux only, through `keyring` → `secret-service` → `zbus`, and are not vulnerabilities. Wait for a `keyring` update.
- **Secret Guard (from the S12 re-check):** an AWS key ID followed directly by `_` (`AKIA…12345_old`) is not detected, because of the trailing boundary. `aws configure set aws_secret_access_key <value>`, which is space-separated with no `=` or `:`, is not detected either. Both are rare; they were accepted.
- **Security audit nits deferred:** N-2 (document that a repo can pick the Provider and model), N-6 (`KUNPT` placeholder collision only garbles output). N-1 and N-8 were done in S15.
- **Mutation testing (S16):** `cargo mutants` covered 68 of 88 mutants in `secret_guard.rs`; the last 20 (`next_line`, `redact`, `merge_ranges`, `decide`) were not run, because mutants that make `next_line` loop forever hang the run. Re-run them with `--exclude-re next_line` when convenient.
- **Release (S8):** the first `v0.1.0` run failed once in `host` with a transient GitHub artifact error ("No artifacts found for ID"), and re-running the failed jobs fixed it. Trusted Publishing is first used for real by `0.1.1`, so watch that release's `custom-publish-crates` job.

- **Secret Guard:** a truncated PEM block inside JSON whose last base64 line ends with a closing quote (`Zg=="`) is not fully redacted. This is rare, because JSON-embedded keys normally include the END line. Accepted at the S1 merge.
- **Accepted trade-offs at the S4/S5 merge:**
  - keyring `PlatformFailure` falls back to the 0600 User Config; this is usability on Linux without Secret Service.
  - The Pair path fills `detected_source` from the first call when the second call reports none; that value is correct by construction.
