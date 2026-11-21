# Contributing to translate-kun

Thank you for your interest. Bug reports, documentation fixes, tests and code are all welcome. This is a small personal project, so please be patient: reviews are best effort.

By taking part you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md). To report a security problem, use the process in [SECURITY.md](SECURITY.md), not a public issue.

## Prerequisites

- Rust **stable**, installed with [rustup](https://rustup.rs). The repository pins the channel in `rust-toolchain.toml`, and rustup picks it up automatically.
- `rustfmt` and `clippy` components (`rustup component add rustfmt clippy`).
- No API keys are needed to build or test. Tests never use the network.

## Getting the code

```sh
git clone https://github.com/Nattha-KT/translate-kun
cd translate-kun
```

## Build, test and lint

These are the checks every change must pass. CI runs the same commands on Linux and macOS.

```sh
cargo build --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets --features kun/fake-provider -- -D warnings
cargo test --workspace --features kun/fake-provider
```

The `kun/fake-provider` feature lets tests use an offline fake Provider (`--provider fake`) and an in-memory credential store. It is for tests only.

Live tests, which call real Provider APIs, are optional and never run in CI:

```sh
KUN_GOOGLE_API_KEY=... cargo test -p kun-core --features live
```

## Code guidelines

- Write simple, idiomatic Rust. Prefer owned types over lifetimes where the cost is small, no async, no macros of your own, no clever generics.
- Terminal I/O, prompts and exit codes live only in `crates/kun`. Logic lives in `crates/kun-core`. See [docs/architecture.md](docs/architecture.md).
- Use the terms in [GLOSSARY.md](GLOSSARY.md) (Provider, Target Language, Project Config, and so on) in code, comments and commit messages.
- **Never leak secrets.** Do not accept API keys as flags. Do not print, log or serialise a credential. A type or error that can hold user text, a config string, a regex, a URL or an HTTP body must not show it in `Debug`, `Display` or `Error::source()`. Add a test that formats with `{:?}` and `{:#?}` and checks that a fake key is absent.
- Test fixtures that look like secrets must be obviously fake (they contain `EXAMPLE` or `FAKE`) and must still exercise the detector. Never commit real keys or `.env` files.
- **No network in tests.** Use the fake Provider.
- Do not reverse a decision in [docs/adr/](docs/adr/) without discussing it first in an issue.
- Security work is kept in proportion. The threat model is stopping credentials and text from leaking to third parties, logs, the terminal, shell history and other local users. See [SECURITY.md](SECURITY.md).

## Branches and commits

- Branch from `main`. Use a short, descriptive name such as `fix/stdin-warning` or `feat/deepl-provider`.
- Use [Conventional Commits](https://www.conventionalcommits.org): `feat:`, `fix:`, `docs:`, `test:`, `chore:`, `refactor:`.
  - `feat: add --timeout flag`
  - `fix: keep Protected Terms in the first line`
- Keep commits focused. One logical change per commit is easiest to review.

## Pull request checklist

The [pull request template](.github/PULL_REQUEST_TEMPLATE.md) has the same list. Before you open a PR:

- [ ] `cargo fmt --all --check` passes.
- [ ] `cargo clippy --workspace --all-targets --features kun/fake-provider -- -D warnings` passes.
- [ ] `cargo test --workspace --features kun/fake-provider` passes.
- [ ] New behaviour has tests, and no test uses the network.
- [ ] Docs are updated (`README.md`, `docs/`) if behaviour or flags changed.
- [ ] `CHANGELOG.md` has an entry under `## [Unreleased]`.
- [ ] No secrets, real keys or personal data in code, fixtures or docs.
- [ ] The PR links the issue it fixes, if there is one.

## Adding a Provider

Providers sit behind the `Provider` trait in `kun-core`. The step-by-step guide is in [docs/architecture.md](docs/architecture.md#how-to-add-a-new-provider). Only official, key-authenticated APIs are accepted ([ADR 0001](docs/adr/0001-official-providers-no-scraping.md)). Please open an issue first so we can agree on the Provider before you build it.

## Good first issues

Issues labelled `good first issue` are small and self-contained. Good places to start:

- Improve a message, a help text or a doc page that confused you.
- Add a missing test case (for example a Secret Guard sample that must be detected, or must not be).
- Fix a typo or an out-of-date example.

If you want to take an issue, comment on it first so two people do not do the same work. Ask questions in the issue: nobody minds.

## AI-assisted contributions

You may use AI tools. The same rules apply as for any contribution: you are responsible for the change, it must pass the checks above, and it must follow [AGENTS.md](AGENTS.md). That file holds the rules written for coding agents (code style, security rules, Definition of Done). Please read what the tool produced before you submit it, and do not paste in code you do not understand. Never give an AI tool a real API key or private text.

## License

Unless you explicitly state otherwise, any contribution you intentionally submit for inclusion in this project, as defined in the Apache-2.0 license, is dual licensed under MIT OR Apache-2.0, without any additional terms or conditions.
