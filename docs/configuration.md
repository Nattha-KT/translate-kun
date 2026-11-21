# Configuration

`kun` reads two optional TOML files. Neither is required: with no files, `kun` uses its built-in defaults and you give the Target Language on the command line.

| | Project Config | User Config |
|---|---|---|
| File | `.kun.toml` | `config.toml` |
| Location | The closest `.kun.toml`, found by walking up from the current directory to the filesystem root | `$XDG_CONFIG_HOME/kun/config.toml` if `XDG_CONFIG_HOME` is an absolute path, otherwise `~/.config/kun/config.toml` |
| Created by | `kun init` (a commented template) | You |
| Trust | **Untrusted** ([ADR 0003](adr/0003-project-config-is-untrusted.md)) | Trusted |
| Can hold credentials | No (hard error) | Yes, in `[credentials]` |

## Why the Project Config is untrusted

A `.kun.toml` lives in a directory tree that may have been cloned from someone else. If it could set an endpoint or a key, running `kun` inside a malicious repository could send your text and your API key to an attacker. So a Project Config may only choose among the built-in Providers and may only make the Secret Guard **stricter**. Anything that sends data somewhere new, or weakens a safety check, belongs in the User Config or on the command line. See [ADR 0003](adr/0003-project-config-is-untrusted.md).

## Precedence

For ordinary settings the order is:

> command-line flag > Project Config > User Config > built-in default

Some settings combine instead of replace:

| Setting | How the layers combine |
|---|---|
| `secret_guard.extra_patterns` | Joined: the User Config's patterns, then the Project Config's, with duplicates removed. A Project Config can add patterns but never remove them. |
| `secret_guard.force_redact` | On if **any** layer turns it on (`--redact`, Project Config or User Config). |
| `secret_guard.allow_secrets` | On only from `--allow-secrets` or the User Config. A Project Config can never turn it on. |
| `protected_terms` | The Project Config's list replaces the User Config's when it sets one (even an empty list). |
| `providers.<name>.endpoint` | From the User Config only. |

The Target Language has one more rule: an explicit `:xx` or `-t xx` wins, then a Language Pair, then `target`. See [usage.md](usage.md#source-language-and-target-language).

## Keys

Types: `string`, `integer`, `boolean`, `array`. "Both" means the key is allowed in both files.

| Key | Type | Default | Allowed in | Meaning |
|---|---|---|---|---|
| `target` | string | none | Both | Default Target Language, for example `"th"`. |
| `pair` | array of 2 strings | none | Both | Language Pair, for example `["en", "th"]`. The two codes must differ. |
| `provider` | string | `"google"` | Both | `"google"` or `"anthropic"`. |
| `protected_terms` | array of strings | `[]` | Both | Terms that must stay as written. See [usage.md](usage.md#protected-terms). |
| `style` | string | none | Both | A tone or style hint for the Anthropic Provider (it can change the wording only). |
| `max_chars` | integer ≥ 1 | `10000` | Both | Longest accepted input, in characters. |
| `timeout_secs` | integer ≥ 1 | `15` | Both | Provider request timeout in seconds. |
| `[secret_guard]` `extra_patterns` | array of regex strings | `[]` | Both | Extra patterns the Secret Guard treats as secrets. An invalid regex is a configuration error (exit 3). |
| `[secret_guard]` `force_redact` | boolean | `false` | Both | Always replace possible secrets with `[REDACTED]`. |
| `[secret_guard]` `allow_secrets` | boolean | `false` | **User only** | Send text even when a secret is found. |
| `[providers.anthropic]` `model` | string | `"claude-haiku-5-5"` | Both | Model used by the Anthropic Provider. |
| `[providers.google]` `model` | string | none | Both | Accepted but not used by the Google Provider. |
| `[providers.<name>]` `endpoint` | string | none | **User only** | A custom endpoint URL. See the note below. |
| `[credentials]` `google_api_key` | string | none | **User only** | Google API key (a fallback, see below). |
| `[credentials]` `anthropic_api_key` | string | none | **User only** | Anthropic API key (a fallback, see below). |

> **Current limitation.** Custom endpoints are not supported by any Provider yet. The key `providers.<name>.endpoint` is accepted and validated, but if it is set for the Provider you use, `kun` stops with exit 3 (`providers.<name>.endpoint is not supported yet; remove it to use the public API.`) instead of silently sending your text to the public API.

Settings with no CLI flag today: `pair`, `timeout_secs`, `protected_terms`, `style` and everything under `[secret_guard]` except `--redact` and `--allow-secrets`.

### User-Config-only keys, and why

These keys are rejected in a Project Config with a hard error (exit 3):

- **`[credentials]` and any key whose name contains `key`, `token`, `secret`, `password` or `credential`** (the table `[secret_guard]` is the one exception). A credential in a file you commit is a leak waiting to happen.
- **`endpoint`.** A repository must not be able to redirect your text and key to its own server.
- **`allow_secrets`.** A repository must not be able to switch off the Secret Guard.

The error names the file and the setting but never prints its value.

Unknown keys are also an error in both files, so a typo does not go unnoticed.

## Credentials

Where `kun` looks for an API key (first match wins):

1. **Environment variable**: `KUN_GOOGLE_API_KEY` or `ANTHROPIC_API_KEY`. Intended for CI and cron.
2. **OS keychain**: macOS Keychain, or Secret Service (libsecret) on Linux. Stored by `kun auth login <provider>`. This is the strongest option.
3. **User Config `[credentials]`**: a plain-text fallback for machines with no keychain.

```toml
# ~/.config/kun/config.toml  (run: chmod 600 ~/.config/kun/config.toml)
[credentials]
google_api_key = "your-key-here"
```

### The 0600 rule

The `[credentials]` table is used **only if the User Config file is readable by you alone** (mode `0600` or stricter). If it is readable by your group or by others:

- `kun` ignores the `[credentials]` values and prints a warning that says to run `chmod 600` on the file;
- `kun auth status` reports that the config-file credentials were ignored.

The rest of the file (target, provider and so on) is still used. File modes are checked on Unix only.

## Proxies

Proxies are set with environment variables, not in a config file. `kun` honours `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY` and `NO_PROXY` (and their lower-case forms). If more than one proxy variable is set, the first of `ALL_PROXY`, `HTTPS_PROXY`, `HTTP_PROXY` wins.

Provider requests are always HTTPS, so they go through the proxy with `CONNECT`: the proxy sees only the host name, and the API key and your text stay inside TLS. A proxy URL may hold a user name and password; `kun` never prints it.

## Example

A Project Config for a repository whose docs are translated between English and Thai:

```toml
# .kun.toml
pair = ["en", "th"]
protected_terms = ["translate-kun", "Cargo.toml"]
max_chars = 20000

[secret_guard]
extra_patterns = ["internal-[a-z0-9]{16}"]
force_redact = true
```

A User Config that picks a model and sets a personal default:

```toml
# ~/.config/kun/config.toml
provider = "anthropic"
timeout_secs = 30

[providers.anthropic]
model = "claude-haiku-5-5"
```
