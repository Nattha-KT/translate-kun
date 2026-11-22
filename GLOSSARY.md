# translate-kun

A command-line translator (`kun`) for developers who work in the terminal, inspired by Translate Shell but built on official translation services instead of scraped endpoints.

## Language

### Translation

**Source Language**:
The language of the text being translated; detected automatically unless stated.
_Avoid_: from-language, input language

**Target Language**:
The language the text is translated into.
_Avoid_: to-language, output language, destination

**Language Pair**:
Two languages configured together so the Target Language is chosen from the detected Source Language: text in one is translated into the other.
_Avoid_: bidirectional mode, auto-swap, toggle

**Provider**:
An external or local service that performs a translation (e.g. a translation API or an LLM).
_Avoid_: engine, backend, service, translator

### Configuration

**Project Config**:
Settings that apply to one directory tree, found by walking up from the current directory; created by `kun init`. Never holds credentials.
_Avoid_: workspace config, local config, .kunrc

**User Config**:
Personal settings for one user across all directories; the only config allowed to hold credentials.
_Avoid_: global config, home config

**Protected Term**:
A word or phrase that must appear in the translation exactly as written in the source (e.g. product or function names).
_Avoid_: glossary, do-not-translate list, keyword

### Safety

**Secret Guard**:
The check that inspects outgoing text for credentials (API keys, tokens, private keys) before it is sent to a Provider.
_Avoid_: scanner, filter, sanitizer

**Usage Budget**:
An opt-in local limit on how many characters `kun` may send to one Provider per calendar month (UTC). Set in the User Config only. When the next call would pass it, `kun` refuses before sending. It is approximate: it counts this machine only.
_Avoid_: quota (that is the Provider's own limit), cap, allowance, credit

### Integration

**Adapter**:
A front end that exposes translate-kun inside another environment (the terminal CLI, a VS Code extension, etc.).
_Avoid_: plugin, integration, client, wrapper
