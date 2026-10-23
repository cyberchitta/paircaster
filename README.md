# Paircaster

Farcaster from your own Snapchain node, for you and your AI agent. No API keys
from anyone: you furnish the compute. It has three parts:

- **A node.** Snapchain, the software Farcaster's nodes run, set up by its own
  installer and run in Docker. `paircaster` downloads its snapshot (about
  800 GB), pausing and resuming where it stopped, and tells you how far the
  node has got.
- **A command-line client, `paircaster`.** It reads profiles, casts, threads,
  follows and the network's recent messages from your node, and casts, reacts
  and follows through it. You run it, and so does your agent, through the
  skill in `skill/`.
- **A viewer.** A page in your browser that shows threads the way a Farcaster
  app does, reading your node itself. The agent puts threads on it, you click
  the cast you mean, and the agent asks the page which one you chose. It reads
  only, and is an option of `paircaster`, built with extra tools.

```
  your machine
 ┌──────────────────────────────────────────────────────────┐
 │  you ───┬──▶ paircaster ────────────HTTP──┐              │
 │         │    (command line)               ▼              │
 │  agent ─┘        │ viewer open      Snapchain node       │
 │                  ▼                  localhost:3381       │
 │  you ──────▶ viewer page ───────────HTTP──▲              │
 │              (your browser)                              │
 └───────────────────────────────────────────┬──────────────┘
                                             │ gossip, UDP 3382
                                             ▼
                                     other Snapchain nodes
                                     and the validators
```

> **Read the code before you give it your recovery phrase.** As with any code
> from the internet, review it yourself, or have someone you trust or an AI
> agent review it, before `paircaster key add` asks for your phrase, and use a
> test account either way. [Safety](https://paircaster.cyberchitta.cc/safety.html)
> lists the files that handle the phrase.

- **Tutorial and quickstart:** [paircaster.cyberchitta.cc](https://paircaster.cyberchitta.cc/)
  (source: [`docs/index.md`](docs/index.md))
- **Testing it for us:** [paircaster.cyberchitta.cc/testing.html](https://paircaster.cyberchitta.cc/testing.html)
  (source: [`docs/testing.md`](docs/testing.md))
- **Farcaster documentation we found wrong, and the fixes we sent upstream:**
  [`docs/upstream-fixes.md`](docs/upstream-fixes.md)

## What is here

| Path | What |
|---|---|
| `crates/core` | Message construction, feed and profile assembly; network access only in its `node` module |
| `crates/cli` | The `paircaster` command: snapshot download and node status, reads, `key add`, and one subcommand per write |
| `crates/wasm` | `paircaster-core`'s reads compiled to WebAssembly for the viewer, returning the same JSON as `paircaster <read> --json` |
| `viewer/` | The viewer page (TypeScript, built with Bun), embedded in `paircaster` by its `viewer` feature |
| `skill/` | An agent skill that drives `paircaster`; Claude Code started in the repo loads it through `.claude/skills/paircaster`, Codex through `.agents/skills/paircaster` |
| `docs/` | The microsite's pages and the documents above; the `site-build` branch holds the toolchain that builds them |

## Status

New. It has been run on one Apple Silicon Mac; on Linux, only building
`paircaster` and its viewer has been tried, and it has not run on a rented
server. The guide ends with a list of what has not been tried.

## Keys

Your recovery phrase is read only from an interactive terminal, used once to
add an app key, and never written to disk. The code wipes it from memory after
use; no test checks that. The app key can post as your account but cannot move
funds or change who owns it. Use a test account.

## Licence

MIT. Made by [cyberchitta](https://github.com/cyberchitta), with Claude as
co-author.
