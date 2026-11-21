# S12 — Fixes from the pre-publish security audit

## Background

A read-only Opus 5.5 audit of main at `f51d4e3` concluded **safe to publish, no blockers**. The owner approved fixing these findings. The IDs refer to the audit report.

## Read first

- `AGENTS.md`, especially "Security in proportion"
- `crates/kun-core/src/secret_guard.rs` and its tests
- `crates/kun/src/providers.rs`, `app.rs`
- `crates/kun-core/src/config/merge.rs`
- `crates/kun-core/src/provider/anthropic.rs`
- `docs/usage.md`, `docs/configuration.md`, `docs/architecture.md`, `SECURITY.md`, `CHANGELOG.md`

## Skills

Use `tdd` for every code change: write a failing test, then make it pass.

## In scope

- `crates/kun-core/**` and `crates/kun/**`: the code and tests for the items below
- The docs listed above

## Items

- **S-1 (should-fix).** `--redact` hides the AWS key ID but not the secret. Add a narrow built-in pattern for the AWS secret access key in assignment form (case-insensitive key name, `=` or `:`, optional quotes, 40 base64-ish characters). Report it as kind `AwsSecretAccessKey`. Test with a fake credentials block that contains both the ID and the secret: after `--redact`, neither appears.
- **S-2 (should-fix).** False negatives after JSON-escaped `\n`/`\t` and after `_`, because the leading `(?-u:\b)` treats `n`, `t` and `_` as word characters.
  - For patterns with a distinctive prefix (`sk-ant-`, `AIza`, `gh*_`, `github_pat_`, `xox?-`, `eyJ`, `AKIA`/`ASIA`), drop the leading boundary, or use `(?:^|[^A-Za-z0-9]|\\[nt])` without capturing the boundary in the reported range.
  - Keep every existing must-not-detect test green. Add must-detect tests for `"a\nAKIA…"`, `"x\tAIza…"`, `KEY_AKIA…`, and a JSON string containing a GitHub token after `\n`.
- **S-3 (should-fix).** A configured `providers.<name>.endpoint` is silently ignored, and the text goes to the public API. Fail closed: if an endpoint is set for the Provider in use, exit 3 with `providers.<name>.endpoint is not supported yet; remove it to use the public API.` Test it.
- **N-3 (nit).** When stdout is a terminal, strip C0/C1 control characters from the Provider output except `\n` and `\t`, so a prompt-injected model cannot emit escape sequences. Leave pipes and `--json` unchanged. Test the stripping function.
- **N-4 (docs).** In `docs/usage.md` and `SECURITY.md`, say that text passed as arguments lands in shell history and `ps`, so sensitive text should go through stdin.
- **N-5 (nit).** Add high-precision patterns: Stripe live keys (`sk_live_` / `rk_live_`), GitLab `glpat-`, npm `npm_` (36 alphanumeric characters), Hugging Face `hf_` (34+). Each gets must-detect and must-not-detect tests. Update the kinds list in the docs.
- **N-7 (nit).** In the `AnthropicProvider` `Debug`, print the model as `<redacted>`, to match the AGENTS.md rule.
- **N-9 + out-of-model notes (docs).**
  - `docs/configuration.md`: note that `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY` and `NO_PROXY` are honoured, and that HTTPS always goes through CONNECT, so the key stays inside TLS.
  - `docs/architecture.md`: state that `kun-core` does not enforce the Secret Guard, so every Adapter must call it before any Provider call.
- Add a CHANGELOG entry under Unreleased → Security.

## Out of scope

N-1, N-2, N-6, N-8 (accepted or deferred by the coordinator).

## Acceptance evidence

- The Definition of Done in `AGENTS.md` passes.
- Report one line per item and the test count.
