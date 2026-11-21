# S16 — Test depth and CI quality tools

## Background

The repository is public now, and `main` is protected by a ruleset: changes land only through pull requests that pass the required checks. This is the first slice delivered as a PR.

The owner approved these items. They add testing tools and CI tools. **Shipped code gains no runtime dependency.** Keep everything proportionate (see "Security in proportion" in `AGENTS.md`).

## Read first

- `AGENTS.md`
- `crates/kun/src/input.rs` (the stdin check), `crates/kun/tests/wiring.rs`
- `crates/kun-core/src/secret_guard.rs` and its tests
- `crates/kun-core/src/protected_terms*` (or wherever Protected Terms live)
- `.github/workflows/ci.yml`, `.github/dependabot.yml`, `CONTRIBUTING.md`, `CHANGELOG.md`

## Skills

Use `tdd` for item 1 (the only behaviour change), and `diagnosing-bugs` if the Windows cause is not obvious.

## In scope

- `crates/kun/src/input.rs`, `crates/kun/tests/**`
- `crates/kun-core/**`: tests and dev-dependencies only; no change to library behaviour
- `Cargo.toml` (workspace dev-dependency entries), `Cargo.lock`, `fuzz/Cargo.lock` if it must follow
- `.github/workflows/ci.yml`, `.github/dependabot.yml`
- `_typos.toml` (new, only if needed)
- `CONTRIBUTING.md`, `CHANGELOG.md`
- `docs/development/slices/S16-test-depth.md` (this file; add it) and `docs/development/slices/README.md`

## Items

1. **Windows: spurious "ignoring stdin" warning** (CI `test (windows-latest)` fails 4 tests in `crates/kun/tests/wiring.rs`).
   - **Cause:** `stdin_is_known_empty()` in `input.rs` is Unix-only and returns `false` elsewhere. So on Windows, an empty file or `NUL` on stdin triggers the warning, which also appears in `--json` `warnings`.
   - **Fix:** without `unsafe` and without a new crate. For example, use `std::os::windows::io::AsHandle` and `try_clone_to_owned` → `File` → `metadata()`. An empty regular file has `is_file() && len() == 0`.
   - For `NUL`:
     - Detect it if std makes that possible safely.
     - If it cannot be detected safely, keep the warning for `NUL` on Windows. Then make the affected tests not depend on it: give the JSON tests an empty temp file as stdin instead of `Stdio::null()`, and `cfg`-gate only the `NUL`-specific assertion with a one-line comment explaining why.
   - **CI:** remove `continue-on-error` from the Windows test job in `ci.yml`, so Windows really gates.
   - **Evidence:** you cannot run Windows locally. Say so, and say that the PR's CI run is the evidence.
2. **Property-based tests with `proptest`** (dev-dependency only).
   - **Secret Guard:**
     - For arbitrary text with a fake key inserted at an arbitrary position (built from FAKE/EXAMPLE-style parts, as in the existing tests), after redaction the key's bytes never appear.
     - Redaction never panics and always returns valid UTF-8.
   - **Protected Terms:** protect → restore round-trips to the original text for arbitrary text and terms (exclude inputs that already contain the `KUNPT` placeholder pattern; that is accepted nit N-6).
   - **Speed:** keep the default case count modest, so that `cargo test` stays fast (a few seconds at most).
3. **Coverage report in CI.** Add a `coverage` job using `cargo-llvm-cov`.
   - Install it with a SHA-pinned `taiki-e/install-action` and resolve the SHA as in S15: `gh api` or `git ls-remote`, never guessed.
   - Print the summary in the job log and in `$GITHUB_STEP_SUMMARY`.
   - **Report only:** no threshold, no third-party upload service, and not a required check.
4. **`cargo-machete`** (unused dependencies). Add a CI job, installed with the same SHA-pinned `taiki-e/install-action`. Fix anything it finds, or add a justified ignore in the crate's `Cargo.toml` metadata.
5. **`typos`** (spell check).
   - Add a CI job using a SHA-pinned `crate-ci/typos` action.
   - Fix real typos.
   - Put intentional words in `_typos.toml`: Thai romanisation, `KUNPT`, fake-key fixtures, and similar.
   - Do not change test fixtures in a way that weakens a detector test.
6. **Mutation testing, one-off (not in CI).** Run `cargo mutants` locally on `crates/kun-core/src/secret_guard.rs` only.
   - If cargo-mutants is not installed, install it with `cargo install cargo-mutants --locked` and report that you did.
   - Add tests that kill the surviving mutants that matter.
   - Do **not** chase equivalent mutants or cosmetic ones. List them in the report instead.
   - Add a short "Mutation testing" section to `CONTRIBUTING.md`, showing how to run it.
   - **Time budget:** if a full run takes too long, use `--file crates/kun-core/src/secret_guard.rs` and `--timeout`, and report what was covered.
7. **Unify the `dtolnay/rust-toolchain` pins.**
   - Every use becomes the SHA of the action's `master` branch with the comment `# master`, keeping the explicit `with: toolchain: <x>` that already selects the toolchain.
   - Resolve the SHA with `git ls-remote https://github.com/dtolnay/rust-toolchain refs/heads/master`.
   - Add a one-line comment in `ci.yml` explaining that the toolchain comes from `with: toolchain`, not from the ref.
8. **Docs.**
   - **`CHANGELOG.md`, Unreleased:**
     - Fixed: the Windows stdin warning.
     - Added (development): the proptest, coverage, machete and typos checks.
   - **`CONTRIBUTING.md`:** how to run each tool locally.
   - **`docs/development/slices/README.md`:**
     - Add the S16 row.
     - Add a follow-up: `zvariant`, `zvariant_derive` and `zbus_names` are flagged unmaintained (RUSTSEC-2026-0370/0371/0372). They are transitive on Linux via `keyring` → `secret-service` → `zbus`, and are not vulnerabilities. Wait for a `keyring` update.

## Out of scope

- S8 (release), `cargo-semver-checks`, N-2, N-6.
- Repository settings.
- Pushing or opening the PR: the coordinator does that.

## Acceptance evidence

- The Definition of Done in `AGENTS.md` passes. Report the test count.
- The new CI jobs are valid YAML, and every new `uses:` is SHA-pinned with a version comment. Report an action → SHA table.
- Report one line per item, the mutants summary (caught, missed, unviable, timeout), and the final runtime of `cargo test`.
