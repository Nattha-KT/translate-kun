# S10 — Packaging, CI and quality gates

## Goal

Bring Cargo metadata, CI and supply-chain checks up to common production Rust standards so the crates can be published, and so contributors get fast, clear feedback. Keep it proportionate: widely used tools only, and no exotic setup.

## Read first

- `AGENTS.md` (all), `Cargo.toml`, `crates/*/Cargo.toml`, `.github/workflows/ci.yml`, `rust-toolchain.toml`

## In scope

- `Cargo.toml` (workspace), `crates/kun/Cargo.toml`, `crates/kun-core/Cargo.toml`, `Cargo.lock`
- `.github/workflows/ci.yml`, `.github/dependabot.yml`, `deny.toml`, `rust-toolchain.toml`, `clippy.toml` (only if needed)
- Rust sources **only** where a rename or a new lint forces a mechanical change (imports, `#[allow]` with a reason). No behaviour changes.

## Tasks

1. **Package rename (owner decision).** Rename the packages; the command stays `kun`.
   - `crates/kun`: `name = "translate-kun"`, keep `[[bin]] name = "kun"`.
   - `crates/kun-core`: `name = "translate-kun-core"`. Add `[lib] name = "kun_core"` so `use kun_core::...` keeps working everywhere.
   - Update the path dependency, and every `--features kun/fake-provider` in CI to `--features translate-kun/fake-provider`.
   - List in your report every other file that mentions the old feature path (for example AGENTS.md and docs), so the coordinator can update them. Do not edit AGENTS.md.

2. **Metadata in `[workspace.package]`, inherited by both crates.**
   - `repository = "https://github.com/Nattha-KT/translate-kun"`, `homepage` (same), `authors = ["Nattha KT"]`, `rust-version`, `readme`.
   - Per crate: a `description`, `keywords` (up to 5, e.g. `translate`, `translation`, `cli`, `thai`, `i18n`), and `categories` (`command-line-utilities`, `localization`, `internationalization`, using valid crates.io slugs).
   - Use `exclude`/`include` so packages do not ship `docs/development` or the test fixtures that look like secrets. Check this with `cargo package --list -p <crate>`.

3. **`[workspace.dependencies]`.** Move every shared dependency version into the workspace table, and have the crates use `dep.workspace = true`.

4. **MSRV.** Determine the lowest Rust version that builds. Edition 2024 needs ≥ 1.85, and dependencies may need more; read their `rust-version` in `Cargo.lock` / `cargo metadata`. Set `rust-version` to that value, and add a CI job that builds and tests on exactly that toolchain.

5. **Lints.** Keep `unsafe_code = "forbid"`. Add a modest, widely used `[workspace.lints.clippy]` set: start from `all` at warn, then add a few pedantic ones that help readability without churn (e.g. `uninlined_format_args`, `semicolon_if_nothing_returned`, `redundant_closure_for_method_calls`). Fix what they flag. **Do not** enable all of `pedantic`, and do not enable `restriction`.

6. **CI (`.github/workflows/ci.yml`).**
   - Triggers: `push` to main and `pull_request`. Set `permissions: contents: read` and `concurrency` with cancel-in-progress.
   - Jobs:
     - `fmt`
     - `clippy` (with the fake-provider feature, `-D warnings`)
     - `test`, on an ubuntu, macos and windows matrix. Windows gets `continue-on-error: true` (best-effort, per the requirements).
     - `msrv`
     - `docs`: `cargo doc --no-deps` with `RUSTDOCFLAGS=-D warnings`
     - `deny`: `EmbarkStudios/cargo-deny-action`
   - Use `dtolnay/rust-toolchain` and `Swatinem/rust-cache`. Pin actions to version tags; dependabot keeps them updated.

7. **`deny.toml`.**
   - Licences: an allow-list (MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2/3-Clause, ISC, Unicode-3.0, Zlib, MPL-2.0 only if a dependency needs it).
   - `advisories` deny on vulnerabilities.
   - `bans`: duplicate versions as warn.
   - `sources`: crates.io only.
   - Run `cargo deny check` locally if `cargo-deny` is available. If it is not installed, note that in your report.

8. **`.github/dependabot.yml`.** `cargo` and `github-actions` ecosystems, weekly, with grouped minor and patch updates.

## Out of scope

- Docs and community files (S9 works in parallel on README, CONTRIBUTING, `docs/` and the issue templates; do not touch them), release workflow and cargo-dist (deferred S8), and behaviour changes.

## Acceptance evidence

- fmt, clippy (`--features translate-kun/fake-provider`) and test all pass.
- `cargo package --list` is clean for both crates (it shows no `docs/development` and no secret-looking fixtures, or you explain why it does).
- Report the chosen MSRV and how you determined it, the lint list, the files that still mention old names, and whether `cargo deny` ran.
