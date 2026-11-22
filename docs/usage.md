# Usage

This page lists every command and flag of `kun`. For settings in config files, see [configuration.md](configuration.md).

```text
kun [OPTIONS] [:xx | xx:yy] [TEXT]...
kun init [--force]
kun auth login <google|anthropic>
kun auth logout <google|anthropic>
kun auth status
kun budget [status]
kun budget set <google|anthropic> <chars>
kun budget unset <google|anthropic>
kun completions <bash|zsh|fish|powershell|elvish>
```

Run `kun --help` for the built-in help.

## Translating text

```sh
kun "hello"                 # Target Language from a config file
kun :th "hello"             # ":" prefix = Target Language
kun en:th "hello"           # Source Language en, Target Language th
kun -s en -t th "hello"     # long form, good for scripts
kun I love it               # translates "I love it"
```

All text arguments are joined with single spaces, so quotes are optional for short text.

### Language codes

A language code is `xx` or `xx-YY` (two or three letters, then an optional two-letter region). Case does not matter: `zh-tw` is read as `zh-TW`.

A language is recognised **only** in these forms:

- the first argument written as `:xx` or `xx:yy`;
- the `-s` / `--source` and `-t` / `--target` flags.

A bare word is always text, so `kun I love it` never reads `it` as Italian. An argument such as `to: me` (with a space) is text too.

You cannot give a language both ways. `kun :th -t fr "hi"` is a usage error (exit 2).

If the text itself starts with `auth`, `budget`, `init` or `completions`, put `--` before it, because those words are subcommands:

```sh
kun :th -- auth is great
```

### Source Language and Target Language

- The **Source Language** is detected by the Provider unless you give it with `-s` or `xx:yy`.
- The **Target Language** is chosen in this order (first match wins):
  1. `:xx` or `-t xx`;
  2. the **Language Pair**, if one is configured (see below);
  3. `target` from a config file;
  4. otherwise an error: no Target Language (exit 2).

If the detected Source Language is the same as the Target Language, `kun` prints the input unchanged and adds a warning.

### Language Pair

A Language Pair is two languages that `kun` switches between. Set it in a config file:

```toml
pair = ["en", "th"]
```

With this pair:

- text detected as `en` is translated to `th`;
- text detected as `th` is translated to `en`;
- text in any other language is translated to the first language (`en`).

An explicit `:xx` or `-t xx` always wins over the pair.

## Options

