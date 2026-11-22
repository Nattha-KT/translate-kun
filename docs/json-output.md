# `--json` output

`kun --json ...` prints one JSON object on stdout (one line, ending with a newline). The object is a public contract (ADR 0002): fields are only added in a compatible way, and a breaking change bumps `schema_version`.

```json
{"schema_version":1,"text":"สวัสดี","source":"en","target":"th","provider":"google","warnings":[]}
```

| Field | Type | Meaning |
|---|---|---|
| `schema_version` | number | Always `1` for this schema. |
| `text` | string | The translation (or the input unchanged, when it was already in the Target Language). |
| `source` | string or `null` | The Source Language: the one the Provider detected, else the one given with `-s` / `xx:yy`. `null` when unknown. |
| `target` | string | The Target Language that was used. |
| `provider` | string | The Provider that produced `text`, e.g. `google`. |
| `warnings` | array of strings | Messages that would otherwise be printed on stderr as `warning: ...` (for example "the text is already in the Target Language"). Empty when there are none. |

## Dry run

`kun --dry-run --json ...` prints one object that describes the request instead of a translation. Nothing is sent.

```json
{"schema_version":1,"dry_run":true,"provider":"google","host":"translation.googleapis.com","model":null,"source":"en","target":"th","pair":null,"second_call_possible":false,"would_send":true,"credentials":"env","secret_guard":{"decision":"redact","findings":[{"kind":"AWS access key","start":10,"end":30}]},"text":"my key is [REDACTED] ok","budget":null,"warnings":["possible secrets would be replaced with [REDACTED]"]}
```

| Field | Type | Meaning |
|---|---|---|
| `schema_version` | number | `1`, as above. |
| `dry_run` | boolean | Always `true`. A translation object has no such field. |
| `provider` | string | The Provider that would be used. |
| `host` | string or `null` | The host that would be contacted (`translation.googleapis.com` or `api.anthropic.com`). |
| `model` | string or `null` | The model. Anthropic only. |
| `source` | string or `null` | The Source Language, or `null` when the Provider detects it. |
| `target` | string or `null` | The Target Language. `null` when a Language Pair is used. |
| `pair` | array of 2 strings or `null` | The Language Pair, when one is used. |
| `second_call_possible` | boolean | `true` for a Language Pair with a detected Source Language: the text may be sent a second time. |
| `would_send` | boolean | `false` when a real run makes no request (the Source Language is already the Target Language). |
| `credentials` | string | Where the key would come from: `env`, `keychain`, `config file` or `none`. Never the key. |
| `secret_guard.decision` | string | `send`, `redact` or `ask` (a real run would ask first; `text` is what would be sent on "yes"). |
| `secret_guard.findings` | array | One object per finding: `kind`, and `start` / `end` as a byte range in the original input. Never the matched value. |
| `text` | string or `null` | The exact text that would be sent, after redaction and Protected Terms placeholders. `null` when `would_send` is `false`. |
| `budget` | object or `null` | The [usage budget](usage.md#usage-budget) check. `null` when no limit is set for the Provider. Otherwise `limit` (the monthly limit), `used` (characters counted this month), `needs` (characters of the first call; `0` when `would_send` is `false`) and `would_refuse` (boolean: a real run would be refused with exit 8). All numbers; never text. |
| `warnings` | array of strings | As above. |

The decision `block` does not appear: a dry run that a real run would block fails with exit code 6 and an error on stderr, and stdout stays empty. Fields may be added later without changing `schema_version`.

## `kun budget status`

`kun budget status --json` (or `kun --json budget`) prints one object with the monthly [usage budget](usage.md#usage-budget) of each Provider.

```json
{"schema_version":1,"month":"2026-11","resets_on":"2026-12-01","providers":{"google":{"limit":450000,"used":448210,"remaining":1790},"anthropic":{"limit":null,"used":0,"remaining":null}}}
```

| Field | Type | Meaning |
|---|---|---|
| `schema_version` | number | `1`, as above. |
| `month` | string | The UTC month the counters belong to, `YYYY-MM`. |
| `resets_on` | string | The date the counters reset (the 1st of the next month, UTC), `YYYY-MM-DD`. |
| `providers.<name>.limit` | number or `null` | The monthly limit in characters. `null` = no limit. |
| `providers.<name>.used` | number | Characters counted this month. |
| `providers.<name>.remaining` | number or `null` | `limit` minus `used`, never below 0. `null` when there is no limit. |

`<name>` is `google` or `anthropic`. Fields may be added later without changing `schema_version`.

Rules:

- Errors are not JSON. They go to stderr with the usual exit codes, and stdout stays empty.
- Prompts (the Secret Guard question on a terminal) still use the terminal.
- `kun init`, `kun auth ...` and `kun budget set | unset` do not use `--json`.
