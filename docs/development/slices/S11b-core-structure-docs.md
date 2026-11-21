# S11b — Core structure: split config, `non_exhaustive`, API docs, licence files

## Goal

Make `kun-core` easy for contributors to navigate and safe to evolve as a published library. Make no behaviour changes and keep the public paths the same.

## Read first

- `AGENTS.md`, `docs/architecture.md`
- `crates/kun-core/src/**`

## In scope

- `crates/kun-core/**`
- `crates/kun/Cargo.toml` and `crates/kun-core/Cargo.toml` (only for the licence-file packaging in task 4)
- The licence files in task 4
- `docs/architecture.md`: update the module map.
- Do **not** edit `crates/kun/src/**`. S11a works there in parallel. If task 2 breaks a `match` in `crates/kun`, add the needed wildcard arm, and **list it in your report**. That is the only allowed edit there; keep it as small as possible.

## Tasks

1. **Split `config.rs` (about 1,500 lines) into a `config/` module directory.**
   - Suggested files: `mod.rs` (re-exports and docs), `schema.rs` (file types), `load.rs` (reading files and permission checks), `merge.rs` (precedence and tighten-only rules), `discover.rs` (finding the Project Config and the User Config path), `trust.rs` (Project Config forbidden keys).
   - Move tests next to the code they test.
   - The public paths stay identical: `kun_core::config::X` must still resolve for every item that is public today. This is a pure move, with no logic changes.

2. **`#[non_exhaustive]`** on public enums that are likely to grow: `SecretKind`, `ProviderError`, `NetworkReason`, `ConfigError`, `CredentialError`, `ProviderName`, `Decision`, `TranslateError` (check each).
   - Do **not** add it to enums whose variants form a closed set, such as `TargetPlan`. Explain your choices in the report.
   - Inside the crate nothing changes. In `crates/kun` a `match` may need a wildcard arm (see the scope note above).

3. **API docs.**
   - Add `#![warn(missing_docs)]` to `kun-core/src/lib.rs`, with a crate-level doc comment containing a short runnable example (a doc-test) that uses `FakeProvider` and `translate`.
   - Document every public item in one or two plain sentences, using `GLOSSARY.md` terms.
   - `cargo doc --no-deps` with `RUSTDOCFLAGS=-D warnings` must pass.

4. **Licence files in the packages.**
   - Make both crates ship `LICENSE-MIT` and `LICENSE-APACHE` (and `NOTICE`), so `cargo package --list -p <crate>` shows them.
   - The usual approach is relative symlinks in each crate directory pointing to the root files. Cargo follows symlinks. If symlinks cause Windows CI problems, copy the files and add a CI check that the copies match the root files. Explain which approach you chose.

## Acceptance evidence

- The Definition of Done in `AGENTS.md` passes, `cargo doc` passes with `-D warnings`, and `cargo package --list` for both crates shows the licence files.
- Report the new module map, the `non_exhaustive` decisions, any edit in `crates/kun`, and the test count. The count must equal the count before the change unless you added doc-tests.
