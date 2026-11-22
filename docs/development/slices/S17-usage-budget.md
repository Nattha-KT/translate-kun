# S17 — Monthly usage budget (local, opt-in)

## Background

The owner wants `kun` itself to stop them going over a Provider's free tier, for example Google Cloud Translation's 500,000 characters per month. Provider-side quota caps still exist; this slice adds a **local** guard.

Decisions made by the owner:

- **Period:** one limit per **calendar month**, per Provider. There is no daily limit.
- **Opt-in:** there is no limit unless the user sets one.
- **The limit can be set two ways:** a CLI command, or the User Config.
- **No override:** once the budget is reached, `kun` refuses. To continue, the user raises or removes the limit. There is no `--ignore-budget` flag.

This becomes release **0.2.0**. Do not bump the version in this slice; the release PR does that.

## Read first

- `AGENTS.md`, `GLOSSARY.md`, `docs/adr/0003-*.md` (the Project Config is untrusted)
- `docs/usage.md` (exit codes; "Limits, quotas and costs"; Dry run; `kun auth`), `docs/configuration.md`, `docs/json-output.md`
- `crates/kun/src/app.rs` (the translate flow, the Language Pair second call, dry run), `args.rs`, `error.rs`, `settings.rs`, `auth.rs` (a subcommand to copy)
- `crates/kun-core/src/config/**`

## Skills

Use `tdd` (mandatory: the budget check is logic). Use `codebase-design` for the new module's interface. Use `domain-modeling` to add the term **Usage Budget** to `GLOSSARY.md`.

## Behaviour

1. **Configuration.** The budget is set in the **User Config only**, under `[budget]`:

   ```toml
   [budget]
   google_chars_per_month = 450000
   anthropic_chars_per_month = 50000
   ```

   - Each value is an integer ≥ 1. A missing key means no limit for that Provider.
   - A `[budget]` section in a **Project Config** is a configuration error (exit 3), with a fixed message, in the same way as other forbidden keys (ADR 0003). An untrusted repository must not be able to change or remove your spending guard.
2. **CLI.** Add a new subcommand, `kun budget`. Copy the structure of `kun auth`.
   - `kun budget` / `kun budget status`: for each Provider, show the limit (or "no limit"), the characters used this month, what remains, and the reset date (the first day of the next month, UTC). With `--json`, print one object; document its shape in `docs/json-output.md`.
   - `kun budget set <google|anthropic> <chars>`: write the value into the User Config `[budget]` section.
     - **Create the file and its directory if they are missing**, with file mode 0600 on Unix.
     - **Keep the rest of the file and its comments unchanged.** Use `toml_edit` for this (justify the dependency in your report; check whether it is already in the tree first). Replace a symlink the same way `kun init --force` does, and never write through it.
   - `kun budget unset <google|anthropic>`: remove that key.
   - `kun budget reset`: **not provided**. The counter resets each month by itself, and deleting the state file by hand is documented.
   - `budget` becomes a reserved word, like `auth`, `init` and `completions`: text that starts with it needs `--`. Update the help text and docs.
3. **What counts.**
   - Count the characters of the text **actually sent** to the Provider: after redaction and Protected Terms placeholders. Use the same unit as `max_chars` (`chars().count()`).
   - Count **each Provider call**. The Language Pair second call counts as well.
   - A call is **counted when it is made, and refunded if it fails** (the characters are reserved in the usage file before sending; see "reserve before sending" below). Document that a Provider may still bill some failures, and that a refund that cannot be written keeps the count too high, which is the safe direction.
   - Nothing is counted when the text is not sent: the Source Language equals the Target Language, `--dry-run`, Secret Guard block, size limit.
4. **The check, before sending.**
   - If `used_this_month + chars_to_send > limit` for the Provider in use, refuse before any network call.
   - With a Language Pair, check the first call before sending it, and check the second call again before sending it. If the second call would go over, fail and report it. The first call has already happened and stays counted.
   - Exit code **8** (rate limited or quota exceeded).
   - The message has a fixed wording with numbers only, for example: `monthly usage budget for google reached: 448210 of 450000 characters used; this text needs 3120. It resets on 2026-12-01 (UTC). Raise it with: kun budget set google <chars>`. It never repeats the text.
   - With `--json`, the error goes to stderr as usual.
