# Releasing translate-kun

This is the maintainer checklist. Nothing here is automatic until the one-time setup is done. The release workflow is built with [cargo-dist](https://opensource.axo.dev/cargo-dist/). It also publishes to crates.io, by calling `publish-crates.yml` as a custom publish job.

## What a release produces

| Channel | How | Pre-release (`-rc`) | Stable tag |
|---|---|---|---|
| GitHub Release | `.github/workflows/release.yml` builds archives for 5 targets, with SHA-256 checksums, build provenance attestations and an SBOM | created as a **pre-release** | created as a normal release |
| Homebrew tap | the same workflow pushes the `kun` formula to `machngai/homebrew-tap` | **not published** | published |
| crates.io | the same workflow calls `.github/workflows/publish-crates.yml` after the GitHub Release exists | **not published** | `translate-kun-core`, then `translate-kun` |

cargo-dist skips every publish job (Homebrew and crates.io) when the tag has a pre-release suffix such as `-rc.1`.

Only the `translate-kun` package (binary `kun`) is distributed as an archive. `translate-kun-core` is a library and goes to crates.io only. There is no shell or PowerShell installer.

## 1. One-time setup (owner)

Do all of these **before the first tag, including the first release candidate**.

1. **Signing.** Set up SSH commit and tag signing so `git tag -s` works:

   ```sh
   git config --global gpg.format ssh
   git config --global user.signingkey ~/.ssh/id_ed25519.pub
   git config --global commit.gpgsign true
   git config --global tag.gpgsign true
   ```

   Add the same public key to GitHub as a **signing key** (Settings → SSH and GPG keys → New SSH key → Key type: Signing Key).
2. **Tag ruleset.** Settings → Rules → Rulesets → New tag ruleset. Target pattern `v*`, enforcement active, rules **Restrict deletions** and **Block force pushes** (also **Restrict updates**, if you want tags to be final). A pushed tag then cannot be moved to different code. Because release candidates are tagged too, a bad `-rc` tag cannot be deleted: you tag the next one (`-rc.2`).
3. **Create the tap repository.** Create `machngai/homebrew-tap` on GitHub: public, with a README. It can otherwise be empty. Homebrew maps `machngai/tap` to the repository `machngai/homebrew-tap`.
4. **Create the tap token.** In GitHub, go to Settings → Developer settings → Fine-grained personal access tokens. Give it:
   - Resource owner: `machngai`; repository access: **only** `machngai/homebrew-tap`;
   - Repository permissions: **Contents: read and write** (nothing else);
   - an expiry (for example 1 year; put a reminder in your calendar).

   Then add it as the repository secret `HOMEBREW_TAP_TOKEN` on `machngai/translate-kun` (Settings → Secrets and variables → Actions). When the token expires, the release workflow's Homebrew job fails: create a new token and replace the secret.
5. **crates.io account.** Log in to [crates.io](https://crates.io) with GitHub and turn on 2FA on your GitHub account.

The next two steps need the crates to exist on crates.io, so they come **after** the release candidate has passed (section 3), just before the stable tag:

6. **Publish the first versions by hand.** Trusted Publishing can only be configured on a crate that already exists, so the first upload is manual. Log in with `cargo login` (a token with only the `publish-new` and `publish-update` scopes and a short expiry; revoke it afterwards), then, from a clean checkout of the commit that sets the version to `0.1.0`:

   ```sh
   cargo publish -p translate-kun-core
   cargo publish -p translate-kun
   ```
7. **Configure Trusted Publishing on crates.io**, for **both** crates (crate page → Settings → Trusted Publishing → Add). Add **two** entries for each crate, because crates.io checks the *calling* workflow (the file that started the run), not the reusable workflow that does the publishing:

   | Repository owner | Repository | Workflow filename | Used when |
   |---|---|---|---|
   | `machngai` | `translate-kun` | `release.yml` | the normal path: a stable tag runs `release.yml`, which calls `publish-crates.yml` |
   | `machngai` | `translate-kun` | `publish-crates.yml` | the manual retry ("Run workflow") |

   Leave Environment empty. Note that the first stable release (`0.1.0`) is published by hand, so the workflow finds both crates already on crates.io and skips them: the Trusted Publishing setup is **first used for real by `0.1.1`**. Watch that release's `custom-publish-crates` job closely.

## 2. Every release

The tag is always `v` plus the exact version in `Cargo.toml`. A release PR sets that version, `dist plan` checks it, and only then do you tag.

1. **Prepare in a PR** (branch from `main`):
   - In `CHANGELOG.md`, move the entries from `## [Unreleased]` into a new `## [X.Y.Z] - YYYY-MM-DD` section, and update the link lines at the bottom.
   - Set `version` in the root `Cargo.toml` (`[workspace.package]`) to `X.Y.Z`, **and** the `version` of `translate-kun-core` under `[workspace.dependencies]` to the same value. Change both together: a pre-release version such as `0.1.0-rc.1` is not matched by a plain `"0.1.0"` requirement, so a mismatch makes `cargo` fail. Run `cargo check --workspace` so `Cargo.lock` follows.
   - Merge the PR after CI is green.
2. **Check the plan from the commit you are going to tag:**

   ```sh
   git checkout main
   git pull
   dist plan --tag vX.Y.Z
   ```

   It must list only `translate-kun` and say `announcing vX.Y.Z`. If it complains that nothing can be released, the version in `Cargo.toml` and the tag differ.
3. **Tag that commit** and push the tag:

   ```sh
   git tag -s vX.Y.Z -m "vX.Y.Z"
   git push origin vX.Y.Z
   ```

4. **Watch the `Release` workflow** in the Actions tab. For a stable tag the order is: `plan` → build jobs → `host` (creates the GitHub Release) → `publish-homebrew-formula` and `custom-publish-crates` → `announce`.
5. **Verify the artefacts** (below).

### Verify the artefacts

Download an archive and its checksum from the release page, then (use the real file name for your system):

```sh
gh attestation verify translate-kun-aarch64-apple-darwin.tar.xz \
  -R machngai/translate-kun \
  --signer-workflow machngai/translate-kun/.github/workflows/release.yml
shasum -a 256 -c translate-kun-aarch64-apple-darwin.tar.xz.sha256   # macOS; on Linux: sha256sum -c
```

Then check that:

- the release has an archive and an `.sha256` file for each of the 5 targets, plus `sha256.sum`, the SBOM (`translate-kun.cdx.xml`), the Homebrew formula `kun.rb` and `source.tar.gz`. (The SBOM upload could not be tested before the first tag: if `translate-kun.cdx.xml` is missing from the release, look at the `cargo-cyclonedx` step of the `build-global-artifacts` job.);
- an archive holds `kun`, `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`, `README.md` and `THIRD-PARTY-LICENSES.md`;
- `kun --version` prints the new version, and `cargo audit bin <path to kun>` works;
- `brew install machngai/tap/kun` and `cargo install translate-kun` install that version (stable releases only).

## 3. Release candidate flow

Run a release candidate before the first stable tag, and whenever the release tooling changes. A release candidate is a normal release with a pre-release version, so it goes through the same PR → plan → tag steps. **Do not tag `v0.1.0-rc.1` while `Cargo.toml` still says `0.1.0`**: `dist plan --tag v0.1.0-rc.1` fails with "This workspace doesn't have anything for dist to Release".

1. **RC PR.** Branch from `main`. Set the workspace `version` to `0.1.0-rc.1` **and** the `translate-kun-core` version under `[workspace.dependencies]` to `0.1.0-rc.1`. Run `cargo check --workspace` (updates `Cargo.lock`). Do not move the CHANGELOG entries yet. Merge it.
2. **Plan, then tag** (section 2, steps 2 and 3), with `v0.1.0-rc.1`:

   ```sh
   dist plan --tag v0.1.0-rc.1
   git tag -s v0.1.0-rc.1 -m "v0.1.0-rc.1"
   git push origin v0.1.0-rc.1
   ```

3. **What happens.** cargo-dist creates a GitHub **pre-release** and builds and attests all archives. It does **not** update the Homebrew tap and does **not** call `publish-crates.yml`. Verify the artefacts as in section 2.
4. **If something is wrong,** fix it on `main` in a PR that also bumps both versions to `0.1.0-rc.2`, then plan and tag `v0.1.0-rc.2`. Never move or reuse a tag.
5. **When a candidate is good,** do the one-time steps 6 and 7 of section 1 only after the stable PR is merged (they need the `0.1.0` commit), in this order:
   1. Final PR: CHANGELOG moved to `0.1.0`, both versions set to `0.1.0`. Merge it.
   2. Publish the two crates by hand (section 1, step 6) from that commit.
   3. Configure Trusted Publishing (section 1, step 7).
   4. `dist plan --tag v0.1.0`, then tag and push `v0.1.0`.

A pre-release stays on the Releases page. Delete the release by hand if you do not want it kept; the tag cannot be deleted (the tag ruleset blocks that, on purpose).

## How the release workflow is maintained

The release configuration is in `dist-workspace.toml`. `.github/workflows/release.yml` is generated by `dist generate` and then **edited by hand**. `allow-dirty = ["ci"]` tells `dist` to accept this, so `dist plan` does not notice when a hand edit is lost: only a diff or code review does.

### Hand edits to re-apply after dist generate

Each edit is marked `HAND EDIT <n>` in `release.yml`:

1. **Permissions.** The workflow default is `contents: read` (dist writes `contents: write` for the whole workflow), and only the `host` job, which creates the GitHub Release, has `contents: write`. `build-local-artifacts` has `attestations: write` and `id-token: write` for the build provenance attestation (that is dist's own setting), and `custom-publish-crates` has `id-token: write` and `contents: read`. Nothing else has `id-token`.
2. **The tag never goes into shell source.** dist writes `${{ github.ref_name }}` and a `tag-flag` output straight into `run:` scripts. A tag name can contain shell characters (`$`, `;`, backticks), so every use goes through `env: TAG` and is quoted: `dist build ${TAG:+"--tag=$TAG"}` and `gh release create "$TAG"`. The `plan` job chooses `dist host` or `dist plan` with an `if` on `$TAG`. The `Build artifacts` step runs with `shell: bash` on every runner (also Windows, so no PowerShell quoting is involved). The `tag-flag` output is removed.
3. **Homebrew commit author** is `github-actions[bot]`, not cargo-dist's default.
4. **cargo-auditable is pinned.** dist installs the *latest* cargo-auditable (`matrix.install_cargo_auditable.run`). The step is replaced by `taiki-e/install-action` with `tool: cargo-auditable@0.7.7` (prebuilt binaries exist for all 5 runners). Bump the version by hand.
5. **The custom publish job `custom-publish-crates`.** dist 0.34 writes `uses: $/.github/workflows/publish-crates.yml`, which is not a valid path: it is `uses: ./.github/workflows/publish-crates.yml`. The generated `secrets: inherit` is removed (publishing uses OIDC only and must not see `HOMEBREW_TAP_TOKEN`), and the permissions are `contents: read` and `id-token: write` (dist also adds `packages: write`, which is not needed).

To check that nothing was lost, run `dist generate` into a clean checkout with `allow-dirty = []` and `diff` the two files: only the five edits above should differ. Also check that every `uses:` line has a 40-character SHA.

### Action pins

Every GitHub Action in `release.yml` is pinned by full commit SHA with cargo-dist's own `[dist.github-action-commits]` table in `dist-workspace.toml`, so the pins survive a regenerate. Dependabot cannot update them (it only reads the workflow file, and `dist generate` would write the table again), so bump them by hand when you upgrade `dist`: resolve each SHA with `gh api repos/<owner>/<repo>/tags` and never guess one. The `taiki-e/install-action` steps in `.github/build-setup.yml` and in `release.yml` (cargo-auditable) are pinned by SHA too; Dependabot does not read `build-setup.yml` and can only rewrite `release.yml` until the next regenerate, so keep both in step with the pin in `ci.yml` by hand.

`dist` itself and `cargo-cyclonedx` are the exceptions: they are installed with their upstream installer scripts (`cargo-dist-installer.sh`, `cargo-cyclonedx-installer.sh`) from a fixed release **tag**, not a hash. That is how cargo-dist works, and there is no setting to change it.

### Regenerating release.yml

Do this only to upgrade `dist`, or to change the targets or installers:

1. `cargo install cargo-dist --locked`, set `cargo-dist-version` in `dist-workspace.toml`, set `allow-dirty = []`, and run `dist generate`.
2. Re-apply the five hand edits above, put `allow-dirty = ["ci"]` back, and check that every `uses:` line still has a 40-character SHA.
3. Run `dist plan`, and tag a release candidate to test the result.

### Third-party licences

Each archive holds `THIRD-PARTY-LICENSES.md`. It is not kept in the repository (it is in `.gitignore`): the build jobs generate it with `cargo-about` (`about.toml` and the template `about.hbs`, through `.github/build-setup.yml`), and `include` in `dist-workspace.toml` puts it in the archive. `cargo-about` 0.9.3 has no prebuilt binary for macOS Intel, so that one runner builds it from source (`cargo install cargo-about --version 0.9.3 --locked --features cli`); every other runner uses `taiki-e/install-action`. To try it locally:

```sh
cargo install cargo-about --locked --features cli
cargo about generate --locked --config about.toml about.hbs --output-file THIRD-PARTY-LICENSES.md
```

The `accepted` list in `about.toml` must stay equal to `[licenses] allow` in `deny.toml`. If a new dependency brings a new licence, add it to **both** after checking that it is permissive; otherwise `cargo about` fails and so does the release build. Do not commit the generated file.

## If something fails

- **Release workflow fails before the release exists:** fix it on `main` and tag the next `-rc` (or, for a stable tag that never produced a release, the next patch version). Do not move or reuse a tag.
- **Homebrew job fails (token expired or wrong scope):** replace `HOMEBREW_TAP_TOKEN`, then re-run the failed job from the Actions tab.
- **`custom-publish-crates` fails:** fix the cause (for example the Trusted Publishing entries in section 1, step 7), then re-run the failed job, or run `Publish crates` by hand (Actions → Publish crates → Run workflow, choosing the release tag in "Use workflow from"). The workflow refuses to run from a branch, from a pre-release tag, or when the tag does not match both crate versions or has no published GitHub Release. A crate version that is already on crates.io is skipped, so a rerun is safe.
- **A bad release was published:** do not delete the tag. Yank the crate versions on crates.io (`cargo yank --version X.Y.Z translate-kun`), mark the GitHub Release as a pre-release, and release `X.Y.Z+1`.
