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

Rules:

- Errors are not JSON. They go to stderr with the usual exit codes, and stdout stays empty.
- Prompts (the Secret Guard question on a terminal) still use the terminal.
- `kun init` and `kun auth ...` do not use `--json`.
