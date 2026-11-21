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
- A malicious repository: a cloned Project Config cannot hold credentials or endpoints and cannot weaken the Secret Guard ([ADR 0003](docs/adr/0003-project-config-is-untrusted.md)).

**Your part**

- Text you pass as arguments (`kun :th "my private note"`) is saved in your shell history and is visible to other local users in `ps` while `kun` runs. Send sensitive text through stdin instead: `kun :th < note.txt` or `pbpaste | kun :th`.

**Out of scope**

- Malware or another process running as the same user, a hostile operating system, attackers with debugger or memory access, and timing side channels.
- Secrets that the Secret Guard's patterns do not recognise. It is a safety net, not a guarantee.
- What a Provider does with your text once it is sent. Check the privacy terms of the Provider you choose.
- Denial of service against your own machine through very large input (there is a size limit, but it is not a security boundary).

If you are unsure whether something is in scope, report it anyway: a short private report is welcome.
