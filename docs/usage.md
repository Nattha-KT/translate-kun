# Usage

This page lists every command and flag of `kun`. For settings in config files, see [configuration.md](configuration.md).

```text
kun [OPTIONS] [:xx | xx:yy] [TEXT]...
kun init [--force]
kun auth login <google|anthropic>
kun auth logout <google|anthropic>
kun auth status
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

If the text itself starts with `auth`, `init` or `completions`, put `--` before it, because those words are subcommands:

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

`completions` is a subcommand like `auth` and `init`, so text that starts with it needs `--` first.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Unexpected error (including a keychain read or write failure) |
| 2 | Usage error: bad arguments, no text, no Target Language, `kun init` over an existing file |
| 3 | Configuration error (including a credential in a Project Config, or a `providers.<name>.endpoint` for the Provider in use), or no OS keychain for `kun auth login` |
| 4 | Missing credentials |
| 5 | Network or Provider error (timeout, rate limit, rejected key, unsupported language, refusal) |
| 6 | Blocked by the Secret Guard, or you declined its prompt |
| 7 | Input longer than the limit |

## Environment variables

| Variable | Use |
|---|---|
| `KUN_GOOGLE_API_KEY` | Google API key. |
| `ANTHROPIC_API_KEY` | Anthropic API key. |
| `XDG_CONFIG_HOME` | Where the User Config directory lives (`$XDG_CONFIG_HOME/kun/config.toml`). Must be an absolute path. |
| `HOME` | Fallback for the User Config (`~/.config/kun/config.toml`). |
| `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, `NO_PROXY` | Proxy settings for Provider requests (lower-case names work too). |
