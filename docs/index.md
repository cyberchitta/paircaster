---
title: Selfcast
dek: Farcaster from your own Snapchain node, for people and their AI agents. No API keys from anyone.
---

<note>

**Draft.** Every command and every output below comes from one macOS machine, run 2026-10-18 to 2026-10-21. None of it has been run on a fresh machine yet. The writes and `key add` went through Farcaster's public node `crackle`, while this machine's own node was still catching up; none has gone through our own node yet. Commands that were never run at all are marked *Not yet run*.

Paths under your home directory are shown as `~`.

</note>

1. [Quickstart](#quickstart)
2. [What you just did](#what-you-just-did)
3. [Running the node](#running-the-node)
4. [Using the client](#using-the-client)
5. [Filtering by client](#filtering-by-client)
6. [Giving your AI agent Farcaster](#giving-your-ai-agent-farcaster)
7. [Safety](#safety)
8. [Going further](#going-further)

## Quickstart

You need Docker and about 800 GB of free disk while the snapshot downloads and unpacks; the node then uses 557 GB and grows ([details](#hardware-and-disk)). Each step below is explained in [Running the node](#running-the-node).

### 1. Install selfcast

```run
git clone https://github.com/cyberchitta/selfcast
cd selfcast
cargo install --path crates/cli --locked
```

<not-run>

There is no release binary yet. On the test machine `selfcast` was built from this repository with `cargo build --release`; the clone and install above have not been run as written.

</not-run>

### 2. Install Snapchain, then stop it

```run
mkdir -p ~/snapchain && echo SKIP_CRONTAB=true >> ~/snapchain/.env
curl -sSL https://raw.githubusercontent.com/farcasterxyz/snapchain/refs/heads/main/scripts/snapchain-bootstrap.sh | bash
```

Run 2026-10-18. The script asks two questions: whether you agree that a read node earns no rewards, and an operator FID, which you can leave blank because of the first line. Without `SKIP_CRONTAB`, a blank FID stops the script before it starts the node: it needs the FID to pick a day for its weekly upgrade job ([snapchain #1030](https://github.com/farcasterxyz/snapchain/issues/1030)). With the line, upgrades happen [by hand](#upgrades). It installs to `~/snapchain` and starts the node, which begins its own snapshot download. That download cannot resume after an interruption ([why](#the-snapshot)), so stop it and let `selfcast` download instead:

```run
cd ~/snapchain && ./snapchain.sh down
```

<not-run>

On the test machine `~/snapchain/.env` held `SKIP_CRONTAB=true` before the script ran, but the command that wrote it was not recorded. The stop on a blank FID without it is read from `scripts/snapchain.sh` (`setup_crontab`), not run.

Snapchain's own download was stopped on the test machine, but the exact command was not recorded. `selfcast node fetch-snapshot` refuses to start while the node's container is running and names this command.

</not-run>

### 3. Download the snapshot and start the node

```run
selfcast node fetch-snapshot
cd ~/snapchain && ./snapchain.sh up
selfcast node status
```

The download and unpack took about 12 h 15 min of wall-clock time on the test machine, pauses included, at 8 to 41 MB/s. Once the node is up, it has to catch up from the snapshot's date to now. Four hours after starting, the test node was [still catching up](#why-sync-is-slow):

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

## Running the node

### Hardware and disk

Snapchain's README asks for 16 GB RAM, 4 vCPUs and 1.5 TB of disk; its getting-started page says 2 TB. What we measured:

| Item | Size | Checked with |
| --- | --- | --- |
| Snapshot, as downloaded | 5,691 chunks in 3 shards | `fetch-snapshot` output |
| Node database after unpacking | 557 GB | `du -sh ~/snapchain/.rocks` |
| Database size the node reports | 557,823,184,195 bytes | `selfcast info` |

The download keeps each shard's chunks until that shard is unpacked, then deletes them. So the most disk it uses is while the second large shard unpacks: the chunks not yet unpacked plus the two large shards already unpacked, about 770 GB. `fetch-snapshot` refuses to start with less than 800 GB, counting what it has already downloaded (`--skip-disk-check` overrides it).

<not-run>

The 770 GB peak is worked out from shard sizes (about 470 GB of chunks; 557 GB unpacked), not measured. Snapchain's own download keeps more on disk at once, which is why its docs ask for 1.5 to 2 TB.

</not-run>

#### Renting a server instead

Look for 4 or more vCPUs, 16 GB of RAM and at least 1 TB of disk (a server's own disk is usually far smaller, so add a block-storage volume). Most providers bill by the hour, so a test that takes two days costs two days. One example, read from Hetzner's price page on 2026-10-21: CPX42, 8 vCPU, 16 GB RAM, 320 GB disk, €0.1122 an hour or €69.99 a month at most, excluding VAT, in its EU locations. The volume is extra; that page gives no price for it. When you are done, delete both the server and the volume: a stopped server is still billed.

<not-run>

We have not rented a server for Selfcast, so none of this has been run on one. "A stopped server is still billed" is Hetzner's general rule as we understand it, not something we checked.

</not-run>

<limit>

The test machine was an Apple M5 Max (18 cores, 128 GB RAM, 4 TB disk) running macOS 26.7.1 with Docker through OrbStack. That is far above the stated minimum, so it shows nothing about whether the minimum is enough. No Linux machine or cloud VM has been tried.

</limit>

### The snapshot

A new node does not replay Farcaster's history block by block. It starts from a *snapshot*: a copy of a node's database, published for new nodes to start from and split into chunks of about 90 MB. The one we fetched was dated 2026-10-18.

Snapchain can download the snapshot itself when it first starts, but that download cannot resume. A network gap of about 50 seconds stops the node, and the restart begins again at chunk 1. Worse, if the first shard has already been unpacked, the restart sees a non-empty database, skips the snapshot, and runs on an incomplete copy without saying so. On a home connection the download takes many hours, so `selfcast` has its own downloader, which can stop and pick up where it left off.

#### Download

```run
selfcast node fetch-snapshot
```

```output
0 of 5691 chunks already on disk
[1/5691] shard 1 chunk_0003.bin  35.2 MB/s  ETA 7h03m
[2/5691] shard 1 chunk_0002.bin  35.5 MB/s  ETA 3h45m
[3/5691] shard 1 chunk_0004.bin  36.7 MB/s  ETA 3h09m
…
```

Ctrl-C stops it. Running it again continues, including chunks that were half done:

```run
selfcast node fetch-snapshot --jobs 8
```

```output
112 of 5691 chunks already on disk
resuming ~/snapchain/.selfcast-snapshot/shard-1/chunk_0115.bin.part at 11.5 MB
resuming ~/snapchain/.selfcast-snapshot/shard-1/chunk_0114.bin.part at 56.2 MB
…
```

`--jobs` is how many chunks download at once; the default is 4. On the test machine 8 was less than 10% faster, because the line, not the server, was the limit.

#### Download in the background

To close the terminal while it runs, use `pause` and `resume` instead. `resume` starts the download in the background and appends its output to `~/snapchain/selfcast-fetch.log`; `pause` stops it, keeping everything downloaded.

```run
selfcast node resume
selfcast node pause
```

What `resume` wrote to the log on the test machine:

```output
=== resume 2026-10-19T11:02:01Z --jobs 4
2649 of 5691 chunks already on disk
resuming ~/snapchain/.selfcast-snapshot/shard-1/chunk_2652.bin.part at 60.0 MB
…
```

Only one download runs per install directory: a second `fetch-snapshot` or `resume` while one is running does not start another.

#### Unpack

When the last chunk arrives, the download unpacks each shard into the node's database:

```output
[5691/5691] shard 0 chunk_0367.bin  8.1 MB/s  ETA 0h00m
unpacking shard 1 (2672 chunks)
unpacking shard 2 (2648 chunks)
unpacking shard 0 (371 chunks)
snapshot ready: 5691 chunks unpacked into ~/snapchain/.rocks
start the node: cd ~/snapchain && ./snapchain.sh up
```

Every pause leaves the snapshot older by the time you were paused, and the node has to sync that time back once it starts.

### Start the node

```run
cd ~/snapchain && ./snapchain.sh up
```

The node starts from the unpacked database without downloading again.

### Watch it

`selfcast node status` works in every phase: downloading the snapshot, unpacking, stopped, syncing and ready. It also reports free disk. While the node is behind:

```run
selfcast node status
```

```output
Syncing (Snapchain 0.14.3): 47h07m behind at most.
  shard 0: height 48391469, 23h49m behind
  shard 1: height 49002681, 41h55m behind
  shard 2: height 48819789, 47h07m behind
Disk: 3165 GB free of 3996 GB.
```

Snapchain splits the network into *shards*, each its own chain of blocks. A shard's *height* is the number of its newest block, and “behind” is how old that block is. The node reports *ready* once every shard is within a minute of its newest block. Your node's own numbers:

```run
selfcast info
```

```output
{
  "dbStats": {
    "approxSize": 557823184195,
    "numFidRegistrations": 3355165,
    "numMessages": 914246336
  },
  "nextEngineVersionTimestamp": 0,
  "numShards": 2,
  "peer_id": "12D3KooWEYXexrGCimknLUiKsEijFohY4HCJxo1vB4joawm8VCvS",
  "shardInfos": [
    {
      "approxSize": 38892955790,
      "blockDelay": 85744,
      "maxHeight": 48391470,
      "mempoolSize": 0,
      "numFidRegistrations": 0,
      "numMessages": 12668689,
      "shardId": 0
    },
    {
      "approxSize": 280148797489,
      "blockDelay": 150942,
      "maxHeight": 49002681,
      "mempoolSize": 4294967295,
      "numFidRegistrations": 1677734,
      "numMessages": 458613892,
      "shardId": 1
    },
    {
      "approxSize": 277674386706,
      "blockDelay": 169654,
      "maxHeight": 48819790,
      "mempoolSize": 4294967295,
      "numFidRegistrations": 1677431,
      "numMessages": 455632444,
      "shardId": 2
    }
  ],
  "version": "0.14.3"
}
```

Shards 1 and 2 hold about 1.68 million accounts each and nearly all the messages; shard 0 holds no accounts. The same data comes straight from Snapchain's API with `curl http://localhost:3381/v1/info`, unformatted.

### Why sync is slow

The test node started 2026-10-20 at 05:56 UTC, 25 to 48 hours behind. What we saw:

- While the machine was awake and the node was catching up, it gained about 2 seconds of the network's time per second, in samples of 35 s to 2 min.
- From 07:36 to 10:00 UTC the furthest-behind shard made no net progress, with the container up the whole time. We have not found why.
- CPU and bandwidth were not the limit: over 30 s the node used about 3% CPU and received about 20 KB/s.

Snapchain v0.14.3 syncs through the Malachite library, pinned at commit `13bca14`, which asks a peer for one block at a time and waits for the answer before asking for the next (`code/crates/sync/src/handle.rs`, `request_value`). From the test machine a peer was about 250 ms away, which fits about 3 blocks per second per shard. Newer Malachite can ask for several blocks at once; no Snapchain release uses it, as of v0.14.3.

<limit>

We have not shown that distance to the peers is what limits sync, and we have not run a node close to them. This page gives no sync time because the one estimate we had did not hold.

</limit>

### Ports, and who can reach your node

| Port | Protocol | Used for |
| --- | --- | --- |
| 3381 | TCP | HTTP API: reads, and submitting messages |
| 3382 | UDP | Gossip with other nodes |
| 3383 | TCP | gRPC API |

Snapchain's compose file publishes these on every network interface. On the test machine that put the HTTP API, including message submission, within reach of anything on the local network. To keep the APIs on your machine and leave only gossip open, add this file next to Snapchain's `docker-compose.yml`. `./snapchain.sh upgrade` replaces `docker-compose.yml` but leaves this file alone.

```file ~/snapchain/docker-compose.override.yml
services:
  snapchain:
    environment:
      RUST_LOG: "warn,informalsystems_malachitebft_sync=error"
    ports: !override
      - "127.0.0.1:3381:3381/tcp"
      - "3382:3382/udp"
      - "127.0.0.1:3383:3383/tcp"
  statsd:
    ports: !override
      - "127.0.0.1:8125:8125/udp"
      - "127.0.0.1:8126:8126"
```

Then restart the node and check:

<not-run>

How the node was restarted after adding this file on the test machine was not recorded. The check below was run.

</not-run>

```run
docker port snapchain-snapchain-1
```

```output
3381/tcp -> 127.0.0.1:3381
3382/udp -> 0.0.0.0:3382
3382/udp -> [::]:3382
3383/tcp -> 127.0.0.1:3383
```

With this in place, the HTTP API answered on `localhost` and refused a connection to the machine's LAN address.

### Logs

The `RUST_LOG` line in the file above keeps warnings and errors only. Without it, the sync library logs a “SYNC REQUIRED” warning for every block while the node catches up: about 0.9 MB of log a minute on the test machine, against about 6 KB with it. To read the log:

```run
docker logs --tail 4 snapchain-snapchain-1
```

```output
2026-10-20T10:06:08.093084Z  WARN snapchain::network::gossip: Failed to publish gossip message: InsufficientPeers ("read-node-peers")
2026-10-20T10:06:17.991223Z  WARN snapchain::network::gossip: Failed to publish gossip message: InsufficientPeers ("read-node-peers")
…
```

The log lives with the container and goes when the container is recreated, for example after changing its ports.

### Sleep, shutdown and logout

Tested on macOS with OrbStack set to pause in sleep: with the lid closed for 40 minutes, the node wrote nothing, the container did not restart, and on waking it reconnected to peers and caught up at the same rate as before. It came back 40 minutes further behind. A node on a laptop is only syncing while the laptop is awake.

<limit>

Not tested: what happens after a reboot or a logout, whether Docker starts again at login, and whether quitting Docker stops the node cleanly. OrbStack has a setting for each of the last two; both were turned on but neither was tried.

</limit>

### Upgrades

The bootstrap script adds a weekly cron job that upgrades Snapchain, unless `~/snapchain/.env` contains `SKIP_CRONTAB=true`. The quickstart adds it, so upgrades happen by hand. The weekly job also runs `docker system prune --volumes -f` after each upgrade (the `autoupgrade` branch of `snapchain.sh`), which deletes every stopped container and unused volume on the machine, not only Snapchain's. On a server used only for the node that is harmless; on your own computer it is not. To upgrade:

```run
cd ~/snapchain && ./snapchain.sh upgrade
```

<not-run>

No upgrade has been run yet.

</not-run>

## Using the client

Every read below goes to your own node's HTTP API and nowhere else. The examples were run on 2026-10-20 at about 12:15 UTC, while the test node was 44 hours behind the network, so nothing newer than that shows up in them.

### Accounts and FIDs

A Farcaster account is a number, its *FID*. `selfcast` names accounts by FID; it has no username lookup yet.

```run
selfcast profile 3
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

The name, bio and username are the account's own *user data* messages (`/v1/userDataByFid`). The username, `dwr`, is an *fname*: one of those messages, not part of the account itself. A follow is a *link* message from one account to another: `following` counts the account's own (`/v1/linksByFid`), `followers` counts the ones aimed at it (`/v1/linksByTargetFid`). The node keeps no follower total, so `selfcast` reads followers 1000 at a time and stops at 10,000. `selfcast profile 3 --all-followers` reads every one: 630,232 for this account, in 7.6 seconds (run 2026-10-21; a first, cold read on 2026-10-20 took 36 seconds).

### Casts, and what a message is

```run
selfcast casts 2,3 --limit 3
```

```output
2026-10-17T02:30:02Z  fid 3  0x0256ae66308d546514fa666cda44052abbfea066  app 9152

2026-10-17T02:23:28Z  fid 3  0xec0c8a227d7f34ad3e2243ae098b46459b7d1847  app 9152
  Neat

2026-10-16T00:31:40Z  fid 3  0x40a0d26b7ea3c4ef79b1f574ff2261a535cb84c6  app 9152
  I just want it for me.
```

The newest casts of FIDs 2 and 3, merged; FID 2 had nothing newer. The first has no text, only an image. Each line gives the time, the author, the cast's hash and the app it was posted from ([below](#filtering-by-client)). Here is the first one as the node stores it:

```run
curl -s 'http://localhost:3381/v1/castsByFid?fid=3&pageSize=1&reverse=true' | jq '.messages[0] | {type: .data.type, fid: .data.fid, timestamp: .data.timestamp, hash, hashScheme, signatureScheme, signer}'
```

```output
{
  "type": "MESSAGE_TYPE_CAST_ADD",
  "fid": 3,
  "timestamp": 182745002,
  "hash": "0x0256ae66308d546514fa666cda44052abbfea066",
  "hashScheme": "HASH_SCHEME_BLAKE3",
  "signatureScheme": "SIGNATURE_SCHEME_ED25519",
  "signer": "0xc887f5bf385a4718eaee166481f1832198938cf33e98a82dc81a0b4b81ffe33d"
}
```

Everything an account does on Farcaster is a *message* like this one: a cast, a like, a follow, a profile field. The hash is the first 20 bytes of the BLAKE3 hash of the message's data, and `signer` is the public half of the *app key* that signed that hash with Ed25519. The timestamp counts seconds from 2021-01-01 UTC; `selfcast` shows it as an ordinary date. The node checks the signature and that the key belongs to the account, so nobody else can post as it.

`--limit` (default 20, at most 1000) is also how many of each account's newest messages are read, so a removed cast counts against it, and there is no paging further back. Add `--json` to any read for output a program can use:

```run
selfcast casts 3 --limit 1 --json
```

```output
{"version":1,"fids":[3],"casts":[{"hash":"0x0256ae66308d546514fa666cda44052abbfea066","fid":3,"timestamp":"2026-10-17T02:30:02Z","text":"","mentions":[],"embeds":[{"url":"https://imagedelivery.net/BXluQx4ige9GuW0Ia56BHw/9f4bda20-a039-4770-5605-8eb24d259e00/original"}],"parent":null,"signer":"0xc887f5bf385a4718eaee166481f1832198938cf33e98a82dc81a0b4b81ffe33d","app_fid":9152}]}
```

### The feed

```run
selfcast feed 2 --limit 5
```

The same as `casts`, over every account FID 2 follows: what that account would see. It reads the follow list, then each followed account's casts, eight accounts at a time. Casts dated in the future are left out.

### Writing: casts, reactions, follows

<not-run>

Run once, on 2026-10-21, from a test account (`@selfcast`, FID 3356360): a cast, a like, an unlike, a delete, a follow and an unfollow, each read back afterwards. They went through Farcaster's public node `crackle`, because our own node had not yet synced the new account; none has gone through our own node yet. An AI agent ran each one with `--yes`, after the person approved the text from a dry run, so the prompt where you type <kbd>yes</kbd> has not been seen for a write. Replies, embeds and `resend` have not been run.

</not-run>

| Command | Message it sends |
| --- | --- |
| `cast <text>`, `--reply-to FID:HASH`, `--embed URL\|FID:HASH` | Cast add |
| `delete <hash>` | Cast remove |
| `react like\|recast FID:HASH`, `unreact …` | Reaction add, reaction remove |
| `follow <fid>`, `unfollow <fid>` | Link add, link remove |

```run
selfcast cast --help
```

```output
Post a cast, or a reply with --reply-to

Usage: selfcast cast [OPTIONS] <TEXT>

Arguments:
  <TEXT>  The text, exactly as it will be posted. `@name` is not turned into a mention. Over 320 bytes it is sent as a long cast, up to 1024

Options:
      --node <NODE>           Node HTTP API base URL [env: SELFCAST_NODE=] [default: http://localhost:3381]
      --reply-to <FID:HASH>   The cast this replies to, as FID:HASH
      --embed <URL|FID:HASH>  A URL, or a cast as FID:HASH, to embed. Repeatable
      --json                  Print machine-readable JSON instead of human output
      --fid <FID>             The account to write as, when keys for several are saved
      --yes                   Send without asking at the terminal. For an agent whose harness asks the user before each write command
      --dry-run               Sign, show, and ask the node whether it would accept the message. Nothing is saved or sent
  -h, --help                  Print help
```

A dry run signs the message and asks the node whether it would accept it. Nothing is sent:

```run
selfcast follow 1 --node https://crackle.farcaster.xyz:3381 --dry-run
```

```output
Send as @selfcast (FID 3356360), signed by app key 0xffac75748030d214200d85986fecf5053a91d096749c4dd2cf41b7915e61212e, dated 2026-10-21T06:25:37Z:

  Follow FID 1
  FID 1 is @farcaster on this node.
Dry run: the node would accept 0x3a91056f44dbc7de30108f12e4908ee40b729e13. Nothing was sent.
```

Everything above the last two lines is written from the signed message. The line naming `@farcaster` is the node's answer for FID 1, so you can see whom you are about to follow. The first write, as the agent sent it:

```run
selfcast cast "This cast will be deleted with selfcast delete. Deleting removes it from the network's current state; anyone who already read it keeps a copy." --node https://crackle.farcaster.xyz:3381 --yes --json
```

```output
Send as @selfcast (FID 3356360), signed by app key 0xffac75748030d214200d85986fecf5053a91d096749c4dd2cf41b7915e61212e, dated 2026-10-21T05:58:14Z:

  Cast:
    | This cast will be deleted with selfcast delete. Deleting removes it from the network's current state; anyone who already read it keeps a copy.
{"version":1,"status":"sent","kind":"cast","fid":3356360,"hash":"0x879a1298bb749bcebd52c08fb3ff1ab166a42bcc"}
```

The rest went the same way. Each hash names a message any node can be asked for:

| Write | Message hash |
| --- | --- |
| Cast | `0x879a1298bb749bcebd52c08fb3ff1ab166a42bcc` |
| Like it | `0x587ade3ad3134783b293109c41f475617c8482f0` |
| Undo the like | `0xa352092ac2ecb2087ebaf08c936becc6467f8dbc` |
| Delete it | `0xf1f1a93b7ebb7d840572fed96a9baf855edc1ae2` |
| Follow FID 1 | `0x42ab0000cf5486129cd3b815cbaa0f9386af0c2e` |
| Unfollow FID 1 | `0x1c0d573152f7c4f879633f3af275b70d9b6d9d1b` |

A delete and an undo are messages of their own. After the delete, the node answers `/v1/castById` for the cast with “not found”, and `/v1/castsByFid` lists only the removal. Anyone who read the cast before then still has it.

How a write goes:

1. `selfcast` encodes the message once, hashes and signs those bytes with your app key, and sends exactly those bytes. Nothing is re-encoded in between.
2. It shows you what it is about to send, written from the signed message itself, starting `Send as @<username> (FID <fid>), signed by app key <key>, dated <time>:`, and asks you to type <kbd>yes</kbd> at the terminal (not yet seen for a write; see above).
3. It saves the signed message to an outbox, then submits it to your node (`POST /v1/submitMessage`). Your node checks it against what it holds and passes it on to the validators; it does not keep it.
4. It reads the message back from your node. Only then does it report the write as sent; every write above was read back. If it does not show up within a minute, `selfcast resend` sends the saved message again later (not yet run).

Because your node checks a write against what it holds, it has to be synced to accept one.

An account needs *storage* to post: units rented on-chain, which the node records. Snapchain refuses messages from an account with none, and when an account is over what its storage allows, it removes that account's oldest messages of that kind (Snapchain 0.14.3 source, `src/mempool/` and `src/storage/store/engine.rs`). Neither has been seen from a node here.

### Adding an app key

Writes are signed with an *app key* (also called a signer), which `selfcast key add` creates and registers. Your account itself is owned by a different key, the *custody key*, which comes from your recovery phrase and can do anything with the account. An app key can only post messages as it.

```run
selfcast key add --help
```

```output
Register a new app key, authorised by the account's recovery phrase, which is read once from the terminal and never stored. Mainnet only

Usage: selfcast key add [OPTIONS]

Options:
      --node <NODE>  Node HTTP API base URL [env: SELFCAST_NODE=] [default: http://localhost:3381]
      --json         Print machine-readable JSON instead of human output
  -h, --help         Print help
```

Run on 2026-10-21 for the test account, through `crackle`, which had seen the new account within minutes; our own node had not. The person ran it in their own terminal; the recovery phrase prompt above this output is left out:

```run
export SELFCAST_NODE=https://crackle.farcaster.xyz:3381
selfcast key add
```

```output
Add an app key to account @selfcast (FID 3356360), custody address 0x337596dD506856c952a89554DE28064146b901D0.

  New key:     0xffac75748030d214200d85986fecf5053a91d096749c4dd2cf41b7915e61212e
  May sign:    casts, deleting casts, likes and recasts, undoing them, follows, unfollows
  Expires:     90 days after it was last used
  Nonce:       1
  Sign by:     2026-10-21T06:03:04Z (the authorisation lapses after this)
  Saved to:    ~/.config/selfcast/keys/3356360-ffac75748030d214200d85986fecf5053a91d096749c4dd2cf41b7915e61212e.key

It cannot change the account, move funds, or add or remove other keys.
Type yes to sign and send: yes
Added: app key 0xffac75748030d214200d85986fecf5053a91d096749c4dd2cf41b7915e61212e for FID 3356360.
Saved at ~/.config/selfcast/keys/3356360-ffac75748030d214200d85986fecf5053a91d096749c4dd2cf41b7915e61212e.key.
```

Check the custody address before typing <kbd>yes</kbd>. The Farcaster app shows two phrases, a Farcaster recovery phrase and a separate wallet one. It was the Farcaster one that gave this account's custody address.

What it does:

1. It asks for the recovery phrase at the terminal, with typing hidden, and derives the custody key from it.
2. It looks up which FID that custody key owns, on your node.
3. It creates a new app key and shows what the custody key is about to authorise: the account, the new key, what it may sign (casts, deleting casts, likes and recasts, undoing them, follows, unfollows), that it expires 90 days after it was last used, the deadline for the authorisation, and where the key will be saved. You type <kbd>yes</kbd>.
4. The custody key signs the authorisation, and is then dropped. The app key is saved at mode 0600.
5. The authorisation goes to your node as a `KEY_ADD` message: a Snapchain message, not a chain transaction, so it costs nothing. `selfcast` reads the key back from the node before it calls it added.

## Filtering by client

Every app key is registered with the FID of the app that asked for it. The node lists an account's keys with that FID (`/v1/signersByFid`), and every cast carries the key that signed it, so `selfcast` can say which app each cast came from. FID 9152 is the Farcaster app (its own profile reads `warpcast`).

FID 194 posts from two apps. Grouping the casts among its newest 100 messages by app:

```run
selfcast casts 194 --limit 100 --json | jq -c '.casts | group_by(.app_fid) | map({app_fid: .[0].app_fid, casts: length})'
```

```output
[{"app_fid":194,"casts":31},{"app_fid":9152,"casts":66}]
```

97 casts from 100 messages; the other three were removes. `--include-app` keeps only the apps named, `--exclude-app` drops them:

```run
selfcast casts 194 --limit 100 --include-app 194 --json | jq '.casts | length'
selfcast casts 194 --limit 100 --exclude-app 194 --json | jq '.casts | length'
```

```output
31
66
```

```run
selfcast casts 194 --limit 20 --exclude-app 9152 --json | jq -r '.casts[:3][] | "\(.timestamp)  \(.hash)  app \(.app_fid)"'
```

```output
2026-10-16T00:43:06Z  0xa3a1f90c784bf8add82000e43315bfeb5e9fb341  app 194
2026-10-16T00:42:35Z  0xbcdc4fb9488efdb6219184b1f1e7c4f71b634310  app 194
2026-10-15T23:40:57Z  0xa5e3ab1283a76fb054795aad42d75b19cefc614d  app 194
```

Both flags take several FIDs, and work on `feed` too. They filter after `--limit`, so they can return fewer casts than asked for.

- A cast whose key is no longer listed, for instance one since removed, has no known app. Such casts are left out by `--include-app` and kept by `--exclude-app`.
- A key can name the account itself as its app, as FID 194's own app above does. Keys added by `selfcast key add` do the same (from its source), so casts sent with `selfcast` show your own FID as the app, not a separate one for Selfcast.

## Giving your AI agent Farcaster

The repository's `skill/` directory is an agent skill: instructions an AI agent loads before it runs `selfcast`. It calls the same commands as you do, with `--json`.

<not-run>

There is no install step yet. In Claude Code a skill is a directory under `~/.claude/skills/`, so copying `skill/` there as `~/.claude/skills/selfcast` should work; it has not been tried. The writes on this page were run by an agent (Claude Code, working in this repository) following the skill's write steps, without the skill installed.

</not-run>

What the skill tells the agent:

- **Check the node first.** Run `selfcast node status` before reading, and say how far behind the node is in every answer.
- **Reads only, until you say otherwise.** It runs no write until you enable writes in that conversation, and that lasts for the conversation only.
- **Your words, its relay.** For each write it runs the command with `--dry-run`, shows you the text `selfcast` printed, exactly as printed, and waits for your yes before running it again with `--yes`.
- **Never a recovery phrase.** It never runs `key add`, never asks for a phrase, and refuses one if it is pasted in: you run `key add` in your own terminal.

Each write is its own command so that your agent's harness can gate it apart from the reads: let reads run, ask you before each write, and refuse `selfcast key` to the agent. How to say that is up to the harness. These were tried in Claude Code on 2026-10-21:

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

The `deny` rule held: the agent's `selfcast key --help` was refused. It also refused a command that only contained the words, in text the agent was writing into this page. The `ask` rules did not prompt. With `--node` between `selfcast` and `cast` the pattern cannot match, and the session was in auto mode, which ran the writes without asking. Whether they prompt outside auto mode was not tried.

<limit>

Rules like these match the command's text, so another spelling (options first, another path, `sh -c`) or a mode that skips prompts gets past them, and harmless text can trip them. They are a convenience, not a boundary. On your machine an agent with a shell can send anything the app key allows. What holds is the key's scope: casts, deletes, reactions and follows, never the account, its funds or its keys; and `key add` reads the phrase only from a terminal, which an agent's shell does not have.

</limit>

What the agent cannot do yet: no reactions or replies to a cast (so no “most liked”), no search, no channels, no username lookup, nothing older than an account's newest 1000 messages.

## Safety

A recovery phrase is like `rm -rf`: useful, and not something to be careless with.

- **Use a test account.** Try this tutorial, and any software that asks for a recovery phrase, with an account you can afford to lose. Paste a phrase only into software from developers you trust, and prefer apps that ask your wallet to approve a key over ones that ask for the phrase.
- **The phrase is read from the terminal only.** `selfcast key add` opens the terminal device itself, so the phrase cannot come from a pipe, a file, an argument or an environment variable, and does not end up in your shell history or an agent's transcript. The phrase and the custody key are held in memory that is wiped when they are dropped, and only the app key is written to disk. That is how the code is written; no test checks the memory.
- **An app key cannot take the account.** It can sign casts, deletes, reactions and follows and their undos. It cannot move funds, change the account's owner, or add other keys. A key file that others can read is refused when `selfcast` loads it.
- **Keys expire.** A key added by `selfcast` expires 90 days after it was last used; each use pushes that back (seen in [Checking a voucher](#checking-a-voucher): `expiresAt` is `lastUsedAt` plus the `ttl`).

<limit>

**Removing a key keeps what it signed.** On Snapchain since 0.10.0 (October 2025), removing an app key does not delete the casts, reactions or follows it made; delete those first if that is what you want. This is from Snapchain's source (0.14.3, and its main branch on 2026-10-15); it has not been seen on a node here. Some Farcaster documentation still says the opposite. `selfcast` has no command to remove a key yet.

</limit>

## Checking a voucher

A voucher says they ran the tutorial. Part of that can be checked by anyone, on any node. We ask vouchers to post the vouching cast with `selfcast cast`, so it is signed by the app key Selfcast added for them. Look the account's keys up:

```run
curl -s "http://localhost:3381/v1/signersByFid?fid=FID" | jq '.signers[] | select(.source == "SIGNER_SOURCE_OFFCHAIN")'
```

A key added by `selfcast key add` is added without gas, names the account itself as the app (`requestFid` is the account's own FID), has a `ttl` of 7776000 seconds (90 days), and has six scopes: casts and their removal, reactions and their removal, follows and unfollows. The vouching cast's `signer` field should be that key. The test account's key, after the writes above:

```run
curl -s "https://crackle.farcaster.xyz:3381/v1/signersByFid?fid=3356360" | jq '.signers[] | select(.source == "SIGNER_SOURCE_OFFCHAIN")'
```

```output
{
  "source": "SIGNER_SOURCE_OFFCHAIN",
  "key": "0xffac75748030d214200d85986fecf5053a91d096749c4dd2cf41b7915e61212e",
  "keyType": 1,
  "fid": 3356360,
  "addedAt": 1792558984,
  "lastUsedAt": 1792563233,
  "ttl": 7776000,
  "expiresAt": 1800339233,
  "scopes": [
    "MESSAGE_TYPE_CAST_ADD",
    "MESSAGE_TYPE_CAST_REMOVE",
    "MESSAGE_TYPE_REACTION_ADD",
    "MESSAGE_TYPE_REACTION_REMOVE",
    "MESSAGE_TYPE_LINK_ADD",
    "MESSAGE_TYPE_LINK_REMOVE"
  ],
  "requestFid": 3356360,
  "nonce": 1
}
```

<limit>

This is evidence, not proof. Another client can make a key with the same settings, and at least one other entry uses the same six scopes with a shorter expiry. Nothing on chain shows which node a message went through, so the node half of a vouch rests on the voucher's report.

</limit>

## Going further

- **Your node's API.** Everything `selfcast` reads is plain HTTP on `localhost:3381`, as the `curl` above shows. Paging uses `reverse=true` (not `1`) and `pageToken`. To build your own interface, read the API directly or run `selfcast` with `--json`, whose output carries a version number.
- **Validators.** Your read node does not need to run one: it syncs from the network and forwards your writes to them.
- **Upstream.** Snapchain's documentation is at [snapchain.farcaster.xyz](https://snapchain.farcaster.xyz), its code at [github.com/farcasterxyz/snapchain](https://github.com/farcasterxyz/snapchain), and the protocol at [github.com/farcasterxyz/protocol](https://github.com/farcasterxyz/protocol). Where the documentation and what we saw disagree, the repository's `docs/upstream-fixes.md` lists it.

---

Selfcast is MIT-licensed: [github.com/cyberchitta/selfcast](https://github.com/cyberchitta/selfcast).
