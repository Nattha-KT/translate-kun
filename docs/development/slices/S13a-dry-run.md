# S13a — `kun --dry-run`: show exactly what would be sent

## Goal

Let users verify for themselves, without trusting us, what leaves their machine. `--dry-run` runs the whole pipeline up to the point of sending, then prints the request it would make, and sends nothing.

## Read first

- `AGENTS.md`
- `crates/kun/src/app.rs`, `guard.rs`, `providers.rs`, `json_output.rs`
- `crates/kun-core/src/translate.rs`, `protected_terms.rs`
- `docs/usage.md`, `docs/json-output.md`

## In scope

- `crates/kun/**`
- `docs/usage.md`, `docs/json-output.md`, `CHANGELOG.md` (add only the parts about `--dry-run`)
- Do **not** change `crates/kun-core` public APIs. If you really need a core change, stop and report.
- S13b works in parallel on other docs, `fuzz/` and the workflows. Do not touch them.

## Behaviour

1. `--dry-run` runs these steps exactly as a real run would:
   1. argument parsing
   2. config loading and merging
   3. the size limit
   4. the Secret Guard
   5. Target Language resolution
   6. Protected Terms `protect`

   It **never** builds a network client and never contacts a Provider. It does **not** read credential values. It may report the credential *source* (env, keychain, config file or none) the way `kun auth status` does.
2. **Secret Guard in dry-run.**
   - Never prompt.
   - Report the finding kinds and byte positions, never the values, and the decision a real run would take: Send, Redact, AskUser or Block.
   - On Block, or on AskUser in a non-interactive context, exit 6, as a real run would.
   - On AskUser in an interactive context, show what would be sent if the user answered "yes", and say clearly that a real run would ask first.
3. **Output (human mode, stdout).** A short report:
   - Provider name
   - destination host (`translation.googleapis.com` or `api.anthropic.com`; map it from the Provider name in the CLI)
   - model (Anthropic only)
   - Source and Target Language, or "detect"
   - whether a Language Pair second call may happen
   - the credential source
   - and finally the **exact text that would be sent**, after redaction and Protected Terms placeholders, between clear begin/end markers
4. **`--json` with `--dry-run`.** Return one object with `"schema_version": 1, "dry_run": true` and fields for each item above. Adding fields is compatible, so `schema_version` stays 1. Document it in `docs/json-output.md`.
5. The exit code is 0 when a real run would have proceeded to send.

## Tests (no network, no keychain)

- A dry run with a fake AWS key and `--redact` shows `[REDACTED]` in the would-send text, and the key appears nowhere in stdout or stderr.
- A dry run with a fake key, piped stdin and no `--redact` exits 6.
- Protected Terms appear as placeholders in the would-send text.
- `--dry-run --json` has the documented shape.
- A dry run with no credentials still succeeds and reports `none`.

## Acceptance evidence

- The Definition of Done in `AGENTS.md` passes.
- Report sample human output, the test count and the docs changed.
