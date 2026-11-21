# S1 — Secret Guard (core, test-first)

## Goal

Pure detection and redaction of credentials in outgoing text, plus the policy decision of what to do about them. No terminal I/O: prompting the user is the CLI's job in a later slice.

## Read first

- `AGENTS.md` (all)
- `docs/requirements.md` §7 (Secret Guard) and §5, the Project Config rows about the Secret Guard
- `docs/adr/0003-project-config-is-untrusted.md`
- `GLOSSARY.md`: Secret Guard

## Mandatory skill

Use the `tdd` skill: write one failing test, make it pass, refactor, then the next behaviour.

## In scope

- `crates/kun-core/src/secret_guard.rs` (or `secret_guard/` as a module directory)
- `crates/kun-core/src/lib.rs`: add **only** `pub mod secret_guard;`
- `crates/kun-core/Cargo.toml`: add `regex` and `thiserror`
- `crates/kun-core/tests/secret_guard.rs` (optional integration tests)

## Behaviour to implement

1. `scan(text) -> Vec<Finding>`. A `Finding` holds a `kind` (e.g. `AwsAccessKey`, `GitHubToken`, `AnthropicApiKey`, `OpenAiApiKey`, `GoogleApiKey`, `SlackToken`, `PrivateKeyBlock`, `Jwt`, `Custom`) and the byte range of the match.
   - **`Finding`'s `Debug` and `Display` must never contain the matched secret text.** Add a test that proves this.
2. Built-in, high-precision patterns:
   - AWS access key id (`AKIA`/`ASIA` + 16 chars)
   - GitHub tokens (`ghp_`, `gho_`, `ghu_`, `ghs_`, `ghr_`, `github_pat_`)
   - Anthropic (`sk-ant-…`)
   - OpenAI (`sk-…` / `sk-proj-…`), without colliding with Anthropic: an Anthropic key must be reported once, as Anthropic
   - Google API key (`AIza` + 35)
   - Slack (`xox[abprs]-…`)
   - PEM private key blocks (`-----BEGIN … PRIVATE KEY-----`)
   - JWT (three base64url segments, the first starting `eyJ`)
   - No entropy heuristics in v1.
3. A `SecretGuard` value built from the built-in patterns plus **extra** patterns: `SecretGuard::new(extra: &[String]) -> Result<Self, SecretGuardError>`.
   - An invalid extra regex is an error, not a panic.
   - There is deliberately **no API to remove or disable a built-in pattern** (ADR 0003).
4. `redact(text, &findings) -> String`: replaces each finding with `[REDACTED]`. Overlapping or adjacent findings must be handled correctly.
5. Policy decision, as a pure function:
   - `decide(findings: &[Finding], policy: GuardPolicy, interactive: bool) -> Decision`
   - `GuardPolicy { allow_secrets: bool, force_redact: bool }`
   - `Decision` is one of `Send`, `Redact`, `AskUser`, `Block`
   - Rules, per §7:
     - No findings → `Send`.
     - `force_redact` → `Redact`.
     - `allow_secrets` → `Send`.
     - Interactive → `AskUser`.
     - Non-interactive → `Block`.
6. Tests must include:
   - **must-detect** samples for each kind
   - **must-not-detect** samples, e.g. the word "skeleton", `sk-` in prose, a git SHA, a UUID, a base64 image fragment, the string `AKIA` alone

   All fixtures must be obviously fake: build them by concatenating parts, or include `EXAMPLE`/`FAKE`, so that no real-looking secret sits in the repo as a single literal.

## Out of scope

- Prompting the user, CLI flags, and reading config files.
- Rust edition and lint changes.

## Acceptance evidence

- The Definition of Done in `AGENTS.md` passes.
- Report the test count, the list of detected kinds, and any pattern you deliberately left out, with the reason.
