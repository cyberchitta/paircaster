---
title: Selfcast
dek: Farcaster from your own Snapchain node, for people and their AI agents. No API keys from anyone.
---

<warning>

**Read the code before you give it your recovery phrase.** As with any code from the internet, review it yourself, or have someone you trust or an AI agent review it, and use a test account either way. [Safety](safety.md) lists the files that handle the phrase.

</warning>

<note>

**Selfcast is new.** This guide was run on one Mac (Apple Silicon, macOS, Docker through OrbStack), except what is [listed at the end](#not-tried-yet) as not tried yet. The outputs are from real runs while Selfcast was being built; where a command has changed since, the guide shows the current one, so what you see may differ in small ways. Paths under your home directory are shown as `~`. To run it on a clean machine and report what happened, see [Testing Selfcast](testing.md).

</note>

## What you get

- **Your own node, no API keys.** `selfcast` reads from the Snapchain node on your machine, at `http://localhost:3381` unless you point it elsewhere, so what you read does not pass through anyone else's server ([what you just did](#what-you-just-did)).
- **No server needed.** It can run on a machine you already use. Snapchain's own snapshot download starts over after a short network drop, so `selfcast` has its own, which you can pause and resume where it stopped. A node on a laptop syncs only while the laptop is awake; after a 40-minute sleep it reconnected and carried on ([The snapshot](node.md#the-snapshot), [Sleep](node.md#sleep,-shutdown-and-logout)).
- **Your recovery phrase stays in your terminal.** `selfcast key add` reads it from the terminal only, never from a file, an argument, a pipe or an environment variable, uses it once to add an app key, and writes only the app key to disk ([Safety](safety.md)).
- **You see what is signed, and every write is checked.** Before a write, `selfcast` shows the message it is about to sign, written from the signed message itself. After sending, it reads the message back from the node before it calls it sent ([Writing](client.md#writing:-casts,-reactions,-follows)).
- **Your AI agent uses the same commands.** A skill tells an agent to run `selfcast`, to stay read-only until you enable writes, and to show you `selfcast`'s own text before each one. Those are instructions, not a lock: an agent with a shell on your machine can send anything the app key allows ([Your AI agent](agent.md)).
- **Farcaster from the command line.** Read profiles, casts and a feed; cast and delete, like and unlike, follow and unfollow. Each cast shows the app that posted it, so you can keep or drop casts by app ([Filtering by client](client.md#filtering-by-client)).
- **What it costs.** Docker, about 800 GB of free disk while the snapshot downloads and unpacks, and time: the download and unpack took about 12 hours on the test machine, and the node then has to catch up with the network ([why that is slow](node.md#why-sync-is-slow)).

## Quickstart

You need Docker and about 800 GB of free disk while the snapshot downloads and unpacks; the node then uses 557 GB and grows ([details](node.md#hardware-and-disk)). Each step below is explained in [Running the node](node.md).

### 1. Install selfcast

```run
git clone https://github.com/cyberchitta/selfcast
cd selfcast
cargo install --path crates/cli --locked
```

There is no release binary yet, so this builds `selfcast` from source and needs a Rust toolchain.

### 2. Install Snapchain, then stop it

```run
mkdir -p ~/snapchain && echo SKIP_CRONTAB=true >> ~/snapchain/.env
curl -sSL https://raw.githubusercontent.com/farcasterxyz/snapchain/refs/heads/main/scripts/snapchain-bootstrap.sh | bash
```

The script asks two questions: whether you agree that a read node earns no rewards, and an operator FID, which you can leave blank because of the first line. Without `SKIP_CRONTAB`, a blank FID stops the script before it starts the node: it needs the FID to pick a day for its weekly upgrade job ([snapchain #1030](https://github.com/farcasterxyz/snapchain/issues/1030)). With the line, upgrades happen [by hand](node.md#upgrades). It installs to `~/snapchain` and starts the node, which begins its own snapshot download. That download cannot resume after an interruption ([why](node.md#the-snapshot)), so stop it and let `selfcast` download instead:

```run
cd ~/snapchain && ./snapchain.sh down
```

`selfcast node fetch-snapshot` refuses to start while the node's container is running, and names this command.

### 3. Download the snapshot and start the node

```run
selfcast node fetch-snapshot
cd ~/snapchain && ./snapchain.sh up
selfcast node status
```

`./snapchain.sh up` starts the node and then follows its log; Ctrl-C stops following the log and leaves the node running. The download and unpack took about 12 h 15 min of wall-clock time on the test machine, pauses included, at 8 to 41 MB/s. Once the node is up, it has to catch up from the snapshot's date to now. Four hours after starting, the test node was [still catching up](node.md#why-sync-is-slow):

```output
Syncing (Snapchain 0.14.3): 47h07m behind at most.
  shard 0: height 48391469, 23h49m behind
  shard 1: height 49002681, 41h55m behind
  shard 2: height 48819789, 47h07m behind
Disk: 3165 GB free of 3996 GB.
```

## What you just did

```
  your machine
 ┌──────────────────────────────────────────────┐
 │  selfcast ──HTTP──▶ localhost:3381           │
 │  (you, or your agent)     │                  │
 │                    Snapchain read node       │
 │                    (~/snapchain/.rocks)      │
 └───────────────────────────┬──────────────────┘
                             │ gossip, UDP 3382
                             ▼
                  other Snapchain nodes
                  and the validators
```

Your node keeps its own copy of the network's messages: casts, reactions, follows and profile data. Ours reports 914,246,336 messages in 557 GB. `selfcast` reads from the node at `http://localhost:3381` unless you point it elsewhere, so what you read does not pass through anyone else's server.

A read node does not decide what goes on the network; validators do. When you post, your node checks the message against what it has, passes it on to the validators, and keeps nothing itself. The message is on the network only once it comes back in a read, which is why `selfcast` reads every write back before it calls it sent.

## The rest of the guide

1. [Running the node](node.md)
2. [Using the client](client.md)
3. [Giving your AI agent Farcaster](agent.md)
4. [Safety](safety.md)
5. [Checking a voucher](voucher.md)
6. [Testing Selfcast on a clean machine](testing.md)

## Not tried yet

- Linux, Intel Macs, and rented servers; the [renting](node.md#renting-a-server-instead) advice is untested.
- A node on a real remote server, reached [through SSH](client.md#where-to-run-selfcast); a write or `key add` through a tunnel. The tunnel was tried only against a stand-in server on the same machine.
- `key add`, and any write other than a cast, through your own node. One cast has gone through the test node while it was still catching up; the other writes and `key add` went through Farcaster's public node `crackle`.
- Typing <kbd>yes</kbd> at a write's prompt: the writes so far were sent by an agent with `--yes`, after a person approved the dry run's text. Also embeds and `resend`.
- Installing the [agent skill](agent.md), and whether harness rules prompt outside Claude Code's auto mode.
- `./snapchain.sh upgrade`; a reboot or logout with the node running; whether Docker starts again at login.

## Going further

- **Your node's API.** Everything `selfcast` reads is plain HTTP on `localhost:3381`, as the `curl` in [Casts, and what a message is](client.md#casts,-and-what-a-message-is) shows. Paging uses `reverse=true` (not `1`) and `pageToken`. To build your own interface, read the API directly or run `selfcast` with `--json`, whose output carries a version number.
- **Validators.** Your read node does not need to run one: it syncs from the network and forwards your writes to them.
- **Upstream.** Snapchain's documentation is at [snapchain.farcaster.xyz](https://snapchain.farcaster.xyz), its code at [github.com/farcasterxyz/snapchain](https://github.com/farcasterxyz/snapchain), and the protocol at [github.com/farcasterxyz/protocol](https://github.com/farcasterxyz/protocol). Where the documentation and what we saw disagree, the repository's `docs/upstream-fixes.md` lists it.

---

Selfcast is MIT-licensed: [github.com/cyberchitta/selfcast](https://github.com/cyberchitta/selfcast).
