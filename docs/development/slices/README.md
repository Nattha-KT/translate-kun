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
| S8 | Release: cargo-dist, Homebrew | S7 | Sonnet 5.5 / medium | deferred: owner chose not to publish yet (2026-11-21) |
| S9 | Open-source docs and community files | S7b | Sonnet 5.5 / medium | merged |
| S10 | Packaging, CI and quality gates | S7b | Sonnet 5.5 / high | merged |
| S11a | CLI: apply Protected Terms and style, shell completions | S10 | Sonnet 5.5 / high | merged |
| S11b | Core structure: split config, `non_exhaustive`, docs, licence files | S10 | Sonnet 5.5 / medium | merged |
| S12 | Security audit fixes | S11 | Opus 5.5 / high | merged |
| S13 | Trust and transparency: `--dry-run`, privacy doc, audit report, fuzzing, Scorecard | S12 | Sonnet 5.5 / high | merged |
| S14 | Rate limits, quotas and output limits (exit code 8) | S13 | Sonnet 5.5 / high | in progress |

At most 2 implementers run at once, and there is 1 reviewer (Codex).

## Follow-ups

- **Secret Guard (from the S12 re-check):** an AWS key ID followed directly by `_` (`AKIA…12345_old`) is not detected, because of the trailing boundary. `aws configure set aws_secret_access_key <value>`, which is space-separated with no `=` or `:`, is not detected either. Both are rare; they were accepted.
- **Security audit nits deferred:** N-1 (no ownership check on a discovered `.kun.toml`), N-2 (document that a repo can pick the Provider and model), N-6 (`KUNPT` placeholder collision only garbles output), N-8 (pin actions by SHA).

- **Secret Guard:** a truncated PEM block inside JSON whose last base64 line ends with a closing quote (`Zg=="`) is not fully redacted. This is rare, because JSON-embedded keys normally include the END line. Accepted at the S1 merge.
- **Accepted trade-offs at the S4/S5 merge:**
  - keyring `PlatformFailure` falls back to the 0600 User Config; this is usability on Linux without Secret Service.
  - The Pair path fills `detected_source` from the first call when the second call reports none; that value is correct by construction.