| Option | Meaning |
|---|---|
| `-s, --source <LANG>` | Source Language, for example `en`. Default: detected. |
| `-t, --target <LANG>` | Target Language, for example `th`. |
| `--provider <NAME>` | `google` (default) or `anthropic`. Overrides the config. |
| `--json` | Print one JSON object on stdout. See [json-output.md](json-output.md). |
| `--max-chars <N>` | Longest accepted input in characters (default 10000). Must be at least 1. |
| `--allow-secrets` | Send the text even if the Secret Guard finds a possible secret. |
| `--redact` | Replace possible secrets with `[REDACTED]` and continue. |
| `--dry-run` | Show exactly what would be sent, and send nothing. See [Dry run](#dry-run). |
| `-h, --help` | Show help. |
| `-V, --version` | Show the version. |

There is deliberately **no flag for API keys**. They would end up in your shell history and in `ps` output.

## Input: arguments and stdin

- **Sensitive text goes through stdin.** Text given as arguments is saved in your shell history and is visible to other local users in `ps` while `kun` runs. For anything private, pipe it in or redirect a file instead: `kun :th < notes.txt`.
- **Text arguments win.** If you give text as arguments, stdin is not read. If stdin is also piped or redirected, `kun` prints a warning on stderr and uses the arguments.
- **No arguments, piped stdin.** The text is read from stdin:

  ```sh
  echo "hello" | kun :th
  kun :th < README.md > README.th.md
  ```

- **No arguments, terminal stdin.** There is nothing to translate, so `kun` prints the help on stderr and exits with code 2.
- Stdin must be valid UTF-8 (otherwise exit 2).
- **Size limit.** The limit is 10,000 characters by default. Longer input is rejected with exit 7. `kun` does not split long text. Raise the limit with `--max-chars` or `max_chars` in a config file.
- Whitespace-only text counts as no text (exit 2).

## Output

- **stdout** holds the translated text only, followed by a newline. When stdout is a terminal, control characters other than newline and tab are removed from the translation, so a Provider cannot send terminal escape sequences. Pipes, redirects and `--json` get the text unchanged.
- **stderr** holds diagnostics: the Provider used, the detected Source Language, warnings, and the Secret Guard prompt.
- A closed pipe (`kun ... | head`) is not an error.
- With `--json`, stdout holds one JSON object and warnings go inside it. Errors are never JSON: they go to stderr and stdout stays empty.

Example:

```sh
$ kun :th "hello"
provider: google, detected source: en      # stderr
สวัสดี                                      # stdout
```

## Providers

| Provider | Name | Credential |
|---|---|---|
| Google Cloud Translation (default) | `google` | `KUN_GOOGLE_API_KEY` |
| Anthropic | `anthropic` | `ANTHROPIC_API_KEY` |

Choose one with `--provider` or `provider` in a config file. The Anthropic model defaults to `claude-haiku-5-5`; change it with `[providers.anthropic] model = "..."` in a config file.

There is **no automatic fallback**. If a Provider fails, `kun` exits with an error and suggests `--provider`; it never sends your text to a second company on its own.

`kun` honours the `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY` and `NO_PROXY` environment variables (see [configuration](configuration.md#proxies)). The request timeout is 15 seconds by default (`timeout_secs` in a config file).

## Protected Terms

A **Protected Term** is a word or phrase that must appear in the translation exactly as written, such as a product or function name. Configure them in a config file:

```toml
protected_terms = ["translate-kun", "Cargo.toml"]
```

Matching is case-sensitive, and the longest term wins when terms overlap. `kun` swaps each term for a placeholder before sending the text and puts it back afterwards. This works with every Provider and with a Language Pair. If a Provider loses a placeholder, the translation is still returned with a warning (on stderr, or in `warnings` with `--json`). The warning never repeats the term.

## Style

`style` is a tone hint, set in a config file (for example `style = "casual, developer tone"`). Only the Anthropic Provider uses it. The Google Provider ignores it, silently. It can change the wording but never the meaning, and it is sent as data, not as instructions.

## Secret Guard

The Secret Guard runs **before** any text leaves your machine. It looks for these kinds of secret, plus any extra patterns from your config:

- AWS access key IDs, and AWS secret access keys written as an assignment (`aws_secret_access_key = …`, `AWS_SECRET_ACCESS_KEY=…`, `"SecretAccessKey": "…"`)
- GitHub and GitLab tokens
- Anthropic, OpenAI and Google API keys
- Slack tokens
- Stripe live keys (`sk_live_`, `rk_live_`)
- npm and Hugging Face tokens
- private key blocks and JWTs

Keys are also found right after `_`, or after a JSON-escaped `\n` or `\t`.

| Situation | What happens |
|---|---|
| Nothing found | The text is sent. |
| Secret found, on a terminal | `kun` asks `Possible secret detected (...). Send anyway? [y/N]` on stderr. Only `y` or `yes` means yes. The default is no. If the answer is no (or cannot be read), nothing is sent and the exit code is 6. |
| Secret found, not on a terminal (pipe, script, cron) | **Blocked**, exit 6. Nothing is sent. |
| `--redact` | Each match is replaced with `[REDACTED]`, a warning is added, and the rest is sent. |
| `--allow-secrets` | The text is sent as it is. |
| `force_redact = true` in a config | Same as `--redact` every time. |

"On a terminal" means both stdin and stderr are terminals. The answer is read from `/dev/tty`, so the prompt also works when you redirect stdout.

The message that blocks you lists only the kind of secret and its byte range. It never shows the value.

## Dry run

`--dry-run` lets you check for yourself what leaves your machine. It runs everything a real run does up to the point of sending (arguments, config, the size limit, the Secret Guard, the Target Language and Protected Terms), prints what it would send, and **sends nothing**. It never builds a network client and never contacts a Provider.

```sh
$ echo "my key is AKIAFAKEEXAMPLE12345 ok" | kun --dry-run --redact en:th
Dry run: nothing was sent.
provider: google
host: translation.googleapis.com
source language: en
target language: th
second call: no
credentials: env
Secret Guard: redact (matches become [REDACTED])
  - possible AWS access key at bytes 10..30
----- begin text that would be sent -----
my key is [REDACTED] ok
----- end text that would be sent -----
```

The report shows:

- the Provider and the host it would contact (`translation.googleapis.com` for Google, `api.anthropic.com` for Anthropic);
- the model (Anthropic only);
- the Source Language (`detect` when the Provider detects it) and the Target Language, or the Language Pair;
- whether a second call may happen (a Language Pair with a detected Source Language can send the text twice);
- where the credential would come from (`env`, `keychain`, `config file` or `none`), the way `kun auth status` does. A missing credential is not an error in a dry run, and the key is never shown;
- the Secret Guard decision (`send`, `redact` or `ask`) and each finding as a kind and a byte range, never the value;
- the **exact text that would be sent**, after redaction and Protected Terms placeholders, between the begin and end markers. If a real run would make no request (the Source Language is already the Target Language), it says so instead;
- when a [usage budget](#usage-budget) is set for the Provider, a line `budget: <used> of <limit> used this month; this run needs <n>`, and whether a real run would be sent or refused (exit 8). A dry run reads the usage file but never changes it, so it never counts.

The Secret Guard never prompts in a dry run. If a real run would block the text (a secret on a pipe, without `--redact`), the dry run fails the same way, with exit code 6. On a terminal, where a real run would ask, the report says `ask`, says that a real run would ask first, and shows the text as if you answered yes.

The exit code is 0 when a real run would have gone on to send. Other errors (usage, config, size limit) are reported as in a real run. With `--json`, the report is one JSON object: see [json-output.md](json-output.md#dry-run).

## `kun init`

Creates a commented `.kun.toml` (the Project Config) in the current directory.

```sh
kun init            # fails if .kun.toml already exists (exit 2)
kun init --force    # replace an existing .kun.toml
```

Every setting in the template is commented out. Remove the leading `#` to use a line. `--force` replaces a symbolic link itself and does not write through it.

## `kun auth`

Manages API keys in the OS keychain (macOS Keychain, or Secret Service on Linux). `<provider>` is `google` or `anthropic`.

```sh
kun auth login google      # read a key and store it in the keychain
kun auth logout google     # delete the stored key
kun auth status            # show where each key comes from
```

- `login` reads the key with a hidden prompt on a terminal. If stdin is piped, it reads one line: `printf '%s\n' "$KEY" | kun auth login google`. An empty key is rejected.
- `logout` says whether a key was stored.
- `status` prints one line per Provider, such as `google: env`, `anthropic: keychain` or `google: none`. The place is one of `env`, `keychain`, `config file` or `none`. It never prints a key.
- If there is no OS keychain (for example a Linux server), `login` exits with code 3 and suggests setting the environment variable or the `[credentials]` table of the User Config.

Where a key is looked up, first match wins:

1. the environment variable (`KUN_GOOGLE_API_KEY`, `ANTHROPIC_API_KEY`), intended for CI;
2. the OS keychain;
3. the `[credentials]` table of the User Config, only if that file is `0600` (see [configuration.md](configuration.md#credentials)).

## Usage budget

A **Usage Budget** is a limit you set on how many characters `kun` may send to one Provider in a calendar month. It exists to keep you inside a free tier, for example Google Cloud Translation's 500,000 characters per month. It is **off by default**: with no limit, `kun` never reads or writes the usage file, and nothing below applies.

Set a limit with a command, or in the User Config:

```sh
kun budget set google 450000       # stay under Google's 500,000-character free tier
kun budget set anthropic 50000
```

```toml
# ~/.config/kun/config.toml
[budget]
google_chars_per_month = 450000
anthropic_chars_per_month = 50000
```

How it works:

- **One limit per Provider, per calendar month (UTC).** There is no daily limit. The counter starts again from 0 on the 1st of each month.
- **What counts.** The characters of the text actually sent to the Provider (the same unit as `max_chars`): after Secret Guard redaction and Protected Terms placeholders. **Every Provider call counts**, so a Language Pair that makes a second call counts twice. A call is counted only **after a successful reply**. A call that fails is not counted, although a Provider may still bill some failures.
- **Nothing is counted** when the text is not sent: the Source Language equals the Target Language, `--dry-run`, a Secret Guard block, or the size limit.
- **The check comes first.** If the characters already used this month plus the characters of the next call would pass the limit, `kun` refuses **before any network call** and exits with **8**:

  ```text
  $ kun :th "hello"
  error: monthly usage budget for google reached: 449998 of 450000 characters used; this text needs 5. It resets on 2026-12-01 (UTC). Raise it with: kun budget set google <chars>
  ```

  The message holds numbers only, never your text. Reaching the limit exactly is allowed. With a Language Pair, the second call is checked again before it is sent; if it would pass the limit, `kun` fails, and the first call stays counted.
- **There is no override.** No flag skips the budget. To continue, raise or remove the limit (`kun budget set ...`, `kun budget unset ...`), or wait for the reset.
- **User Config only.** A `[budget]` table in a Project Config (`.kun.toml`) is a configuration error (exit 3), so a repository you cloned cannot change or remove your spending guard. See [ADR 0003](adr/0003-project-config-is-untrusted.md).

### The usage file

The counters live in `$XDG_STATE_HOME/kun/usage.json`, or `~/.local/state/kun/usage.json` when `XDG_STATE_HOME` is not set to an absolute path. It holds counters only, for example `{"version":1,"month":"2026-11","google":12345,"anthropic":0}`: no text, no hashes and no timestamps beyond the month. It is written with mode `0600`, atomically (a temporary file, then a rename).

- A missing file means 0 used. A file from another month counts as 0.
- If a budget is set and the file cannot be read or is not valid, `kun` **fails closed**: exit 3, nothing is sent, and the message tells you to delete the file. Without a budget for the Provider in use, the file is ignored and left untouched.
- There is no `kun budget reset`: the counter resets by itself each month. To start from 0 sooner, delete the file by hand.
- Two `kun` processes running at the same moment can lose an increment. `kun` does not lock the file, so the count is approximate.

## `kun budget`

Shows, sets or removes the monthly Usage Budget. `<provider>` is `google` or `anthropic`.

```sh
kun budget                     # same as `kun budget status`
kun budget status
kun budget set google 450000   # write the limit to the User Config
kun budget unset google        # remove the limit
```

- `status` shows, for each Provider, the limit (or `no limit`), the characters used this month, what is left, and the reset date (the 1st of the next month, UTC). With `--json` it prints one object: see [json-output.md](json-output.md#kun-budget-status).

  ```text
  $ kun budget status
  Usage budget for 2026-11 (resets on 2026-12-01, UTC):
  google: 448210 of 450000 characters used, 1790 left
  anthropic: no limit, 0 characters used
  ```

- `set` writes `google_chars_per_month` or `anthropic_chars_per_month` into the `[budget]` table of the User Config. It creates the file and its directory when they are missing (file mode `0600` on Unix) and keeps the rest of the file, comments included, as it was. `<chars>` is a whole number of at least 1. A symbolic link in place of the file is replaced, not written through. If the file is not valid TOML, `set` stops with exit 3 and does not change it.
- `unset` removes only that key. It changes nothing when the key is not there.

`budget` is a subcommand like `auth`, `init` and `completions`, so text that starts with it needs `--` first.

## `kun completions`

Prints a completion script for your shell on stdout. `<shell>` is `bash`, `zsh`, `fish`, `powershell` or `elvish`.

```sh
kun completions bash
```

Install it once, then open a new shell:

| Shell | Install |
|---|---|
| bash | `mkdir -p ~/.local/share/bash-completion/completions && kun completions bash > ~/.local/share/bash-completion/completions/kun` (or add `source <(kun completions bash)` to `~/.bashrc`) |
| zsh | `mkdir -p ~/.zfunc && kun completions zsh > ~/.zfunc/_kun`, and put `fpath=(~/.zfunc $fpath)` before `compinit` in `~/.zshrc` |
| fish | `kun completions fish > ~/.config/fish/completions/kun.fish` |
| powershell | add `kun completions powershell \| Out-String \| Invoke-Expression` to your `$PROFILE` |
| elvish | add `eval (kun completions elvish \| slurp)` to `~/.config/elvish/rc.elv` |

`completions` is a subcommand like `auth`, `budget` and `init`, so text that starts with it needs `--` first.

## Limits, quotas and costs

- **Free tiers exist, but they change.** Both Providers have a free or low-cost way to start, with limits. Check the current terms on their pricing pages before you rely on them.
- **Use two guards: a local budget and a Provider-side cap.** Set a local [usage budget](#usage-budget) (`kun budget set google 450000`) **and** a quota cap or budget alert in the Provider's console (the links below). The local budget is a convenience, not a guarantee: it counts only what this machine sends through `kun` (other machines, scripts and other tools using the same key are not counted), and it is approximate. Google counts characters its own way, and Anthropic bills **tokens**, not characters, so a character budget for Anthropic is only a rough guide. Only the Provider-side cap is enforced by the Provider.
- **`max_chars` bounds the cost of one call.** The default is 10,000 characters per input. Lower it (`--max-chars`, or `max_chars` in a config file) if you want a smaller ceiling. Text longer than the limit is refused before anything is sent (exit 7).
- **A reply that is too long fails clearly.** A very long translation (Thai uses many tokens per character) can hit the Anthropic output limit of 16,000 tokens. `kun` then exits with 5 and tells you to split the text or lower `max_chars`. It never returns a half-finished translation.
- **There are no automatic retries, by design.** A retry could cost money twice and send your text twice. When a Provider rate limits you or a quota runs out, `kun` exits with **8**, and the message says how long to wait if the Provider said so. A script can retry on 8:

  ```sh
  kun :th "$TEXT" || { [ $? -eq 8 ] && sleep 30 && kun :th "$TEXT"; }
  ```

  A rate limit passes on its own. A quota, credit or billing limit does not: open your account page (below), then retry.

Where to check your account and limits:

| Provider | Pricing | Quotas and billing |
|---|---|---|
| Google Cloud Translation | <https://cloud.google.com/translate/pricing> | <https://console.cloud.google.com/apis/api/translate.googleapis.com/quotas> (billing: <https://console.cloud.google.com/billing>) |
| Anthropic | <https://www.anthropic.com/pricing> | <https://console.anthropic.com/settings/limits> (credits: <https://console.anthropic.com/settings/billing>) |

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Unexpected error (including a keychain read or write failure) |
| 2 | Usage error: bad arguments, no text, no Target Language, `kun init` over an existing file |
| 3 | Configuration error (including a credential or a `[budget]` in a Project Config, or a `providers.<name>.endpoint` for the Provider in use), no OS keychain for `kun auth login`, or a usage file that cannot be read while a budget is set |
| 4 | Missing credentials |
| 5 | Network or Provider error (timeout, rejected key, unsupported language, refusal, or a translation too long for one request) |
| 6 | Blocked by the Secret Guard, or you declined its prompt |
| 7 | Input longer than the limit |
| 8 | Rate limited, or the Provider's quota, credit or billing limit was reached, **or your local [usage budget](#usage-budget) was reached** (nothing was sent; raise the limit or wait for the reset). Waiting and retrying, or checking your Provider account, may help. When the Provider sends a `Retry-After` header, the message says how many seconds to wait |

## Environment variables

| Variable | Use |
|---|---|
| `KUN_GOOGLE_API_KEY` | Google API key. |
| `ANTHROPIC_API_KEY` | Anthropic API key. |
| `XDG_CONFIG_HOME` | Where the User Config directory lives (`$XDG_CONFIG_HOME/kun/config.toml`). Must be an absolute path. |
| `XDG_STATE_HOME` | Where the usage file lives (`$XDG_STATE_HOME/kun/usage.json`). Must be an absolute path. |
| `HOME` | Fallback for the User Config (`~/.config/kun/config.toml`) and the usage file (`~/.local/state/kun/usage.json`). |
| `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, `NO_PROXY` | Proxy settings for Provider requests (lower-case names work too). |
