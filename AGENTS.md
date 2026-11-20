# AGENTS.md — rules for every agent working on translate-kun

`kun` is a Rust CLI translator. Read these before touching code:

- `docs/requirements.md`: what to build. Your slice spec names the sections that matter; read only those.
- `GLOSSARY.md`: canonical terms. Use them in code, comments and commit messages.
- `docs/adr/`: decisions you must not silently reverse. If you have to contradict one, stop and say so in your report.
- `docs/slices/<your-slice>.md`: your task, scope and acceptance criteria.

## Environment

- Rust comes from Homebrew rustup.
  - **Claude Code workers**: `.claude/settings.json` puts `/opt/homebrew/opt/rustup/bin` on `PATH`, so just run `cargo ...`. Don't `export PATH=...`: it triggers a permission prompt.
  - **Codex / other agents**: prefix each command with `PATH=/opt/homebrew/opt/rustup/bin:$PATH`. The bare absolute cargo path fails, because cargo cannot find `rustc`.
- Read files with the Read tool, not long `grep`/`head` shell chains.
- Create and edit files with the Write and Edit tools, one file per call. **Never create files via bash heredocs or `cat >`**: every such command needs a human approval. Use Bash only for `cargo` and `git`, **one plain command per call**: no `;`, `&&` chains or pipes into other tools, because each extra command can trigger an approval prompt.
- Toolchain: stable (rustc 1.100). `clippy` and `rustfmt` are installed.

## Code rules

- The repo owner is **new to Rust**. Write simple, idiomatic Rust:
  - owned types over lifetimes where cost is negligible
  - no async
  - no macros of your own
  - no clever generics
  - `anyhow` in the binary, `thiserror` in `kun-core`
- Workspace layout: `crates/kun-core` (logic, no terminal I/O) and `crates/kun` (CLI Adapter). Terminal I/O, prompts and process exit codes live only in `crates/kun`.
- **No network in tests.** Use the fake Provider. Live tests go behind `--features live` only.
- **Never** commit credentials, real API keys, or `.env` files. Test fixtures that look like secrets must be obviously fake (e.g. contain `EXAMPLE` or `FAKE`) and still exercise the detector.
- Never accept API keys via CLI flags. Never print, log or serialise a credential.
- **No secret leaks through `Debug`, `Display` or errors.** This is the most frequent review finding so far (S1, S2, S3).
  - Any type or error that can hold user-supplied free text (input text, config strings, regex patterns, HTTP bodies, URLs) must not show that text in `Debug`, `Display` or `Error::source()`.
  - Use fixed messages and structured, non-sensitive fields (enums, codes, indices, byte ranges), or write a manual `Debug` that prints `<redacted>`.
  - Add a test that formats with `{:?}` and `{:#?}` and asserts a fake key is absent.
- **Security in proportion.** The owner explicitly asked that security not be taken to an extreme. `kun` is a personal CLI translator, not a vault.
  - **Threat model:** stop credentials and the user's text from leaking to third parties, logs, terminal output, shell history or other local users. That is what matters.
  - **Out of scope:** malware running as the same user, a hostile OS, attackers with debugger access, memory scraping, timing side channels and similar threats. Do not design for them.
  - **Usability counts.** Do not break a normal workflow (e.g. fallback to the 0600 User Config when no keychain exists, running from cron or CI) to close a theoretical gap. If a fix costs real usability for a small risk reduction, say so and let the coordinator decide.
  - Prefer the simplest fix that closes a realistic leak. No extra crates, wrappers or ceremony "just in case".
- Edit only the files your slice spec lists as in scope. If something outside scope must change, stop and report it.
- Do not add dependencies beyond those your slice spec allows without saying why in your report.

## Definition of Done (every slice)

1. `cargo fmt --all --check` passes.
2. `cargo clippy --workspace --all-targets -- -D warnings` passes.
3. `cargo test --workspace --features kun/fake-provider` passes. Clippy runs with the same feature.
4. Code uses `GLOSSARY.md` terms and does not contradict any ADR.
5. Changes are committed on your worktree branch with a Conventional Commit message (`feat:`, `fix:`, `chore:`, `test:`, `docs:`).

## Report format (keep it short)

Reports go to the coordinator. Write them **in Thai**; keep code, commands and identifiers in English.

```
Outcome: done | blocked | partial
Branch / commit: <branch> <sha>
Evidence: last lines of fmt / clippy / test output (pass counts)
Changed files: <list>
Open issues: <list or "none">
Rust concepts used (3–5 bullets, Thai, for the owner to learn from)
```

## Skills to use

Skills from mattpocock-skills are installed for both Claude Code and Codex. Use them:

| Role | When | Skill |
|---|---|---|
| Implementer | Any slice with logic (S1–S7); **mandatory** for S1 Secret Guard | `tdd` (red → green → refactor, one behaviour at a time) |
| Implementer | Designing a public interface (e.g. the Provider trait in S3, config types in S2) | `codebase-design` |
| Implementer | A failing test or bug you can't explain quickly | `diagnosing-bugs` |
| Reviewer (Codex) | Every slice | `code-review`: Standards = `AGENTS.md`; Spec = the slice spec + named `requirements.md` sections |
| Anyone | A new domain term or a decision worth an ADR | `domain-modeling` (update `GLOSSARY.md` / `docs/adr/`) |

Orca coordination (`orchestration` skill) is handled by the coordinator. Workers only report back with `worker_done` / `ask` as their task preamble says.

## Token discipline

- Read only the requirement sections named in your spec; don't explore the whole repo.
- Don't paste large files or full diffs into reports. Summarise.
- Reviewers: review the diff against the spec + named requirement sections only.

## Reviewer severity (Codex)

- **blocker**: a realistic path by which a credential or the user's text reaches a third party, a log, terminal output or another local user; a crash or data loss; a spec rule broken in a way users will hit.
- **should-fix**: a wrong behaviour a user may hit; a spec deviation.
- **nit**: theoretical, rare or cosmetic issues; trade-offs (convenience vs. hardening) belong here with a one-line note, not as blockers.
- Do not raise the same class of issue again once it is fixed, unless the fix itself is wrong.
