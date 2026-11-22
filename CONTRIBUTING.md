# Contributing to translate-kun

Thank you for your interest. Bug reports, documentation fixes, tests and code are all welcome. This is a small personal project, so please be patient: reviews are best effort.

By taking part you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md). To report a security problem, use the process in [SECURITY.md](SECURITY.md), not a public issue.

## Prerequisites

- Rust **stable**, installed with [rustup](https://rustup.rs). The repository pins the channel in `rust-toolchain.toml`, and rustup picks it up automatically.
- `rustfmt` and `clippy` components (`rustup component add rustfmt clippy`).
- No API keys are needed to build or test. Tests never use the network.

## Getting the code

```sh
git clone https://github.com/machngai/translate-kun
cd translate-kun
```

## Build, test and lint

These are the checks every change must pass. CI runs the same commands: fmt, clippy and the other checks on Ubuntu, and the tests on Ubuntu, macOS and Windows.

```sh
cargo build --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets --features translate-kun/fake-provider -- -D warnings
cargo test --workspace --features translate-kun/fake-provider
```

The `translate-kun/fake-provider` feature lets tests use an offline fake Provider (`--provider fake`) and an in-memory credential store. It is for tests only.

Live tests, which call real Provider APIs, are optional and never run in CI:

```sh
KUN_GOOGLE_API_KEY=... cargo test -p kun-core --features live
```

## Fuzzing

The `fuzz/` directory holds [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) targets for the code that handles untrusted text. It is its own workspace and is not part of `cargo test --workspace`, so you only need it if you work on these areas.

| Target | What it checks |
|---|---|
| `secret_guard` | Secret Guard `scan` then `redact` on arbitrary UTF-8: no panic, and a redacted finding is gone from the output |
| `config_parse` | Arbitrary bytes as a Project Config and as a User Config: no panic, and a Project Config never carries credentials, an endpoint or `allow_secrets` |
| `protected_terms` | `protect` then `restore` gives back the original text (input is `text\|term\|term...`) |

Fuzzing needs a **nightly** toolchain and `cargo-fuzz`:

```sh
rustup toolchain install nightly
cargo install cargo-fuzz --locked

cargo +nightly fuzz run secret_guard -- -max_total_time=60
cargo +nightly fuzz run config_parse -- -max_total_time=60
cargo +nightly fuzz run protected_terms -- -max_total_time=60
```

Without `-max_total_time` a target runs until you stop it or it finds a crash. A crash is saved under `fuzz/artifacts/<target>/`: replay it with `cargo +nightly fuzz run <target> fuzz/artifacts/<target>/<file>`, then turn it into a normal unit test in `kun-core` and fix the bug.

The seed inputs in `fuzz/corpus/<target>/seed-*` are committed. Anything else the fuzzer writes into `fuzz/corpus/` is ignored by git. The Secret Guard seeds are fake secrets cut by the marker `<SPLIT>` (the target removes it before scanning), so no file in the repository contains a complete secret-looking string. Keep it that way when you add a seed. CI runs each target for 60 seconds every week (`.github/workflows/fuzz.yml`).

## Extra checks

CI also runs these. You do not need them for every change, but each one can be run locally.

| Check | What it does | Install | Run |
|---|---|---|---|
| Property tests | `proptest` runs a rule against many random inputs (Secret Guard, Protected Terms). Part of `cargo test`: nothing to install. | none | `cargo test -p translate-kun-core --test properties` |
| Coverage | `cargo-llvm-cov` prints which lines the tests run. Report only: no threshold, not a required check. | `rustup component add llvm-tools-preview` and `cargo install cargo-llvm-cov --locked` | `cargo llvm-cov --workspace --features translate-kun/fake-provider --summary-only` |
| Unused dependencies | `cargo-machete` finds dependencies that no code uses. | `cargo install cargo-machete --locked` | `cargo machete --with-metadata` |
| Spell check | `typos` finds common misspellings. Intentional words go in `_typos.toml`. | `cargo install typos-cli --locked` | `typos` |

Property tests run 64 cases per property by default. To try more: `PROPTEST_CASES=2000 cargo test -p translate-kun-core --test properties`. If a property fails, proptest prints a small failing input: turn it into a normal test and fix the bug.

## Mutation testing

[cargo-mutants](https://mutants.rs) changes the code in small ways (for example `<` to `<=`, or a function body to a default value) and runs the tests. A mutant that the tests do not notice ("missed") points at behaviour that no test checks. It is not part of CI, because a full run is slow. We ran it on the Secret Guard, the code where a missed bug is a leak:

```sh
cargo install cargo-mutants --locked

# One file, with a per-mutant time limit (seconds). Only the example tests
# run (`--test secret_guard`): a failing property test shrinks its input for
# minutes, which makes a mutation run crawl.
cargo mutants -p translate-kun-core --file crates/kun-core/src/secret_guard.rs --timeout 30 -- --test secret_guard

# Only list the mutants, without running them:
cargo mutants -p translate-kun-core --file crates/kun-core/src/secret_guard.rs --list
```

The results go to `mutants.out/` (do not commit it; or choose another place with `--output <dir>`); `missed.txt` lists the survivors. Add a test that fails for a missed mutant when the mutant changes behaviour that matters. Some mutants in `next_line` turn a loop into an endless one: they count as detected ("timeout"), but on some machines the hung test process is not killed and the run stalls. If that happens, stop the stray `secret_guard-*` process, or leave `next_line` out of the run with `--exclude-re next_line`. Do not chase equivalent mutants (the code behaves the same either way) or cosmetic ones (for example a `Display` text nobody parses).

## Releasing

Maintainers: the release steps are in [docs/development/releasing.md](docs/development/releasing.md).

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
- [ ] `cargo clippy --workspace --all-targets --features translate-kun/fake-provider -- -D warnings` passes.
- [ ] `cargo test --workspace --features translate-kun/fake-provider` passes.
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
