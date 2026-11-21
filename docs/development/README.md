# How translate-kun was built

This folder explains the development process behind `kun`. You do not need any of it to use or contribute to the tool. It is here so the decisions are easy to trace.

## The process

1. **Requirement interviews.** The scope, the security rules and the user-facing behaviour were agreed in discovery sessions and written down in [`../requirements.md`](../requirements.md).
2. **Architecture Decision Records (ADRs).** Decisions that must not be reversed by accident are recorded in [`../adr/`](../adr/). Examples: official Providers only (no scraping), Rust, and "the Project Config is untrusted input".
3. **Slice specs.** The work was cut into small slices (S0, S1, and so on). Each slice has a spec with a goal, the requirement sections to read, the files in scope, and acceptance criteria. They live in [`slices/`](./slices/). The status table is in [`slices/README.md`](./slices/README.md).
4. **AI agents under written rules.** Each slice was implemented by an AI coding agent and reviewed by a second agent, then approved by the repository owner. The agents follow the rules in [`AGENTS.md`](../../AGENTS.md): code style, the security model, the Definition of Done and the report format.

## Where things live

| Artefact | Location |
|---|---|
| What to build | [`docs/requirements.md`](../requirements.md) |
| Canonical terms | [`GLOSSARY.md`](../../GLOSSARY.md) |
| Decisions | [`docs/adr/`](../adr/) |
| Slice specs and status | [`docs/development/slices/`](./slices/) |
| Rules for agents (and a useful read for humans) | [`AGENTS.md`](../../AGENTS.md) |
| Rules for human contributors | [`CONTRIBUTING.md`](../../CONTRIBUTING.md) |

## Why keep this?

The author is learning Rust through the project, so the process is written down on purpose: a reader can see why each piece exists, not just what it does. Contributions do not need to follow the slice process. See [`CONTRIBUTING.md`](../../CONTRIBUTING.md).
