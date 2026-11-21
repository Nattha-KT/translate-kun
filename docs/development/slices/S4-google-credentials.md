# S4 — Credentials + Google Cloud Translation Provider

## Goal

Resolve a Provider credential safely (env var → OS keychain → User Config file), and implement the Google Cloud Translation Provider on top of the `Provider` trait. Everything lives in `kun-core`. The CLI wiring (`kun auth`, the provider factory) comes in a later slice.

## Read first

- `AGENTS.md` (all, especially the rule on leaks through `Debug`, `Display` and errors)
- `docs/requirements.md` §4 (Providers), §6 (Credentials), §10 (proxy and timeout)
- `docs/adr/0001-official-providers-no-scraping.md`, `docs/adr/0003-project-config-is-untrusted.md`
- Existing code: `crates/kun-core/src/provider.rs` (trait, `ProviderError`, `NetworkReason`), `crates/kun-core/src/config.rs`

## Skills

Use `tdd` for every behaviour. Use `codebase-design` for the credential types.

## In scope

- `crates/kun-core/src/credentials.rs` (new)
- `crates/kun-core/src/provider/google.rs` (new) and one `pub mod google;` line in `provider.rs`
- `crates/kun-core/src/config.rs`: only the change in item 3 below
- `crates/kun-core/src/lib.rs`: add **only** `pub mod credentials;`
- `crates/kun-core/Cargo.toml` and `Cargo.lock`. Allowed new dependencies: `ureq` (blocking HTTP, rustls TLS, JSON feature), `serde_json`, `keyring`. Add a `live` cargo feature for live tests.
- Tests under `crates/kun-core/tests/`

## Behaviour

1. **`Secret` type.** A newtype around `String` that holds a credential.
   - `Debug` and `Display` print `<redacted>`.
   - Not `Clone` unless needed.
   - `fn expose(&self) -> &str` is the only way to read it. Name the method so every use is obvious in review.
   - Test it with `{:?}` and `{:#?}`.

2. **Credential lookup.** `resolve_credential(provider, env: &dyn Fn(&str) -> Option<String>, store: &dyn CredentialStore, user_file: Option<&FileCredentials>) -> Result<Option<Secret>, CredentialError>`
   - Lookup order:
     1. env var (`KUN_GOOGLE_API_KEY` for Google, `ANTHROPIC_API_KEY` for Anthropic)
     2. the store
     3. the User Config `[credentials]` table
   - Empty or whitespace-only values count as absent.
   - Pass the env lookup in as a closure, so tests never touch the real environment.

3. **`[credentials]` in config.**
   - Extend `config.rs` so a `[credentials]` table with `google_api_key` and `anthropic_api_key` is accepted in the **User Config only**.
   - It stays a hard `ForbiddenInProjectConfig` error in a Project Config. Keep the existing tests green and add one for each direction.
   - `FileConfig`'s manual `Debug` must show `credentials: <redacted>`.

4. **`CredentialStore` trait.** `get`, `set` and `delete` per provider.
   - `KeyringStore`: the real implementation using the `keyring` crate, with service `translate-kun` and account = the provider name.
   - `MemoryStore`: an in-memory implementation for tests.
   - Unit tests use `MemoryStore` only. **Never touch the real keychain in tests.**

5. **Google Provider.** `GoogleProvider::new(key: Secret, timeout: Duration)` implements `Provider`.
   - Request: `POST https://translation.googleapis.com/language/translate/v2`, with JSON body `{ "q", "target", "source"?, "format": "text" }`.
   - Send the key in the `X-goog-api-key` header, **never in the URL**, so it cannot leak through URL-bearing error messages or proxies' logs.
   - Response: read `data.translations[0].translatedText` and `detectedSourceLanguage` (parse it into `LanguageCode`; ignore it if invalid).
   - Map statuses to `ProviderError`:
     - 401/403 → `Authentication`
     - 429 → `RateLimited`
     - 400 that mentions an unsupported language → `UnsupportedLanguage(target)`
     - other 4xx/5xx → `Network(NetworkReason::Status(code))`
     - timeouts and connect failures → `Timeout` / `Connect`
     - TLS failures → `Tls`
     - malformed JSON → `InvalidResponse`
   - **No `ProviderError` may contain the key, the URL with any query, the source text, or the response body.**
   - Honour `HTTPS_PROXY` and `NO_PROXY` via ureq's proxy-from-env support, and the timeout.
   - Split the pure parts into testable functions: building the request body, parsing a response body, and mapping a status code. Unit-test them with canned JSON. The real HTTP call is covered by **one** test behind `--features live`, which reads the key from `KUN_GOOGLE_API_KEY` and is skipped when it is absent.

## Out of scope

- `kun auth login` and any prompting (S7).
- Choosing the provider in the CLI (S5 and S7).
- The Anthropic Provider (S6).
- Retries and caching.

## Acceptance evidence

- The Definition of Done in `AGENTS.md` passes. Live tests are **not** run in CI.
- Report the public API (signatures only), the test count, and the exact status-to-error mapping table.
