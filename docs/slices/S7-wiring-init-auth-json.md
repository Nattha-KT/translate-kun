# S7 — Real Provider wiring, `kun auth`, `kun init`, `--json`

## Goal

Make `kun` usable for real. Wire the Google Provider with credential lookup, add `kun auth` to store keys in the keychain, add `kun init` to create a Project Config, and implement `--json` output. Work in the `kun` crate only.

## Read first

- `AGENTS.md` (all, including **Security in proportion**)
- `docs/requirements.md` §3 (commands), §6 (Credentials), §9 (output, `--json`, exit codes)
- Existing code: `crates/kun/src/*` (from S5), `crates/kun-core/src/credentials.rs`, `provider/google.rs`, `config.rs`

## Skills

Use `tdd`.

## In scope

- `crates/kun/**` (sources and tests), `crates/kun/Cargo.toml`, `Cargo.lock`
- Allowed new dependency: `rpassword`, for hidden key input. `serde` and `serde_json` are allowed in `kun` for `--json`.
- Do **not** touch `crates/kun-core/**`. S6 is working there in parallel. If you need a core change, stop and report it.

## Behaviour

1. **Provider wiring.** In `build_provider`, `google` resolves its credential with `resolve_credential(Google, real env, &KeyringStore, user_config_credentials)` and builds `GoogleProvider` using the configured timeout.
   - No credential → exit 4 with the message: `No Google API key found. Run "kun auth login google" or set KUN_GOOGLE_API_KEY.`
   - If the User Config reports `credentials_ignored_insecure`, print a one-line warning to stderr: `~/.config/kun/config.toml is readable by others; its [credentials] were ignored. Run: chmod 600 <path>`.
   - `anthropic` stays "not wired yet" (exit 1). S6 wires it after both slices merge.

2. **`kun auth login <provider>`**
   - On a terminal, read the key with `rpassword` (hidden input). Otherwise read one line from stdin, so that `echo $KEY | kun auth login google` works in scripts.
   - Trim the key. Reject an empty key (exit 2). Store it with `KeyringStore`.
   - If no keychain is available, print a clear message suggesting the env var or a 0600 User Config `[credentials]` entry, then exit 3. Do not write files automatically.

   **`kun auth logout <provider>`** deletes the key from the keychain.

   **`kun auth status`** shows, for each provider, where its key would come from: `env`, `keychain`, `config file` or `none`. **It never prints the key itself**, not even a prefix.

3. **`kun init`**
   - Writes `.kun.toml` in the current directory as a commented template, with a short commented example of each Project Config field from S2: `target`, `pair`, `provider`, `protected_terms`, `style`, `max_chars`, `timeout_secs`, `[secret_guard] extra_patterns` / `force_redact`.
   - Add a comment saying credentials and endpoints are not allowed in this file.
   - Refuse to overwrite an existing file (exit 2) unless `--force` is given.
   - Test that the generated file parses with the S2 loader without errors.

4. **`--json`.** Write to stdout a single JSON object:
   ```json
   {"schema_version": 1, "text": "…", "source": "en" | null, "target": "th", "provider": "google", "warnings": ["…"]}
   ```
   - Warnings move into `warnings` instead of stderr. Errors still go to stderr, with the same exit codes.
   - `schema_version` is a public contract (ADR 0002). Document the schema in a short `docs/json-output.md`, which is in scope.

5. **Follow-up from S5.** Do not print the "args and stdin" warning when stdin is `/dev/null` or an empty regular file. Check this with file metadata; no reading or waiting.

## Tests

- Use `MemoryStore`, temp HOME and XDG dirs, and no network.
- Missing credential → exit 4 and the message above.
- `auth status` output never contains a fake key placed in env or config.
- `auth login` from piped stdin stores the key. Test this with an injectable store; do not touch the real keychain in tests.
- `init` creates the file, refuses to overwrite it, and `--force` works.
- The JSON shape is checked with the fake provider.

## Out of scope

- The Anthropic wiring (after S6), release packaging (S8), and DeepL or Ollama.

## Acceptance evidence

- The Definition of Done in `AGENTS.md` passes.
- Report `kun --help` and `kun auth --help` output, the test count, and the JSON schema.
