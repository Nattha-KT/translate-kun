# AGENTS.md — rules for every agent working on translate-kun

`kun` is a Rust CLI translator. Read these before touching code:

- `docs/requirements.md`: what to build. Your slice spec names the sections that matter; read only those.
- `GLOSSARY.md`: canonical terms. Use them in code, comments and commit messages.
- `docs/adr/`: decisions you must not silently reverse. If you have to contradict one, stop and say so in your report.
- `docs/slices/<your-slice>.md`: your task, scope and acceptance criteria.

## Environment

- The Rust toolchain is installed via Homebrew rustup but may not be on `PATH`. Call cargo by absolute path: `/opt/homebrew/opt/rustup/bin/cargo` (the rustup proxy finds rustc, clippy and rustfmt itself). **Don't `export PATH=...`**: it triggers a permission prompt for every worker.
- Read files with the Read tool, not long `grep`/`head` shell chains.
- Create and edit files with the Write and Edit tools, one file per call. **Never create files via bash heredocs or `cat >`**: every such command needs a human approval. Use Bash only for `cargo` and `git`.
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
- Edit only the files your slice spec lists as in scope. If something outside scope must change, stop and report it.
- Do not add dependencies beyond those your slice spec allows without saying why in your report.

## Definition of Done (every slice)

1. `cargo fmt --all --check` passes.
2. `cargo clippy --workspace --all-targets -- -D warnings` passes.
3. `cargo test --workspace` passes.
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

## Token discipline

- Read only the requirement sections named in your spec; don't explore the whole repo.
- Don't paste large files or full diffs into reports. Summarise.
- Reviewers: review the diff against the spec + named requirement sections only.
