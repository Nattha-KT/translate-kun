## Summary

<!-- What does this change, and why? Keep it short. -->

## Linked issue

<!-- For example: Fixes #123. Write "none" if there is no issue. -->

## Checklist

- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --workspace --all-targets --features kun/fake-provider -- -D warnings` passes
- [ ] `cargo test --workspace --features kun/fake-provider` passes, and no test uses the network
- [ ] Docs are updated (`README.md`, `docs/`) if behaviour or flags changed
- [ ] `CHANGELOG.md` has an entry under `## [Unreleased]`
- [ ] No secrets, real API keys or personal data in code, fixtures or docs (test keys contain `EXAMPLE` or `FAKE`)
- [ ] Commit messages follow Conventional Commits
