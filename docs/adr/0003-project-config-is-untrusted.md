# Project Config is untrusted input

A Project Config (`.kun.toml`) lives inside a directory tree that may have been cloned from someone else, so `kun` treats it like untrusted input: it may only choose among built-in Providers, may never hold credentials or custom endpoint URLs, and may only make the Secret Guard stricter, never looser. Anything that sends data somewhere new or weakens a safety check is allowed only in the User Config or by an explicit CLI flag. Without this, running `kun` inside a malicious repository could exfiltrate the user's API key and text to an attacker-controlled endpoint, or silently disable secret detection.

## Consequences

- Users who want a custom endpoint (e.g. a local Ollama) per project cannot get it from the Project Config; they set it in their User Config.
- A credential found in a Project Config is a hard error, not a warning.
