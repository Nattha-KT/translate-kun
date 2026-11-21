# S15 — Pre-publish hardening

## Background

Before the repository goes public, the coordinator compared it against NIST SSDF (SP 800-218), the OpenSSF Scorecard checks and the OpenSSF OSPS Baseline. The owner approved the items below. Keep everything proportionate (see "Security in proportion" in `AGENTS.md`).

`deny.toml` already sets `unmaintained = "workspace"`, so no cargo-deny change is needed.

## Read first

- `AGENTS.md`
- `SECURITY.md` (threat model), `docs/adr/0003-*.md` (the Project Config is untrusted)
- `crates/kun-core/src/config/discover.rs`, `crates/kun-core/src/config/load.rs`
- `crates/kun/src/settings.rs` (`load_settings`, where the Project Config is found and loaded)
- `.github/workflows/*.yml`, `.github/dependabot.yml`
- `README.md`, `docs/privacy.md`, `docs/configuration.md`, `CHANGELOG.md`

## Skills

Use `tdd` for item 3 (the only code change).

## In scope

- `.github/workflows/*.yml`, `.github/dependabot.yml`, `.github/CODEOWNERS` (new), `.github/workflows/codeql.yml` (new)
- `crates/kun-core/src/config/**`, `crates/kun/src/settings.rs` and their tests; `crates/kun-core/Cargo.toml` and the root `Cargo.toml` only if item 3 needs a dependency
- `README.md`, `docs/privacy.md`, `docs/configuration.md`, `SECURITY.md`, `CONTRIBUTING.md`, `CHANGELOG.md`
- `docs/development/slices/README.md`: add the S15 row; remove N-1 and N-8 from the deferred list

## Items

1. **Pin GitHub Actions by full commit SHA** (Scorecard *Pinned-Dependencies*, audit N-8).
   - Every `uses:` in every workflow becomes `owner/repo@<40-hex-sha> # vX.Y.Z`.
   - Resolve each SHA with `gh api repos/<owner>/<repo>/commits/<tag> --jq .sha`, or `git ls-remote https://github.com/<owner>/<repo> refs/tags/<tag>` (for annotated tags, use the peeled `^{}` line). Never guess a SHA.
   - For `dtolnay/rust-toolchain`, the ref is a branch (`1.89.0`, `stable`, `nightly`). Pin the branch's current commit and pass the toolchain explicitly with `with: toolchain: <x>`, because a pinned commit no longer implies the toolchain.
   - Check that `.github/dependabot.yml` has the `github-actions` ecosystem, so Dependabot keeps the SHAs updated.
   - List every action and its SHA in your report.
2. **`.github/CODEOWNERS`.** Make `@Nattha-KT` the owner of `*`, and list the sensitive paths explicitly for clarity:
   - `crates/kun-core/src/secret_guard*`
   - `crates/kun-core/src/credentials*`
   - `crates/kun-core/src/config/`
   - `crates/kun-core/src/provider/`
   - `.github/`
   - `deny.toml`
   - `SECURITY.md`
3. **Ownership check on a discovered Project Config** (audit N-1; same idea as git's `safe.directory`, CVE-2022-24765).
   - On Unix, when the `.kun.toml` found by walking up from the start directory is owned by a different user than the one running `kun`:
     - **skip it** (do not load it);
     - add a warning to `Loaded.warnings`, the same path the CLI already prints for warnings, such as: `ignoring <path>: owned by another user`;
     - continue with the User Config and defaults. This is not an error, so normal use is never blocked.
   - A file owned by the current user is loaded as before.
   - Outside Unix there is no check.
   - Get the current uid without `unsafe` (the workspace forbids it). Options:
     - the `rustix` crate with only the `process` feature, as a `[target.'cfg(unix)'.dependencies]` entry; it is already in the tree on Linux;
     - any other safe approach you find simpler.
     - Justify any new dependency in your report.
   - Put the uid comparison in `kun-core`, as a small pure function that is easy to test (for example `is_owned_by(file_uid, current_uid) -> bool`), and the decision to skip in the loader. Test both outcomes. Tests must not need root or `chown`: test the pure function, plus a real-file test for the same-owner path.
   - Document it in `docs/configuration.md` and `SECURITY.md`, in one or two sentences each.
4. **CodeQL (SAST)** (Scorecard *SAST*).
   - Add `.github/workflows/codeql.yml` with `github/codeql-action` (`init` + `analyze`) for `language: rust`, using `build-mode: none`.
   - Triggers: push to main, pull requests to main, and weekly.
   - Permissions: `contents: read` at the top level, and `security-events: write` on the job.
   - Pin it by SHA like item 1.
   - First check that CodeQL supports Rust in the action version you pin (read the codeql-action README or its changelog). If it does not, skip this item and say so.
5. **Provider data notice.**
   - In `README.md`, add a short "Where your text goes" note near the top-level usage: the text, after the Secret Guard, is sent to the Provider you chose (Google Cloud Translation or Anthropic) and is handled under that Provider's terms and privacy policy. Do not use `kun` for text that your organisation does not allow to be sent to that Provider. Link to `docs/privacy.md`.
   - Check that `docs/privacy.md` says the same; add a sentence if it does not.
6. **CHANGELOG.** Add entries under Unreleased: Security for items 1, 3 and 4, and Documentation for item 5.

## Out of scope

- Anything in S8 (release, SBOM, attestations, signing).
- N-2 and N-6, which remain deferred.
- Repository settings on GitHub (the owner does those).

## Acceptance evidence

- The Definition of Done in `AGENTS.md` passes. Report the test count.
- `grep -rn "uses:" .github/workflows` shows only SHA-pinned refs, each with a version comment.
- Report one line per item, plus the action → SHA table.
