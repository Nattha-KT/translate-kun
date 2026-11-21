# S11a — CLI: apply Protected Terms and style, add shell completions

## Goal

Close a feature gap found in S9: the core library supports `protected_terms` and `style`, but the CLI never passes them. Also add shell completions, which users of production CLIs expect.

## Read first

- `AGENTS.md`
- `docs/requirements.md` §4 (style as data), §8 (Protected Terms)
- `docs/usage.md` and `docs/configuration.md` (they currently say "accepted but not yet applied")
- `crates/kun/src/app.rs`, `providers.rs`, `args.rs`; `crates/kun-core/src/translate.rs` (`translate_with_terms`); `provider/anthropic.rs` (how style is passed)

## In scope

- `crates/kun/**`
- `docs/usage.md`, `docs/configuration.md` and `CHANGELOG.md`: update only the parts about these features.
- Allowed new dependency: `clap_complete`, added via `[workspace.dependencies]`.
- Do **not** edit `crates/kun-core/**`. S11b is restructuring it in parallel. If the core API is missing something you need, stop and report it.

## Behaviour

1. **Protected Terms.** The CLI translation path calls the core function that applies `effective.protected_terms` (`translate_with_terms`, or whatever the core exposes). This covers the Language Pair path too.
   - If a placeholder was lost, print a warning on stderr, or put it in `warnings` when `--json` is set. The translation still succeeds.
2. **Style.** When the Provider is Anthropic, pass `effective.style` to `AnthropicProvider` the way the core expects (constructor or builder, as it exists today). Google ignores style; do not warn about that.
3. **`kun completions <shell>`** (bash, zsh, fish, powershell, elvish) prints a completion script to stdout using `clap_complete`.
   - It is a subcommand, like `auth` and `init`; the existing `--` rule for text that starts with a subcommand name still applies.
   - Document the install lines for each shell in `docs/usage.md`.

## Tests (no network, fake provider)

- With `protected_terms = ["translate-kun"]` in a temp `.kun.toml`, `kun --provider fake :th "use translate-kun now"` keeps `translate-kun` verbatim in the output.
- The same works on the Language Pair path.
- `kun completions bash` exits 0 and its output contains `kun`.
- For style: a unit test checks that the Anthropic construction path receives the style. Test at the CLI boundary only; no network.

## Acceptance evidence

- The Definition of Done in `AGENTS.md` passes.
- Report the changed files, the test count, and which docs lines changed.
