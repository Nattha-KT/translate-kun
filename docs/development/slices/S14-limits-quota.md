# S14 — Rate limits, quotas and output limits

## Problem (found by the coordinator in code review)

| Situation | What users get today | Problem |
|---|---|---|
| Google quota exhausted or billing off (403 `quotaExceeded`, `dailyLimitExceeded`, `billingNotEnabled`, `accessNotConfigured`; or 429 `RESOURCE_EXHAUSTED`) | "the provider rejected the credentials" | Misleading: the user thinks the key is wrong |
| Anthropic credit balance too low (400 `invalid_request_error`, message about credit balance) or a billing error | "network error: HTTP status 400" | Misleading |
| Anthropic output hits `max_tokens` (4096) on long EN→TH input. Thai uses more tokens per character, and `max_chars` allows 10,000 | "response could not be understood" | A real bug: long translations fail |
| A rate limit with a `Retry-After` header | A generic rate-limit message | The user cannot tell how long to wait |

All of these exit with 5, so scripts cannot tell "wait and retry" from other failures.

## Read first

- `AGENTS.md`
- `crates/kun-core/src/provider.rs`, `provider/google.rs`, `provider/anthropic.rs`
- `crates/kun/src/error.rs`
- `docs/usage.md` (exit codes), `README.md` (exit codes), `docs/json-output.md`

## Skills

Use `tdd`.

## In scope

- `crates/kun-core/src/provider*` and their tests
- `crates/kun/src/error.rs` and the CLI tests
- `docs/usage.md`, `README.md`, `CHANGELOG.md`, `docs/architecture.md` (only if the error list is described there)

## Behaviour

1. **`ProviderError` changes.** Keep every message a fixed string with no Provider text, per AGENTS.md.
   - `RateLimited { retry_after_secs: Option<u64> }`: Display is "the provider is rate limiting requests" plus ", retry in N seconds" when known.
   - `QuotaExceeded`: "the provider quota, credit or billing limit was reached; check your provider account". This new variant covers quota, credit and billing.
   - `OutputTruncated`: "the translation was too long for one request; split the text or lower max_chars".
   - The enum is `#[non_exhaustive]` already; update the match arms in `crates/kun`.
2. **Google mapping.** On 403 or 429, parse `error.errors[].reason` and `error.status` from the JSON body. The body is read only for these statuses, size-capped like today.
   - `rateLimitExceeded`, `userRateLimitExceeded`, or 429 without a quota reason → `RateLimited`.
   - `quotaExceeded`, `dailyLimitExceeded`, `billingNotEnabled`, `accessNotConfigured`, or status `RESOURCE_EXHAUSTED` with a quota reason → `QuotaExceeded`.
   - A plain 403 with no recognised reason stays `Authentication`.
   - Read `Retry-After` (seconds form only) into `retry_after_secs`.
3. **Anthropic mapping.**
   - Parse `error.type`. `rate_limit_error` (429) and `overloaded_error` (529) → `RateLimited`, with `retry-after` read if present.
   - A 400 or 402 whose `error.type` or message indicates credit or billing (`billing_error`, or a message containing "credit balance") → `QuotaExceeded`.
   - On a successful response with `stop_reason == "max_tokens"` → `OutputTruncated`, even if the JSON happens to parse.
   - Raise `MAX_TOKENS` to 16000. That is safe for non-streaming requests.
4. **Exit codes.** Add **exit code 8** = "rate limited or quota exceeded; retrying later or checking the provider account may help". Map `RateLimited` and `QuotaExceeded` to 8; `OutputTruncated` and the other Provider errors stay 5.
   - Update every exit-code table: README, usage.md, and the `--help` text if it lists codes.
5. **Dry-run wording nit (from S13a).** In dry-run mode the redaction warning must not say "before sending". Use "would be replaced with [REDACTED]".
6. **Docs.** Add a "Limits, quotas and costs" section to `docs/usage.md`:
   - the free tiers exist, but check the providers' pricing pages
   - `max_chars` bounds the cost per call
   - no automatic retries, by design; scripts can retry on exit 8
   - link the providers' quota and console pages (official URLs)

## Tests (no network)

- A canned Google 403 `quotaExceeded` body → `QuotaExceeded` → exit 8.
- Google 403 `rateLimitExceeded` → `RateLimited`. A 429 with `Retry-After: 30` → the message contains "30 seconds".
- An Anthropic credit-balance 400 → `QuotaExceeded`; 529 → `RateLimited`.
- An Anthropic `stop_reason: max_tokens` with a truncated body → `OutputTruncated`.
- `MAX_TOKENS` is 16000 in the request body.
- Every new error's Display/Debug contains no body text (the AGENTS.md rule).

## Acceptance evidence

- The Definition of Done in `AGENTS.md` passes.
- Report the mapping tables for both Providers, the test count and the docs changed.