5. **Usage state file.**
   - Location: `$XDG_STATE_HOME/kun/usage.json`, otherwise `~/.local/state/kun/usage.json`. Pass the environment into the function, as `user_config_path` does, so it stays testable.
   - **Only counters**, per Provider: `{"version":1,"month":"2026-11","google":12345,"anthropic":0}`. No text, no hashes, no timestamps beyond the month.
   - **Write:** mode 0600 on Unix, written atomically (a temp file in the same directory, then rename). Create the directory if it is missing.
   - **A different month:** when the stored month is not the current UTC month, start from 0.
   - **A missing file** means 0 used.
   - **An unreadable or corrupt file:**
     - with a budget set, fail closed: exit 3 with a fixed message telling the user to delete the file;
     - with no budget set, ignore it and do not touch it.
   - **Only touch the file when a budget is set for the Provider in use.** With no budget, `kun` does not read or write the state file at all, so the default behaviour is unchanged.
   - **Reserve before sending (added after review).** Before each Provider call: load the file, check `used + n <= limit` (exit 8 if not), then write `used + n` with the real atomic temp-and-rename. If that write fails for any reason, exit 3 with a fixed message and send nothing; there is no separate probe. If the call then fails, write `used` back (a refund); if the refund fails, keep the over-count, print a short fixed warning and return the Provider's own error. With a Language Pair each call reserves on its own.
   - **Not a regular file (added after review).** Look at the path with `symlink_metadata`. A symlink (dangling or not), a directory or any other non-regular file, with a budget set, is exit 3 with a fixed message ("usage file is not a regular file; delete it"). It is never followed and never treated as missing.
   - **Concurrency:** two `kun` processes running at the same moment may lose an increment. Accept this and document it; no file locking.
6. **Dry run.** When a budget is set, the dry-run report adds one line, `budget: <used> of <limit> used this month; this run needs <n>`, plus whether a real run would be refused. A dry run never changes the counter.
7. **Docs.**
   - `docs/usage.md`:
     - a "Usage budget" section;
     - the `kun budget` commands;
     - exit code 8 now also covers a local budget;
     - "Limits, quotas and costs": recommend a local budget **and** a Provider-side quota cap, and explain that the local budget only counts this machine and is approximate. Google counts characters its own way, and Anthropic bills tokens, not characters.
   - `docs/configuration.md`: the `[budget]` keys, that they are User Config only, and the state file location.
   - `README.md`: one example in Quick start: `kun budget set google 450000`.
   - `CHANGELOG.md`, under Unreleased → Added.
   - `GLOSSARY.md`: Usage Budget.
   - `docs/development/slices/S17-usage-budget.md` (this file; add it) and `docs/development/slices/README.md` (S17 row).

## Tests (at least)

- The pure budget decision: under the limit, exactly at the limit, over it, and no limit.
- Month rollover resets the counter.
- A missing file means 0.
- A corrupt file with a budget gives exit 3; with no budget it is ignored and left untouched.
- The state file is 0600 on Unix and never contains the translated text. Use a fake Provider and assert the file content.
- **CLI tests with the fake Provider:**
  - `budget set`, then `status`;
  - a translation increments the counter;
  - a translation over budget gives exit 8 and makes no Provider call;
  - the Language Pair counts both calls;
  - a dry run does not count;
  - `budget set` keeps comments in an existing User Config;
  - `[budget]` in a Project Config gives exit 3.
- Tests use a temp HOME / XDG directory and never touch the real home directory.

## Out of scope

- A daily limit.
- Token-based counting for Anthropic.
- Syncing usage across machines.
- File locking.
- A version bump.

## Acceptance evidence

- The Definition of Done in `AGENTS.md` passes. Report the test count.
- Example terminal output of `kun budget status`, and of a refused translation.
- Any new dependency, with its justification.
