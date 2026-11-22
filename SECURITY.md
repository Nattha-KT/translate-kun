# Security Policy

## Supported versions

`translate-kun` is pre-1.0. Only the **latest `0.x` release** receives security fixes. Please update before reporting, to check the problem still exists.

## Reporting a vulnerability

Please report vulnerabilities **privately**, never in a public issue, pull request or discussion.

1. Open the repository on GitHub and go to the **Security** tab.
2. Choose **Report a vulnerability** (GitHub private vulnerability reporting).
3. Describe the problem, the version (`kun --version`), your OS, and the steps to reproduce it.

**Do not include real API keys or private text in your report.** Use an obviously fake key (for example one that contains `EXAMPLE` or `FAKE`).

### What to expect

This is a personal project, so the response is **best effort**. You can normally expect an acknowledgement within about a week and a fix or a decision as soon as the maintainer has time. You will be told what was decided. If you want credit in the release notes, say so in your report.

## Threat model

`kun` is a personal CLI translator, not a vault. The security work is in proportion to that.

**In scope: what we try to prevent**

- API keys leaking to third parties, logs, terminal output, shell history or other local users. Keys are never accepted as flags, never printed, and never included in `--json` output, error messages or `Debug` output. A key in the User Config file is used only if the file is `0600`.
- The user's text leaking to a service they did not choose: there is no automatic Provider fallback, no telemetry, no cache and no history of source or translated text.
- Credentials in the text being sent. The Secret Guard checks outgoing text first (prompt on a terminal, block in scripts).
- Terminal escape sequences in a translation. On a terminal, control characters other than newline and tab are stripped from the Provider's output.
- A malicious repository: a cloned Project Config cannot hold credentials or endpoints and cannot weaken the Secret Guard ([ADR 0003](docs/adr/0003-project-config-is-untrusted.md)). On Unix, a discovered `.kun.toml` that is owned by a different user than the one running `kun` is skipped with a warning (the same idea as git's `safe.directory`), so a file planted in a shared directory such as `/tmp` is not loaded.

**Your part**

- Text you pass as arguments (`kun :th "my private note"`) is saved in your shell history and is visible to other local users in `ps` while `kun` runs. Send sensitive text through stdin instead: `kun :th < note.txt` or `pbpaste | kun :th`.

**Out of scope**

- Malware or another process running as the same user, a hostile operating system, attackers with debugger or memory access, and timing side channels.
- Secrets that the Secret Guard's patterns do not recognise. It is a safety net, not a guarantee.
- What a Provider does with your text once it is sent. Check the privacy terms of the Provider you choose.
- Denial of service against your own machine through very large input (there is a size limit, but it is not a security boundary).

If you are unsure whether something is in scope, report it anyway: a short private report is welcome.

## Security assurance

What has been checked, and how much weight to give it.

**Audit.** Before the first release the code was reviewed in a read-only security audit. The public summary, with every finding and its status, is in [docs/security/audit-2026-11.md](docs/security/audit-2026-11.md).

> **Disclaimer: the audit was performed by an AI model (Claude Opus 5.5), not by an independent professional security firm, and the project is pre-1.0 personal software.** It is useful evidence, not a guarantee. Please read the code and the [privacy statement](docs/privacy.md) yourself if the stakes are high for you.

**Continuous checks.** These run automatically, so the assurance does not depend on one review:

- **cargo-deny** runs in CI on every push and pull request. It checks dependencies for known advisories, banned crates, licences and untrusted sources (`deny.toml`).
- **Dependabot** opens weekly pull requests for Cargo dependencies and GitHub Actions. Every GitHub Action in the workflows is pinned by full commit SHA, and Dependabot keeps the pins up to date.
- **CodeQL** runs static analysis on the Rust code for every push and pull request to `main`, and weekly.
- **Fuzzing.** Three [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) targets (`secret_guard`, `config_parse`, `protected_terms`) run weekly in CI for a short time. See [CONTRIBUTING.md](CONTRIBUTING.md#fuzzing) to run them locally.
- **OpenSSF Scorecard** scores the repository's security practices automatically and publishes the result. The score is shown as a badge in the [README](README.md).

**Verifiable releases.** Release archives are built by the `release.yml` workflow on GitHub-hosted runners, not on a developer's machine. Each one has a SHA-256 checksum and a build provenance attestation (GitHub Artifact Attestations, SLSA-style). To check a download:

```sh
# Provenance: needs the GitHub CLI (`gh`). It proves which repository,
# workflow and commit built the file. --signer-workflow makes it check that the
# signer is this repository's release workflow. (Use the file name you
# downloaded; this is the macOS Apple silicon archive.)
gh attestation verify translate-kun-aarch64-apple-darwin.tar.xz \
  -R machngai/translate-kun \
  --signer-workflow machngai/translate-kun/.github/workflows/release.yml

# Checksum: an `.sha256` file is published next to each archive.
sha256sum -c translate-kun-aarch64-apple-darwin.tar.xz.sha256       # Linux
shasum -a 256 -c translate-kun-aarch64-apple-darwin.tar.xz.sha256   # macOS
```

Every release also carries a CycloneDX **SBOM** (software bill of materials), and the binaries are built with `cargo-auditable`, which embeds the exact dependency list in the binary. To check an installed `kun` against the RustSec advisory database:

```sh
cargo install cargo-audit --locked
cargo audit bin "$(which kun)"
```

The attestation shows where a file came from. It does not show that the code is free of bugs. Third-party licences of the built-in dependencies are in `THIRD-PARTY-LICENSES.md`, inside every archive. The maintainer's release steps are in [docs/development/releasing.md](docs/development/releasing.md).

**Privacy.** What data leaves your machine, and how to verify it yourself with `kun --dry-run`, is in [docs/privacy.md](docs/privacy.md).
