# S18 — README overhaul with an architecture overview

## Goal

The owner wants a README that explains `kun` thoroughly for a first-time visitor: what it is, why it exists, every capability with clear examples, and a short look at how it works inside. It stays accurate, scannable and welcoming to contributors.

The architecture diagram is already in the branch:

- `docs/assets/architecture.svg`: a self-contained, dual-theme SVG (light and dark) exported from Archify. It is about 1770×552, so on GitHub it is scaled down; link it so readers can open it full size.
- `docs/assets/architecture.archify.json`: its source, with a file and line reference for every box, pinned to commit `c095511`.

Do **not** edit either file.

## Read first

- `AGENTS.md`, `GLOSSARY.md`
- The current `README.md`
- `docs/usage.md`, `docs/configuration.md`, `docs/privacy.md`, `docs/json-output.md`, `docs/architecture.md`, `SECURITY.md`, `CONTRIBUTING.md`, `CHANGELOG.md` (Unreleased holds the Usage Budget, which ships in 0.2.0)
- `docs/assets/architecture.archify.json`: the components and cards say what the diagram shows

## In scope

- `README.md`: the main work.
- `docs/architecture.md`: embed the same SVG near the top, with one short paragraph. Keep the rest as it is.
- `docs/development/slices/S18-readme.md`: this file; add it.
- `docs/development/slices/README.md`: the S18 row.
- `CHANGELOG.md`: one line under Unreleased → Documentation.
- `_typos.toml`, only if needed.

## README structure (suggested; adjust if something reads better)

1. **Title, one-line pitch, badges.** Keep the existing badges: CI, Scorecard and licence. Add a crates.io version badge (`https://img.shields.io/crates/v/translate-kun`) and a link to the latest release.
2. **Why `kun`**: 4–6 short bullets.
   - It is a translator in your terminal and pipes.
   - The Secret Guard stops API keys and tokens before they leave.
   - Protected Terms keep names exact.
   - Two Providers (Google Cloud Translation, Anthropic), with no silent fallback.
   - A Usage Budget for free tiers.
   - Script-friendly: `--json` and stable exit codes.
   - Verifiable releases.
3. **Demo**: a short, realistic terminal session in a code block: translate, pipe, Secret Guard redact, `--dry-run`, `--json`. Use only FAKE/EXAMPLE secrets, and make every command and output match real behaviour (check `docs/usage.md`).
4. **Install**: Homebrew, cargo, prebuilt binaries, plus the existing "Verify your download" subsection. Keep it accurate.
5. **Quick start (5 minutes)**:
   1. Get an API key (Google or Anthropic; one or two lines each, linking to the Provider consoles).
   2. `kun auth login google`
   3. First translation.
   4. Set a Language Pair in `~/.config/kun/config.toml`. Say that the file does not exist until you create it, or until `kun budget set` creates it.
6. **Usage guide**: the capabilities explained with examples. Keep each subsection short and link to `docs/usage.md` for every flag.
   - Languages: `:xx`, `xx:yy`, `-s`/`-t`, the "bare words are text" rule, `--` before text that starts with a reserved word (`auth`, `init`, `completions`, `budget`).
   - Input and output: arguments vs stdin (sensitive text goes through stdin, because of shell history and `ps`); files; the clipboard on macOS (`pbpaste | kun :th | pbcopy`).
   - Language Pair.
   - Protected Terms and style (style is Anthropic only).
   - Choosing a Provider (`--provider`, config); no automatic fallback.
   - Secret Guard: what it detects (a summary of the kinds), and ask/block/redact behaviour on a terminal vs a pipe; `--redact`, `--allow-secrets`, `force_redact`, `extra_patterns`.
   - `--dry-run`: see exactly what would leave.
   - **Usage Budget** (new in 0.2.0): `kun budget set/status/unset`, `[budget]` in the User Config; it counts the characters actually sent and reserves them before sending; it is local to this machine and approximate. Recommend pairing it with a Provider-side quota cap. Mark clearly that it is available from 0.2.0.
   - Project Config (`kun init`, `.kun.toml`): what it can and cannot do (it is untrusted; ownership check).
   - Shell completions.
   - Scripting: `--json` with a `jq` example; the exit code table (0–8, one line each).
7. **Configuration at a glance**: a compact annotated example User Config: `provider`, `pair`, `protected_terms`, `max_chars`, `[secret_guard]` and `[budget]`. Show where files live; link to `docs/configuration.md`.
8. **How it works**: embed `docs/assets/architecture.svg` (Markdown image, linked to the file so it opens full size). Under it, a numbered walk-through of one translation in 6–8 steps that matches the diagram: CLI → config → Secret Guard → Protected Terms → Usage Budget (reserve) → Provider adapter + Credentials → HTTPS to Google or Anthropic → restore terms → stdout or JSON. Then a short "Workspace layout" note (`crates/kun` = CLI adapter with terminal I/O and exit codes; `crates/kun-core` = logic with no terminal I/O) and links to `docs/architecture.md` and the ADRs.
9. **Privacy and security**: 3–5 bullets.
   - What leaves your machine (link to `docs/privacy.md`).
   - Where keys are stored.
   - No telemetry.
   - The audit, with its honest disclaimer: it was done by an AI model, not an independent firm.
   - The Scorecard badge.
   - How to report a vulnerability (`SECURITY.md`).
10. **Costs and limits**: Provider pricing is the Provider's (link the official pricing pages); the Google free tier (verify the figure in `docs/usage.md` "Limits, quotas and costs"; do not invent numbers); `max_chars`; the Usage Budget; exit 8.
11. **FAQ / Troubleshooting**: 5–8 entries:
    - `kun: command not found` (PATH after cargo install);
    - exit 4, no key;
    - the Secret Guard blocked my text;
    - `ignoring .kun.toml: owned by another user`;
    - the harmless checksum WARNING;
    - Keychain unavailable on Linux (the 0600 file fallback);
    - why my text started with `auth` and failed.
12. **Contributing**: a short welcome and the three DoD commands; link to `CONTRIBUTING.md` and `CODE_OF_CONDUCT.md`.
13. **Licence**: MIT OR Apache-2.0.

## Style rules

- English, plain and friendly. Short sentences; active voice; no marketing fluff.
- Every command and output must be correct for the current `main`. When unsure, check the code or the docs, or leave it out. Do not invent flags.
- Use only FAKE/EXAMPLE-looking secrets, and never a pattern that could be a real key.
- Use the GLOSSARY terms with their capitalisation: Provider, Secret Guard, Protected Terms, Language Pair, User Config, Project Config, Usage Budget, Target Language, Source Language.
- Keep the README a reasonable length (roughly 400–600 lines). Push long detail to `docs/` with links. Use a table of contents near the top.
- Relative links must resolve; check every one.
- No personal data. Org links use `machngai/translate-kun`.

## Out of scope

- Code changes.
- Regenerating the diagram.
- Changing `docs/usage.md` content, except to fix a fact you find wrong; report any such fix.

## Acceptance evidence

- The DoD in `AGENTS.md` passes (docs-only changes; still run it) and `typos` passes.
- A list of the README sections, and confirmation that every relative link resolves (for example with a small `grep` plus `test -e` loop).
- Any doc inconsistency you found and fixed or reported.
