# Privacy: what data leaves my machine?

Short answer: **your text and your API key go to the Provider you chose, and nothing else goes anywhere.** `kun` has no server of its own, no telemetry, no update check, no cache and no history.

## What leaves your machine

| Data | When it leaves | Where it goes | Can I prevent it? |
|---|---|---|---|
| **The text to translate**, after the Secret Guard, `--redact` and Protected Terms have been applied | Only when you run a translation, and only after the Secret Guard allows it (a prompt on a terminal, a block in scripts) | The one Provider you chose: `translation.googleapis.com` (Google Cloud Translation) or `api.anthropic.com` (Anthropic). With a Language Pair, the text can be sent a second time to the same Provider, to translate in the other direction. | Yes. Use `kun --dry-run` to see it without sending anything. Use `--redact` or Protected Terms to remove parts. Do not run `kun`, or choose a different Provider with `--provider`. |
| **The API key** | With every Provider request, in an HTTPS header (`x-goog-api-key` for Google, `x-api-key` for Anthropic). Never in a URL. | The same Provider only, over TLS. | It is how the Provider knows who you are, so a translation needs it. `--dry-run` does not read it. |
| Anything else | Never | Nowhere | Nothing to prevent: there is no telemetry, no update check, no crash report, no cache and no history of source or translated text. |

Details that are easy to miss:

- **Protected Terms and Secret Guard patterns stay local.** Protected Terms are replaced by placeholder tokens before sending, so the Provider sees the token, not the term.
- **`style` is sent to Anthropic.** If you set `style` (in a config file or the Project Config), that text is part of the request to the Anthropic Provider. The Google Provider ignores it.
- **The Provider sees ordinary network metadata** that any HTTPS request carries, such as your IP address (or your proxy's). `kun` honours `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY` and `NO_PROXY`.
- **A Project Config can change which Provider you use** (it cannot add credentials or endpoints, see [ADR 0003](adr/0003-project-config-is-untrusted.md)). `kun` prints `provider: …` on stderr, and `kun --dry-run` shows the destination before anything is sent, so check it in a repository you do not trust.
- **Text you pass as arguments** is saved in your shell history and visible in `ps` to other local users before `kun` even starts. Send sensitive text through stdin. See [SECURITY.md](../SECURITY.md).

## Verify it yourself

You do not have to take this page's word for it.

1. **`kun --dry-run`.** It runs everything up to the point of sending (config, size limit, Secret Guard, Protected Terms), prints the destination and the exact text that would be sent, and sends nothing. It never builds a network client. See [usage.md](usage.md).
2. **Read the source.** The whole network surface is two small files: [`google.rs`](../crates/kun-core/src/provider/google.rs) and [`anthropic.rs`](../crates/kun-core/src/provider/anthropic.rs). Each one holds a single fixed HTTPS endpoint. There is no other HTTP client code, no logger that could print headers, and redirects are disabled. The dependency list is in `Cargo.toml` and is checked by `cargo deny` in CI.
3. **Watch the network.** Run a real translation while you capture traffic with a tool you trust (for example `tcpdump`, Wireshark, or a firewall such as Little Snitch or `lsof -i`). You should see connections to your chosen Provider's host only. The content is encrypted, but the list of hosts is not.

## What the Providers do with your text

Once the text reaches a Provider, **their** privacy terms apply, not ours. `kun` cannot see or control retention, logging, or whether your data is used for any other purpose. Read the terms for the Provider you use, and check them again from time to time, because they change:

- Google Cloud: [Privacy notice](https://cloud.google.com/privacy) and [Cloud Translation data usage](https://cloud.google.com/translate/data-usage)
- Anthropic: [Privacy policy](https://www.anthropic.com/legal/privacy)

Do not use `kun` for text that your organisation does not allow to be sent to that Provider. If a text must never leave your machine, do not translate it with `kun`. The Secret Guard is a safety net for credentials, not a way to make private text safe to send.

## Related

- [SECURITY.md](../SECURITY.md): threat model and how to report a vulnerability
- [Security audit](security/audit-2026-11.md): what was checked, and the limits of that check
