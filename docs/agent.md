---
title: Giving your AI agent Farcaster
dek: A skill that lets your AI agent read Farcaster from your own node, show you a thread in your browser, and ask you before each write.
---

Selfcast runs a Farcaster node on your own machine, which needs about 800 GB of disk and a day or more before it is ready, and a command-line client, `selfcast`, that reads from that node and posts through it. This page hands the client to your AI agent. Not set up yet? Start at the [quickstart](index.md#quickstart).

Before you start: a node that `selfcast node status` reports ready, and `selfcast` on your `PATH`, both from the quickstart. For writes, an app key, which you add yourself in your own terminal ([Adding an app key](client.md#adding-an-app-key)).

## The skill

The repository's `skill/` directory is an agent skill: instructions an AI agent loads before it runs `selfcast`. The agent runs the same commands you do, with `--json`, and has the same limits: no search, and nothing older than an account's newest 1000 messages.

Claude Code finds the skill in the clone. From the directory you cloned into:

```run
cd selfcast
claude
```

It loads the skill from `.claude/skills/selfcast`, a link to `skill/`, with nothing to copy. Asked “How far behind is my Farcaster node?”, it loads the skill on its own and runs `selfcast node status --json`. [What you can do with it](uses.md) has more questions, with the agent's answers. Another agent can be told to read `skill/SKILL.md`.

Codex loads the skill from `.agents/skills/selfcast`, another link to `skill/`. Its default sandbox blocks network access, `localhost` included, so `selfcast` cannot reach the node and says so. Allow network access for the session:

```run
codex exec -s workspace-write -c sandbox_workspace_write.network_access=true "How far behind is my Farcaster node?"
```

```output
Your node is **syncing**. Shards 0 and 1 are about **2 seconds behind**; shard 2 is about **49 minutes 38 seconds behind**. So shard 2 is the lagging one.
```

It read the skill and ran `selfcast node status --json` to answer. Under the default sandbox, `selfcast node status` fails with:

```output
error: not permitted to connect to http://localhost:3381, so the node's state is unknown; an agent's sandbox may be blocking network access for selfcast
```

On Windows, git checks both links out as plain files unless symlinks are enabled (`core.symlinks`), and then the skill does not load.

## What the skill tells the agent

- **Check the node first.** Run `selfcast node status` before reading, and say how far behind the node is in every answer.
- **Reads only, until you say otherwise.** It runs no write until you tell it, in that conversation, that it may write, and that lasts for the conversation only.
- **You approve Selfcast's text, not the agent's.** For each write it runs the command with `--dry-run`, shows you the text `selfcast` printed, exactly as printed, and waits for your yes before running it again with `--yes`. `selfcast` writes that text from the message it signs, so within one run it cannot show one thing and send another. The two runs are separate commands, though: the `--yes` run prints the text again from what it actually sent, and the skill shows you that too, so check that it matches what you approved.
- **Never a recovery phrase.** It never runs `key add`, never asks for a phrase, and refuses one if it is pasted in: you run `key add` in your own terminal.

The writes in [Using the client](client.md#writing:-casts,-reactions,-follows) were sent by an agent following these steps.

## Gating writes in your harness

Each write is its own command so that your agent's harness can gate it apart from the reads: let reads run, ask you before each write, and refuse `selfcast key` to the agent. How to say that is up to the harness. In Claude Code, this rule refuses `selfcast key` to the agent:

```file .claude/settings.local.json
{
  "permissions": {
    "deny": ["Bash(*selfcast key *)"]
  }
}
```

In Claude Code's auto mode it refused the agent's `selfcast key --help`, and also a command that only contained those words. Rules that ask before each write have not yet been seen to prompt. The ones tried, such as `Bash(*selfcast cast *)`, cannot match a write run as `selfcast --node … cast …`, which is how the writes were run (through Farcaster's public node, because the test node had not yet synced the new test account), and auto mode ran them without asking. Until one does, what stands between the agent and a write is the skill's instructions, which are not a lock, and the key's scope.

<limit>

Rules like these match the command's text, so another spelling (options first, another path, `sh -c`) or a mode that skips prompts gets past them, and harmless text can trip them. They are a convenience, not a boundary. On your machine an agent with a shell can send anything the app key allows. What holds is the key's scope: casts, deletes, reactions and follows, never the account, its funds or its keys; and `key add` reads the phrase only from a terminal, which an agent's shell does not have.

</limit>

## Looking at a thread together

The agent can show you a thread instead of describing it: a page in your browser that reads your node, where you follow replies and go back without the agent. When you click a cast, the agent can read which one you chose. You and the agent are looking at the same page.

### Build it

The page is built into `selfcast` only with the `viewer` feature, which the quickstart's build leaves out: building the page needs tools the rest of `selfcast` does not. They are [Bun](https://bun.sh), the WebAssembly target, and `wasm-bindgen-cli` at the same version as the `wasm-bindgen` in `Cargo.lock`. From the clone, find that version:

```run
grep -A1 'name = "wasm-bindgen"' Cargo.lock
```

```output
name = "wasm-bindgen"
version = "0.2.129"
```

Then build, with that version in the second line:

```run
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
cd viewer && bun install && bun build.ts && cd ..
cargo install --path crates/cli --locked --features viewer
```

```output
dist/: index.html 3419 bytes, viewer.js 1232386 (wasm 892799)
```

`bun build.ts` prints the line above; the other steps print their usual output.

### You start the page

In a terminal of its own, since it keeps running, start the page and open the address it prints:

```run
selfcast viewer serve
```

```output
open the viewer at http://127.0.0.1:63677/?pair=42909cb06db6d5baacdcceccccf3ecbe769860af1788d5515b7d2a632033ff1d
stop it with Ctrl-C
```

It listens on `127.0.0.1` only, and the page can reach only the node `selfcast` reads from. The address carries a token; a page opened without it is refused. The agent needs neither the address nor the token: its `viewer` commands reach the server through a local socket that only your user account can open. `selfcast viewer serve --images` also shows profile pictures (see the limit below).

### The agent shows you a thread

Ask the agent to show you a thread. It runs `viewer open` with the cast as `FID:HASH` (the author's FID and the cast's hash, as every read returns them), then `viewer state` to see what the page shows:

```run
selfcast viewer open 3356360:0x3820fb679133cd50df09d95beb8a5fbe2553069d
selfcast viewer state
```

```output
revealed selfcast://selfcast/thread/3356360:0x3820fb679133cd50df09d95beb8a5fbe2553069d
message: thread of 3356360:0x3820fb679133cd50df09d95beb8a5fbe2553069d: 0 casts above, 1 reply, from http://localhost:3381
told selfcast
message: the thread of 3356360:0x3820fb679133cd50df09d95beb8a5fbe2553069d by @selfcast ("This cast went out through my own Snapchain node, signed by an app key I added w"), 0 casts above it, 1 reply
result: {"back":[],"cast":"3356360:0x3820fb679133cd50df09d95beb8a5fbe2553069d","node":"http://localhost:3381","selected":null,"user_selected":null,"view":"thread"}
```

The `selfcast://…` line names the view, and `viewer open` takes that name too. In `result`, `view` and `cast` are what is on screen, `back` is what Back returns to, and `selected` lists casts the agent marked when it opened the view (none here).

### You point, the agent reads

Click the reply in the page. The agent learns what you clicked only when it next runs `viewer state`, so ask it about the cast you are on:

```run
selfcast viewer state
```

```output
told selfcast
message: the thread of 3356360:0x4853a7d5837632037621a83f1068c8b57ba5ed24 by @selfcast ("How: selfcast node fetch-snapshot cd ~/snapchain && ./snapchain.sh up selfcast k"), 1 cast above it, 1 reply
result: {"back":["3356360:0x3820fb679133cd50df09d95beb8a5fbe2553069d"],"cast":"3356360:0x4853a7d5837632037621a83f1068c8b57ba5ed24","node":"http://localhost:3381","selected":null,"user_selected":"selfcast://selfcast/thread/3356360:0x4853a7d5837632037621a83f1068c8b57ba5ed24","view":"thread"}
```

The page opened the reply's own thread, and `back` holds the one you came from. `user_selected` is the cast you last clicked or opened in the page; only you change it, never what the agent opens. The skill tells the agent that this is the cast you are pointing at.

<limit>

The page cannot tell whether you are looking at it, or whether you or the agent put up what it shows now: only `user_selected` is always yours. Without `--images` it loads nothing but from your node, and shows initials. With it, each profile picture loads from the server its account chose, which learns your address and when you looked. Images in casts are not shown either way. `viewer open` and `viewer state` fail when no page is open, and say so.

</limit>

---

Next: [What you can do with it](uses.md)
