---
title: Giving your AI agent Farcaster
dek: A skill that lets your AI agent read Farcaster from your own node, show you a thread in your browser, and ask you before each write.
---

Selfcast runs a Farcaster node on your own machine, which needs about 800 GB of disk and a day or more before it is ready, and a command-line client, `selfcast`, that reads from that node and posts through it. This page hands the client to your AI agent. Not set up yet? Start at the [quickstart](index.md#quickstart).

Before you start: a node that `selfcast node status` reports ready, and `selfcast` on your `PATH`, both from the quickstart. For writes, an app key, which you add yourself in your own terminal ([Adding an app key](client.md#adding-an-app-key)).

## The skill

The repository's `skill/` directory is an agent skill: instructions an AI agent loads before it runs `selfcast`. The agent runs the same commands you do, with `--json`, and has the same limits: search reaches back about three days, through `selfcast events`, and nothing older than an account's newest 1000 messages can be read.

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
dist/: index.html 3419 bytes, viewer.js 1245675 (wasm 902684)
```

`bun build.ts` prints the line above; the other steps print their usual output.

### You start the page

Ask the agent in plain words. In this section the prompts are as typed, and the replies as the agent wrote them:

> Start the viewer with profile pictures so we can look at threads together.

It ran `selfcast viewer serve --images` in the background and answered:

```output
The viewer is running with profile pictures on. Open this address in your browser:

http://127.0.0.1:62331/?pair=d378d2db50a261591237c7a8507453a1bd6bea3b9a1fa1b8e2a8c4244acf2ef4

With pictures on, each one loads from whatever server that account chose for it. Those servers can see your IP address and when you looked.

The page is blank until there's a thread on it. Name a cast or a conversation and I'll put it up, or click a cast on the page and I can see which one you picked. It keeps running until I stop it or this session ends.
```

You can also start it yourself, in a terminal of its own since it keeps running:

```run
selfcast viewer serve
```

```output
open the viewer at http://127.0.0.1:63677/?pair=42909cb06db6d5baacdcceccccf3ecbe769860af1788d5515b7d2a632033ff1d
stop it with Ctrl-C
```

It listens on `127.0.0.1` only, and the page can reach only the node `selfcast` reads from. The address carries a token; a page opened without it is refused. The agent needs neither the address nor the token: its `viewer` commands reach the server through a local socket that only your user account can open.

### The agent shows you a thread

> Find Dan Romero's wishlists for a Farcaster client and a Snapchain node, and show me the client one.

It searched his newest casts, ran `selfcast viewer open 3:0x597007de6219f87fdf71ad5769db542c8f2d1dde`, and answered:

```output
The viewer now shows Dan Romero's client wishlist with its 16 replies. He posted it on 16 October, starting with "Client wishlist", and it's a top-level post, so it isn't replying to anything.

His Snapchain node wishlist went up seven minutes later, starting with "Snapchain node wishlist". It asks for two things: to sync only part of the network, and to keep no more than 7 days of content. I can put that one on the page too.

I found both by searching his newest 1,000 casts for "wishlist", "client" and "node". Your node is caught up, so nothing recent should be missing.
```

### Only what is worth reading

> Show me just the replies worth reading.

It read the thread with `selfcast thread --json`, chose six replies, and opened the thread with only those, in its order. The rest fold behind a "Show all" button. The reply names each cast by its author and a few of its words, so you can find it on the page, and says why it chose it:

<showcase type="image" src="images/viewer-replies.webp" frame="1600x904" alt="The agent's reply in a terminal on the left, listing six replies with a reason for each; on the right, the client wishlist in the browser under the heading 6 of 16 replies, the first of them Dan Romero's own continuation of the list." caption="Six of the wishlist's 16 replies, in the agent's order." />

### You point, the agent reads

> Now show me the node wishlist

It opened that thread. In the page, click KMac's reply, "Yes & filter only from specific signers": the page opens the reply's own thread, under the post it answers. Then ask about it:

> What does the person I've selected want, and can Selfcast do it?

The agent learns what you clicked only when it runs `viewer state`, and it ran that first:

```output
told selfcast
message: the thread of 4163:0x45563785c747be1f0c407defdae26e3b5d4f596b by @kmacb.eth ("Yes & filter only from specific signers"), 1 cast above it, 0 replies
result: {"back":["3:0x35f104e944de03c93cc4eb513e8b84cef58d9b77"],"cast":"4163:0x45563785c747be1f0c407defdae26e3b5d4f596b","node":"http://localhost:3381","selected":null,"user_selected":"selfcast://selfcast/thread/4163:0x45563785c747be1f0c407defdae26e3b5d4f596b","view":"thread"}
```

`user_selected` is the cast you last clicked or opened in the page; only you change it, never what the agent opens. `view` and `cast` are what is on screen, `back` is what Back returns to, and `selected` lists casts the agent marked (none here). It then read the reply and the post above it, and answered:

<showcase type="image" src="images/viewer-pointing.webp" frame="1600x904" alt="The agent's answer in a terminal on the left: Selfcast can filter what it reads by signer, but not what the node stores. On the right, the browser shows Dan Romero's Snapchain node wishlist with KMac's reply under it, opened by the click." caption="The reply you clicked, and the agent's answer about it." />

<limit>

The page cannot tell whether you are looking at it, or whether you or the agent put up what it shows now: only `user_selected` is always yours. Without `--images` it loads nothing but from your node, and shows initials; the screenshots above were taken with it. With it, each profile picture loads from the server its account chose, which learns your address and when you looked. Images in casts are not shown either way. `viewer open` and `viewer state` fail when no page is open, and say so.

</limit>

---

Next: [What you can do with it](uses.md)
