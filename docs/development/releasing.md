# Releasing translate-kun

This is the maintainer checklist. Nothing here is automatic until the one-time setup is done. The release workflow is built with [cargo-dist](https://opensource.axo.dev/cargo-dist/); crates.io publishing is a separate workflow.

## What a release produces

| Channel | How | Pre-release (`-rc`) | Stable tag |
|---|---|---|---|
| GitHub Release | `.github/workflows/release.yml` builds archives for 5 targets, with SHA-256 checksums, build provenance attestations and an SBOM | created as a **pre-release** | created as a normal release |
| Homebrew tap | the same workflow pushes the `kun` formula to `machngai/homebrew-tap` | **not published** | published |
| crates.io | `.github/workflows/publish-crates.yml` runs when a release is **published** and is not a pre-release | **not published** | `translate-kun-core`, then `translate-kun` |

Only the `translate-kun` package (binary `kun`) is distributed as an archive. `translate-kun-core` is a library and goes to crates.io only. There is no shell or PowerShell installer.

## 1. One-time setup (owner)

Do these once, before the first tag.

1. **Create the tap repository.** Create `machngai/homebrew-tap` on GitHub: public, with a README. It can otherwise be empty. Homebrew maps `machngai/tap` to the repository `machngai/homebrew-tap`.
2. **Create the tap token.** In GitHub, go to Settings → Developer settings → Fine-grained personal access tokens. Give it:
   - Resource owner: `machngai`; repository access: **only** `machngai/homebrew-tap`;
   - Repository permissions: **Contents: read and write** (nothing else);
   - an expiry (for example 1 year; put a reminder in your calendar).

   Then add it as the repository secret `HOMEBREW_TAP_TOKEN` on `machngai/translate-kun` (Settings → Secrets and variables → Actions). When the token expires, the release workflow's Homebrew job fails: create a new token and replace the secret.
3. **crates.io account.** Log in to [crates.io](https://crates.io) with GitHub and turn on 2FA on your GitHub account.
4. **Publish the first versions by hand.** Trusted Publishing can only be configured on a crate that already exists, so the first upload is manual. Log in with `cargo login` (a short-lived or scoped token, and revoke it afterwards), then, from a clean checkout of the release commit:

   ```sh
   cargo publish -p translate-kun-core
   cargo publish -p translate-kun
   ```

   Do this for `0.1.0` only after the release candidate (section 3) has passed. After that, the workflow publishes every later version.
5. **Configure Trusted Publishing on crates.io**, for **both** crates (crate page → Settings → Trusted Publishing → Add):
   - Repository owner `machngai`, repository `translate-kun`;
   - Workflow filename `publish-crates.yml`;
   - Environment: leave empty.
6. **Signing.** Set up SSH commit and tag signing so `git tag -s` works:

   ```sh
   git config --global gpg.format ssh
   git config --global user.signingkey ~/.ssh/id_ed25519.pub
   git config --global commit.gpgsign true
   git config --global tag.gpgsign true
   ```

   Add the same public key to GitHub as a **signing key** (Settings → SSH and GPG keys → New SSH key → Key type: Signing Key).
7. **Tag ruleset.** Settings → Rules → Rulesets → New tag ruleset. Target pattern `v*`, enforcement active, rules **Restrict deletions** and **Block force pushes** (also **Restrict updates**, if you want tags to be final). A published tag then cannot be moved to different code.

## 2. Every release

1. **Prepare in a PR** (branch from `main`):
   - In `CHANGELOG.md`, move the entries from `## [Unreleased]` into a new `## [X.Y.Z] - YYYY-MM-DD` section, and update the link lines at the bottom.
   - Bump `version` in the root `Cargo.toml` (`[workspace.package]`) and the `version` of `translate-kun-core` in `[workspace.dependencies]`. Run `cargo check` so `Cargo.lock` follows.
   - Run `dist plan` (see below) and check that it still lists only `translate-kun`.
   - Merge the PR after CI is green.
2. **Tag the merge commit** and push the tag:

   ```sh
   git checkout main
   git pull
   git tag -s vX.Y.Z -m "vX.Y.Z"
   git push origin vX.Y.Z
   ```

   The tag must be exactly `v` plus the version in `Cargo.toml`.
3. **Watch the workflows** in the Actions tab: `Release` first, then (when the release is published) `Publish crates`.
4. **Verify the artefacts** (below).

### Verify the artefacts

Download an archive and its checksum from the release page, then:

```sh
gh attestation verify <archive> -R machngai/translate-kun
shasum -a 256 -c <archive>.sha256       # macOS; on Linux: sha256sum -c
```

Then check that:

- the release has an archive and an `.sha256` file for each of the 5 targets, plus `sha256.sum`, the SBOM (`translate-kun.cdx.xml`), the Homebrew formula `kun.rb` and `source.tar.gz`. (The SBOM upload could not be tested before the first tag: if `translate-kun.cdx.xml` is missing from the release, look at the `cargo-cyclonedx` step of the `build-global-artifacts` job.);
- an archive holds `kun`, `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`, `README.md` and `THIRD-PARTY-LICENSES.md`;
- `kun --version` prints the new version, and `cargo audit bin <path to kun>` works;
- `brew install machngai/tap/kun` and `cargo install translate-kun` install that version (stable releases only).

## 3. Release candidate flow

Run a release candidate before the first stable tag, and whenever the release tooling changes:

```sh
git tag -s v0.1.0-rc.1 -m "v0.1.0-rc.1"
git push origin v0.1.0-rc.1
```

A tag with a pre-release suffix (`-rc.1`) makes cargo-dist create a GitHub **pre-release**. It builds and attests all archives, but it does **not** update the Homebrew tap, and `publish-crates.yml` skips it. Verify the artefacts as in section 2. If something is wrong, fix it on `main`, then tag `v0.1.0-rc.2`. When the candidate is good, tag `v0.1.0`.

A pre-release stays on the Releases page. Delete it by hand if you do not want it kept (tag rules may block deleting the tag: that is intended).

## How the release workflow is maintained

The release configuration is in `dist-workspace.toml`. `.github/workflows/release.yml` is generated by `dist generate` and then **edited by hand**. `allow-dirty = ["ci"]` tells `dist` to accept this.

The hand edits are:

- Job permissions. The workflow default is `contents: read`, and only the `host` job (which creates the GitHub Release) has `contents: write`. `dist generate` would give the whole workflow `contents: write`. The `build-local-artifacts` job has `attestations: write` and `id-token: write` for the build provenance attestation, and nothing else has them.
- The Homebrew commit author is `github-actions[bot]`, not cargo-dist's default.

Every GitHub Action in `release.yml` is pinned by full commit SHA with cargo-dist's own `[dist.github-action-commits]` table in `dist-workspace.toml`, so the pins survive a regenerate. Dependabot cannot update them (it only reads the workflow file, and `dist generate` would write the table again), so bump them by hand when you upgrade `dist`: resolve each SHA with `gh api repos/<owner>/<repo>/tags` and never guess one. The `taiki-e/install-action` step in `.github/build-setup.yml` is pinned by SHA too; Dependabot does not read that file either, so keep it in step with the pin in `ci.yml` by hand.

Two things are pinned by version, not by hash: `dist` itself and `cargo-cyclonedx` are installed with their upstream installer scripts (`cargo-dist-installer.sh`, `cargo-cyclonedx-installer.sh`) at a fixed release. That is how cargo-dist works, and there is no setting to change it.

### Regenerating release.yml

Do this only to upgrade `dist`, or to change the targets or installers:

1. `cargo install cargo-dist --locked`, set `cargo-dist-version` in `dist-workspace.toml`, and run `dist generate`.
2. Re-apply the hand edits above (`git diff` shows what the regenerate undid), and check that every `uses:` line still has a 40-character SHA.
3. Run `dist plan` and tag a release candidate to test the result.

### Third-party licences

Each archive holds `THIRD-PARTY-LICENSES.md`. It is not kept in the repository: the build jobs generate it with `cargo-about` (`about.toml` and the template `about.hbs`, through `.github/build-setup.yml`), and `include` in `dist-workspace.toml` puts it in the archive. To try it locally:

```sh
cargo install cargo-about --locked --features cli
cargo about generate --locked --config about.toml about.hbs --output-file THIRD-PARTY-LICENSES.md
```

The `accepted` list in `about.toml` must stay equal to `[licenses] allow` in `deny.toml`. If a new dependency brings a new licence, add it to **both** after checking that it is permissive; otherwise `cargo about` fails and so does the release build. Do not commit the generated file.

## If something fails

- **Release workflow fails before the release exists:** fix it on `main` and push a new `-rc` tag. Do not move or reuse a tag.
- **Homebrew job fails (token expired or wrong scope):** replace `HOMEBREW_TAP_TOKEN`, then re-run the failed job from the Actions tab.
- **`Publish crates` fails:** fix the cause, then re-run the workflow (Actions → Publish crates → Run workflow, on the release tag). A crate version that is already on crates.io is skipped, so a rerun is safe.
- **A bad release was published:** do not delete the tag. Yank the crate versions on crates.io (`cargo yank --version X.Y.Z translate-kun`), mark the GitHub Release as a pre-release, and release `X.Y.Z+1`.
