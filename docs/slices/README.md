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
| S6 | Anthropic Provider + Protected Terms (core) | S4, S5 | Sonnet 5.5 / high | in progress |
| S7 | Google wiring, `kun auth`, `kun init`, `--json` (CLI) | S4, S5 | Sonnet 5.5 / medium | in progress |
| S8 | Release: cargo-dist, Homebrew | S7 | Sonnet 5.5 / medium | todo |

At most 2 implementers run at once, and there is 1 reviewer (Codex).

## Follow-ups

- **Secret Guard:** a truncated PEM block inside JSON whose last base64 line ends with a closing quote (`Zg=="`) is not fully redacted. This is rare, because JSON-embedded keys normally include the END line. Accepted at the S1 merge.
- **Anthropic CLI wiring:** a small task after S6 and S7 merge.
- **Accepted trade-offs at the S4/S5 merge:**
  - keyring `PlatformFailure` falls back to the 0600 User Config; this is usability on Linux without Secret Service.
  - The Pair path fills `detected_source` from the first call when the second call reports none; that value is correct by construction.
