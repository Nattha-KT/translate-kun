# Slices

Each slice is one unit of work for one implementer, reviewed by one reviewer, approved by the owner before merge to `main`.

| Slice | Title | Depends on | Implementer model | Status |
|---|---|---|---|---|
| S0 | Workspace skeleton + CI + `kun --version` | — | Sonnet 5.5 / medium | merged |
| S1 | Secret Guard (TDD) | S0 | Opus 5.5 / high | in progress |
| S2 | Config loading + Project Config trust rules | S0 | Sonnet 5.5 / high | todo |
| S3 | Provider trait + fake Provider | S0 | Sonnet 5.5 / high | in progress |
| S4 | Google Provider + credentials / keychain | S3 | Opus 5.5 / high | todo |
| S5 | CLI: `:th` syntax, stdin, Language Pair, output | S2, S3 | Sonnet 5.5 / high | todo |
| S6 | Anthropic Provider + Protected Terms | S3, S5 | Sonnet 5.5 / high | todo |
| S7 | `kun init`, `kun auth`, exit codes, `--json` | S4, S5 | Sonnet 5.5 / medium | todo |
| S8 | Release: cargo-dist, Homebrew | S7 | Sonnet 5.5 / medium | todo |

At most 2 implementers run at once, and there is 1 reviewer (Codex).
