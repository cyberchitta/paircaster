---
title: Giving your AI agent Farcaster
---

The repository's `skill/` directory is an agent skill: instructions an AI agent loads before it runs `selfcast`. It calls the same commands as you do, with `--json`.

It comes with the repository. Start Claude Code in the clone from the [quickstart](index.md#quickstart)'s first step:

```run
cd selfcast
claude
```

It loads the skill from `.claude/skills/selfcast`, a link to `skill/`, with nothing to copy. Asked “How far behind is my Farcaster node?”, it loads the skill on its own and runs `selfcast node status --json`. The skill calls `selfcast` by name, so the `cargo install` from that step has to have put it on your `PATH`. Another agent can be pointed at `skill/SKILL.md`.

Codex loads it from `.agents/skills/selfcast`, another link to `skill/`, and reads `AGENTS.md`, a link to the repository's `CLAUDE.md`. Its default sandbox blocks network access, `localhost` included, so `selfcast` cannot reach the node and says so. Allow network access for the session:

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

On Windows, git checks both links out as plain files unless symlinks are enabled (`core.symlinks`), and then the skill does not load. The writes in [Using the client](client.md#writing:-casts,-reactions,-follows) were sent by an agent following the skill's write steps.

What the skill tells the agent:

- **Check the node first.** Run `selfcast node status` before reading, and say how far behind the node is in every answer.
- **Reads only, until you say otherwise.** It runs no write until you enable writes in that conversation, and that lasts for the conversation only.
- **You approve Selfcast's text, not the agent's.** For each write it runs the command with `--dry-run`, shows you the text `selfcast` printed, exactly as printed, and waits for your yes before running it again with `--yes`. `selfcast` writes that text from the message it signs, so within one run it cannot show one thing and send another. The two runs are separate commands, though: the `--yes` run prints the text again from what it actually sent, and the skill shows you that too, so check that it matches what you approved.
- **Never a recovery phrase.** It never runs `key add`, never asks for a phrase, and refuses one if it is pasted in: you run `key add` in your own terminal.

Each write is its own command so that your agent's harness can gate it apart from the reads: let reads run, ask you before each write, and refuse `selfcast key` to the agent. How to say that is up to the harness. In Claude Code, for example:

```file .claude/settings.local.json
{
  "permissions": {
    "ask": ["Bash(*selfcast cast *)", "Bash(*selfcast delete *)", "Bash(*selfcast react *)",
            "Bash(*selfcast unreact *)", "Bash(*selfcast follow *)", "Bash(*selfcast unfollow *)",
            "Bash(*selfcast resend*)"],
    "deny": ["Bash(*selfcast key *)"]
  }
}
```

Tried in Claude Code's auto mode: the `deny` rule held, refusing the agent's `selfcast key --help`, and also a command that only contained the words, in text the agent was writing into this site. The `ask` rules did not prompt: with `--node` between `selfcast` and `cast` the pattern cannot match, and auto mode ran the writes without asking.

<limit>

Rules like these match the command's text, so another spelling (options first, another path, `sh -c`) or a mode that skips prompts gets past them, and harmless text can trip them. They are a convenience, not a boundary. On your machine an agent with a shell can send anything the app key allows. What holds is the key's scope: casts, deletes, reactions and follows, never the account, its funds or its keys; and `key add` reads the phrase only from a terminal, which an agent's shell does not have.

</limit>

## Looking at a thread together

The agent can show you a thread instead of describing it: a page in your browser that reads your node, where you follow replies and go back without the agent. When you click a cast, the agent can read which one you chose. You and the agent are looking at the same page.

The page is built into `selfcast` only with the `viewer` feature, which the quickstart's `cargo install` leaves out. Building it needs [Bun](https://bun.sh), the WebAssembly target, and `wasm-bindgen-cli` at the version in `Cargo.lock`. From the clone:

```run
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
cd viewer && bun install && bun build.ts && cd ..
cargo install --path crates/cli --locked --features viewer
```

```output
dist/: index.html 3329 bytes, viewer.js 909127 (wasm 656341)
```

`bun build.ts` prints the line above; the other steps print their usual output. Then start the page and open the address it prints:

```run
selfcast viewer serve
```

```output
open the viewer at http://127.0.0.1:63677/?pair=42909cb06db6d5baacdcceccccf3ecbe769860af1788d5515b7d2a632033ff1d
stop it with Ctrl-C
```

It keeps running, on `127.0.0.1` only, and the page can reach only the node `selfcast` reads from. The address carries a token; a page opened without it is refused. With `--images` the page also shows profile pictures:

```run
selfcast viewer serve --images
```

```output
open the viewer at http://127.0.0.1:59216/?pair=89ca3260eb3c0f58241ca683e4eb1574dc8dd575d657acbc7fddc0cf04564125
stop it with Ctrl-C
``` The agent then shows a thread with `viewer open`, and asks the page what it shows with `viewer state`:

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

Click the reply in the page, and the agent asks again:

```run
selfcast viewer state
```

```output
told selfcast
message: the thread of 3356360:0x4853a7d5837632037621a83f1068c8b57ba5ed24 by @selfcast ("How: selfcast node fetch-snapshot cd ~/snapchain && ./snapchain.sh up selfcast k"), 1 cast above it, 1 reply
result: {"back":["3356360:0x3820fb679133cd50df09d95beb8a5fbe2553069d"],"cast":"3356360:0x4853a7d5837632037621a83f1068c8b57ba5ed24","node":"http://localhost:3381","selected":null,"user_selected":"selfcast://selfcast/thread/3356360:0x4853a7d5837632037621a83f1068c8b57ba5ed24","view":"thread"}
```

The page opened the reply's own thread, `back` holds the one you came from, and `user_selected` is the cast you last clicked or opened in the page. What the agent opens never changes it, so `cast` and `selected` say what is on screen and `user_selected` what you chose. The skill tells the agent that this is the cast you are pointing at.

<limit>

The page cannot tell whether you are looking at it, or who chose what it shows. Without `--images` it loads nothing but from your node, and shows initials. With it, each profile picture loads from the server its account chose, which learns your address and when you looked. Images in casts are not shown either way. `viewer open` and `viewer state` fail when no page is open, and say so.

</limit>

What the agent cannot read yet: search results, channels, and anything older than an account's newest 1000 messages.

---

Next: [What you can do with it](uses.md)
