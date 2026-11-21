# S3 — Provider trait + fake Provider

## Goal

Define the core translation types and the `Provider` abstraction that every real Provider (Google in S4, Anthropic in S6) will implement, plus a deterministic fake Provider for tests.

## Read first

- `AGENTS.md` (all)
- `docs/requirements.md` §4 (Providers), §3, the "Target Language resolution" part only, for vocabulary
- `GLOSSARY.md`: Provider, Source Language, Target Language

## Skill

Use the `codebase-design` skill to keep the interface small and deep. Use `tdd` for the behaviour.

## In scope

- `crates/kun-core/src/provider.rs` (or `provider/` as a module directory, e.g. `provider/fake.rs`)
- `crates/kun-core/src/language.rs`: the language-code type
- `crates/kun-core/src/lib.rs`: add **only** `pub mod provider;` and `pub mod language;`
- `crates/kun-core/Cargo.toml`: add `thiserror` if needed

## Design requirements

1. `LanguageCode`: a validated newtype.
   - Accepts BCP-47-ish codes such as `en`, `th`, `ja`, `zh-TW`, `pt-BR`.
   - Normalises case: lowercase language, uppercase region.
   - Rejects empty strings, digits, and anything longer than reasonable.
   - Provide `FromStr` and `Display`.
2. `TranslationRequest { text: String, source: Option<LanguageCode>, target: LanguageCode }`. `None` source means auto-detect.
3. `Translation { text: String, detected_source: Option<LanguageCode>, provider: &'static str }`.
4. `trait Provider`:
   - `fn name(&self) -> &'static str`
   - `fn translate(&self, request: &TranslationRequest) -> Result<Translation, ProviderError>`
   - Synchronous only: **no async** (AGENTS.md).
   - Must be object-safe, so the CLI can hold a `Box<dyn Provider>`.
5. `ProviderError`, using `thiserror`, with variants the CLI can map to exit codes later:
   - `MissingCredentials`
   - `Authentication`
   - `Network`
   - `RateLimited`
   - `UnsupportedLanguage`
   - `InvalidResponse`
   - `Other`

   **Error messages must never include credentials or the full source text.**
6. `FakeProvider`, deterministic and offline:
   - Returns the text wrapped as `"[<target>] <text>"`.
   - Detects the source as `th` if the text contains Thai script (U+0E00–U+0E7F), otherwise `en`, when the request source is `None`.
   - Can be configured to return a chosen `ProviderError`, so later slices can test error paths.
   - It is a public module so the `kun` crate's tests can use it.

## Out of scope

- Real HTTP, credentials, the Language Pair logic (S5), Protected Terms (S6), and the Secret Guard (S1, being built in parallel).
- Do not touch `secret_guard`.

## Acceptance evidence

- The Definition of Done in `AGENTS.md` passes.
- Report the public API (signatures only), the test count, and one sentence on why the trait is shaped this way.
