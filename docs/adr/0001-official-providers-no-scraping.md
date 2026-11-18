# Official provider APIs only — no scraping of free web endpoints

Translate Shell works without any API key by calling unofficial Google/Bing web endpoints, which is why it breaks, gets rate-limited, and sits in a terms-of-service grey area. translate-kun only talks to official, key-authenticated Providers (Google Cloud Translation as the default, Anthropic as the second Provider in v1). We accept the cost of first-run friction (users must obtain an API key) in exchange for reliability and legitimacy.

## Considered Options

- **Scrape free endpoints (Translate Shell's approach)**: zero setup, but fragile and possibly against provider ToS. Rejected.
- **Offline-only models**: private, but large installs and weaker quality. Deferred; the Provider abstraction keeps it possible later.
