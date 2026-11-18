# S0 — Workspace skeleton + CI + `kun --version`

## Goal

Create the empty-but-working Cargo workspace that every later slice builds on.

## Read first

- `AGENTS.md` (all)
- `docs/requirements.md` §11 (technical decisions) and §13 (licensing)

## In scope (files you may create or edit)

- `Cargo.toml` (workspace root), `Cargo.lock`
- `crates/kun-core/**`
- `crates/kun/**`
- `rust-toolchain.toml`
- `.github/workflows/ci.yml`
- `LICENSE-MIT`, `LICENSE-APACHE`. Copyright holder: `Nattha KT`, year 2026.
- `README.md`: a minimal stub with one-paragraph description, the license section, and a "status: pre-alpha" note.

## Requirements

1. Cargo workspace with `resolver = "3"` (or the current recommended value) and two members:
   - `crates/kun-core`: a library crate. Expose `pub fn version() -> &'static str` that returns `env!("CARGO_PKG_VERSION")`, with one unit test.
   - `crates/kun`: a binary crate with the binary name `kun`, depending on `kun-core` by path.
2. Use `clap` (derive feature) in `crates/kun` only. `kun --version` prints `kun <version>`, and `kun --help` works. Running bare `kun` with no args prints help to stderr and exits with code 2 for now.
3. Share common package metadata through `[workspace.package]`: version `0.1.0`, `edition = "2024"`, `license = "MIT OR Apache-2.0"`, and the repository field left out for now.
4. Set `[workspace.lints]` with `unsafe_code = "forbid"`. Each crate opts in with `lints.workspace = true`.
5. Pin `rust-toolchain.toml` to `stable` with components `clippy` and `rustfmt`.
6. Add a GitHub Actions CI workflow on push/PR that runs fmt check, clippy `-D warnings`, and tests on `ubuntu-latest` and `macos-latest`. It is not run now; it just has to be valid YAML.
7. Add an integration test in `crates/kun/tests/` that runs the built binary with `--version` and asserts the output. Use `assert_cmd`; it is allowed as a dev-dependency.

## Allowed dependencies

`clap` (derive), `anyhow`, `thiserror`. Dev-dependencies: `assert_cmd`, `predicates`. Nothing else.

## Out of scope

Any translation, config, Provider, or Secret Guard code. Don't create placeholder modules for them.

## Acceptance evidence

- The Definition of Done from `AGENTS.md` passes.
- `cargo run -p kun -- --version` prints `kun 0.1.0`.
- Committed on your branch.
