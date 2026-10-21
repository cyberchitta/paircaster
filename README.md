# Selfcast

Your own Farcaster read node, and a client and AI agent skill that read from
it. No API keys from anyone: you furnish the compute, Snapchain syncs the
network, and `selfcast` reads and writes through your node.

- **Tutorial and quickstart:** [selfcast.cyberchitta.cc](https://selfcast.cyberchitta.cc/)
  (source: [`docs/index.md`](docs/index.md))
- **Testing it for us:** [selfcast.cyberchitta.cc/testing.html](https://selfcast.cyberchitta.cc/testing.html)
  (source: [`docs/testing.md`](docs/testing.md))
- **Farcaster documentation we found wrong, and the fixes we sent upstream:**
  [`docs/upstream-fixes.md`](docs/upstream-fixes.md)

## What is here

| Path | What |
|---|---|
| `crates/core` | Message construction, feed and profile assembly, with no network access |
| `crates/cli` | The `selfcast` command: snapshot download and node status, reads, `key add`, and one subcommand per write |
| `skill/` | An agent skill that drives `selfcast` |
| `docs/` | The microsite's pages and the documents above; the `site-build` branch holds the toolchain that builds them |

## Status

New. It has been run on one Apple Silicon Mac, not yet on Linux or a rented
server; the guide ends with a list of what has not been tried.

## Keys

Your recovery phrase is read only from an interactive terminal, used once to
add an app key, and never written to disk. The code wipes it from memory after
use; no test checks that. The app key can post as your account but cannot move
funds or change who owns it. Use a test account.

## Licence

MIT. Made by [cyberchitta](https://github.com/cyberchitta), with Claude as
co-author.
