---
title: Paircaster
dek: Farcaster from your own Snapchain node, for you and your AI agent. No API keys from anyone.
---

[Farcaster](https://farcaster.xyz) is a decentralised social network: its accounts, casts (posts), likes and follows are signed messages, and every node keeps a copy of them. Snapchain is the software those nodes run. Paircaster sets up a Snapchain node on your own machine and gives you, and the AI agent you use, a command-line client that reads from that node and posts through it. It is for people at home in a terminal who want Farcaster without an app or a hosted API in between. The node needs about 800 GB of disk and about 12 hours of download before it starts catching up with the network. Paircaster was called Selfcast. Its test account is still `@selfcast`, and output captured before the rename shows the old name.

<warning>

**Reading needs no keys. Writing needs an app key, which you add once with your recovery phrase: read the code before you give it that phrase.** As with any code from the internet, review it yourself, or have someone you trust or an AI agent review it, and use a test account either way. [Safety](safety.md) lists the files that handle the phrase.

</warning>

<note>

**Paircaster is new.** This guide was run on one Mac (Apple Silicon, macOS, Docker through OrbStack), except what is [listed at the end](#not-tried-yet) as not tried yet. The outputs are from real runs while Paircaster was being built; where a command has changed since, the guide shows the current one, so what you see may differ in small ways. Paths under your home directory are shown as `~`. To run it on a clean machine and report what happened, see [Testing Paircaster](testing.md).

</note>

## What you get

- **Your own node, no API keys.** `paircaster` reads from the Snapchain node on your machine, at `http://localhost:3381` unless you point it elsewhere, so what you read does not pass through anyone else's server ([how it fits together](#what-you-just-did)).
- **No server needed.** It can run on a machine you already use. Snapchain's own snapshot download starts over after a short network drop, so `paircaster` has its own, which you can pause and resume where it stopped. A node on a laptop syncs only while the laptop is awake; after a 40-minute sleep it reconnected and carried on ([The snapshot](node.md#the-snapshot), [Sleep](node.md#sleep,-shutdown-and-logout)).
- **Your recovery phrase stays in your terminal.** `paircaster key add` reads it from the terminal only, never from a file, an argument, a pipe or an environment variable, uses it once to add an app key, and writes only the app key to disk ([Safety](safety.md)).
- **You see what is signed, and every write is read back.** Before a write, `paircaster` shows the message it is about to sign, written from the signed message itself. After sending, it reads the message back from the node before it calls it sent. So far that text has been approved through an agent's dry run, not yet typed at the prompt ([Writing](client.md#writing:-casts,-reactions,-follows)).
- **Your AI agent uses the same commands.** A skill tells an agent to run `paircaster`, to stay read-only until you enable writes, and to show you `paircaster`'s own text before each one. Those are instructions, not a lock: an agent with a shell on your machine can send anything the app key allows ([Your AI agent](agent.md)).
- **A thread you and your agent look at together.** You start a page in your browser, and your agent puts threads on it. You follow its replies and click the cast you mean, and the agent reads what you chose. The page reads your node itself. It needs `paircaster` built with the `viewer` feature, which the quickstart's build leaves out ([building it](agent.md#build-it)).
- **Farcaster from the command line.** Read profiles, casts and a feed; cast and delete, like and unlike, follow and unfollow. Each cast shows the app that posted it, so you can keep or drop casts by app ([Filtering by client](client.md#filtering-by-client)). There is no search command: `paircaster events` lists the network's last three days or so of messages, each cast with its text, for you to filter with `jq` or `grep` ([Events](client.md#events)). Nothing older than an account's newest 1000 messages can be read.

## Quickstart

You need:

- `git`, `curl` and a [Rust toolchain](https://rustup.rs). There is no release binary yet, so step 1 builds `paircaster` from source.
- Docker, running. Snapchain's script runs the node in a container.
- 16 GB of RAM and 4 vCPUs, Snapchain's stated minimum.
- About 800 GB of free disk while the snapshot downloads and unpacks; the node then uses 557 GB and grows ([details](node.md#hardware-and-disk)).
- Time: about 12 hours for the download and unpack, then the node's catch-up with the network, which took the test node a day or more.

Each step below is explained in [Running the node](node.md).

### 1. Install paircaster

```run
git clone https://github.com/cyberchitta/paircaster
cd paircaster
cargo install --path crates/cli --locked
```

### 2. Install Snapchain, then stop it

```run
mkdir -p ~/snapchain && echo SKIP_CRONTAB=true >> ~/snapchain/.env
curl -sSL https://raw.githubusercontent.com/farcasterxyz/snapchain/refs/heads/main/scripts/snapchain-bootstrap.sh | bash
```

The script asks two questions. Agree to the first, that a read node earns no rewards. Leave the second, an operator FID, blank: the first command made that safe, and upgrades then happen [by hand](node.md#upgrades). It installs to `~/snapchain` and starts the node, which begins its own snapshot download. That download cannot resume after an interruption ([why](node.md#the-snapshot)), so stop it and let `paircaster` download instead:

```run
cd ~/snapchain && ./snapchain.sh down
```

`paircaster node fetch-snapshot` refuses to start while the node's container is running, and names this command.

### 3. Download the snapshot

```run
paircaster node fetch-snapshot
```

The download and unpack took about 12 h 15 min of wall-clock time on the test machine, pauses included, at 8 to 41 MB/s.

### 4. Start the node

```run
cd ~/snapchain && ./snapchain.sh up
```

This starts the node and then follows its log; Ctrl-C stops following the log and leaves the node running. The node has to catch up from the snapshot's date to now. Check on it from any terminal:

```run
paircaster node status
```

Four hours after starting, the test node was [still catching up](node.md#why-sync-is-slow):

```output
Syncing (Snapchain 0.14.3): 47h07m behind at most.
  shard 0: height 48391469, 23h49m behind
  shard 1: height 49002681, 41h55m behind
  shard 2: height 48819789, 47h07m behind
Disk: 3165 GB free of 3996 GB.
```

### 5. Read from your node

You are done when `paircaster node status` says `Ready`:

```output
Ready (Snapchain 0.14.3): every shard within a minute of its newest block.
  shard 0: height 48725626, 2s behind
  shard 1: height 49402047, 1s behind
  shard 2: height 49237300, 1s behind
Disk: 3145 GB free of 3996 GB.
```

Then read an account from it:

```run
paircaster profile dwr
```

```output
dwr  fid 3
  name: Dan Romero
  bio: Interested in technology and other stuff.
  url: https://danromero.org
  twitter: dwr
  following: 79
  followers: at least 10000 (counting stopped; --all-followers counts them all)
```

[Using the client](client.md) goes on from here, and [Giving your AI agent Farcaster](agent.md) hands the same commands to an agent.

### Already running a Snapchain node?

If another guide has already given you a synced mainnet node, skip steps 2 to 4: install `paircaster` (step 1) and point it at that node. On the machine the node runs on, there is nothing to set, because `paircaster` asks `http://localhost:3381`, Snapchain's own port, however the node was installed. Elsewhere, reach the node's API [through SSH](client.md#where-to-run-paircaster), or name its address with `--node` on every command, reads and writes alike:

```run
paircaster node status --node http://127.0.0.1:3381
```

```output
Ready (Snapchain 0.14.3): every shard within a minute of its newest block.
  shard 0: height 48642234, 1s behind
  shard 1: height 49318266, 0s behind
  shard 2: height 49153538, 0s behind
```

If nothing answers at that address, it says `could not reach node` and exits 1. Without `--node`, reads and writes both go to your own node; some of the guide's writes were sent through Farcaster's public node instead, by naming it with `--node`, because the test node had not yet synced the new test account, and a node accepts writes only from accounts it has seen ([Writing](client.md#writing:-casts,-reactions,-follows)).

## What you just did

```
  your machine
 ┌──────────────────────────────────────────────┐
 │  paircaster ──HTTP──▶ localhost:3381           │
 │  (you, or your agent)     │                  │
 │                    Snapchain read node       │
 │                    (~/snapchain/.rocks)      │
 └───────────────────────────┬──────────────────┘
                             │ gossip, UDP 3382
                             ▼
                  other Snapchain nodes
                  and the validators
```

Your node keeps its own copy of the network's messages: casts, reactions, follows and profile data. Ours reports 914,246,336 messages in 557 GB. `paircaster` reads from the node at `http://localhost:3381` unless you point it elsewhere, so what you read does not pass through anyone else's server.

A read node does not decide what goes on the network; validators do. When you post, your node checks the message against what it has and passes it on to the validators, without adding it to its copy ([Snapchain's node types](https://snapchain.farcaster.xyz/node-types#message-submission-and-the-mempool)). The message is on the network only once it comes back in a read, which is why `paircaster` reads every write back before it calls it sent.

## The protocol in brief

Each idea is explained where the guide first uses it.

- **FID.** An account is a number, registered on-chain to its custody address ([Accounts and FIDs](client.md#accounts-and-fids)).
- **Custody address.** The Ethereum address that owns an FID. Its key comes from the recovery phrase and can do anything with the account ([Accounts and FIDs](client.md#accounts-and-fids)).
- **App key.** A key the custody key authorises to post as the account, and nothing more. `paircaster` adds one with a `KEY_ADD` message, which costs nothing, rather than an on-chain transaction ([Adding an app key](client.md#adding-an-app-key)).
- **Message.** Everything an account does, a cast, a like, a follow, a profile field, is a message signed by an app key ([Casts, and what a message is](client.md#casts,-and-what-a-message-is)).
- **Read node and validators.** Your node keeps a copy of the network and passes your writes on; validators decide what goes on the network ([What you just did](#what-you-just-did)).
- **Shard.** Snapchain splits accounts across shards, which a node syncs separately ([Watch it](node.md#watch-it)).
- **Storage.** Rented on-chain for a year at a time. Over its limit, an account's oldest messages are dropped ([Storage](client.md#storage)).

## The rest of the guide

1. [Running the node](node.md)
2. [Using the client](client.md)
3. [Giving your AI agent Farcaster](agent.md)
4. [What you can do with it](uses.md)
5. [Safety](safety.md)
6. [Testing Paircaster on a clean machine](testing.md)
7. [Checking a voucher](voucher.md)
8. [Contributions upstream](upstream.md)

## Not tried yet

- Linux, Intel Macs, and rented servers; the [renting](node.md#renting-a-server-instead) advice is untested.
- A node on a real remote server, reached [through SSH](client.md#where-to-run-paircaster); a write or `key add` through a tunnel. The tunnel was tried only against a stand-in server on the same machine.
- `key add` and `delete` through your own node. Three casts, a like and its undo, and a follow and its undo have gone through the test node; `key add` and the delete went through Farcaster's public node `crackle`.
- Typing <kbd>yes</kbd> at a write's prompt: the writes so far were sent by an agent with `--yes`, after a person approved the dry run's text.
- A cast sent with `--embed`.
- The [agent skill](agent.md) in agents other than Claude Code and Codex, writes through Codex, and whether harness rules prompt outside Claude Code's auto mode.
- `./snapchain.sh upgrade`; a shutdown or logout with the node running; a restart with Docker other than OrbStack.
- The viewer built on a machine that did not already have Bun, the `wasm32-unknown-unknown` target and `wasm-bindgen-cli`; the viewer in browsers other than Chrome.
- A node installed some other way than this guide's, apart from Farcaster's public node: reaching its API through a web proxy, and pointing `node status` at its install directory.

## Going further

- **Your node's API.** Everything `paircaster` reads is plain HTTP on `localhost:3381`, as the `curl` in [Casts, and what a message is](client.md#casts,-and-what-a-message-is) shows. Paging uses `reverse=true` (not `1`) and `pageToken`. To build your own interface, read the API directly or run `paircaster` with `--json`, whose output carries a version number.
- **Validators.** Your read node does not need to run one: it syncs from the network and forwards your writes to them.
- **Upstream.** Snapchain's documentation is at [snapchain.farcaster.xyz](https://snapchain.farcaster.xyz), its code at [github.com/farcasterxyz/snapchain](https://github.com/farcasterxyz/snapchain), and the protocol at [github.com/farcasterxyz/protocol](https://github.com/farcasterxyz/protocol). Where the documentation and what we saw disagree, the repository's `docs/upstream-fixes.md` lists it; what we have filed is on [Contributions upstream](upstream.md).

---

Paircaster is MIT-licensed: [github.com/cyberchitta/paircaster](https://github.com/cyberchitta/paircaster).
