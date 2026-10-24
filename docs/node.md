---
title: Running the node
---

Paircaster was called Selfcast; output captured before the rename shows the old name.

## Hardware and disk

Snapchain's README asks for 16 GB RAM, 4 vCPUs and 1.5 TB of disk; its getting-started page says 2 TB. What we measured:

| Item | Size | Checked with |
| --- | --- | --- |
| Snapshot, as downloaded | 5,691 chunks in 3 shards | `fetch-snapshot` output |
| Node database after unpacking | 557 GB | `du -sh ~/snapchain/.rocks` |
| Database size the node reports | 557,823,184,195 bytes | `paircaster info` |

The download keeps each shard's chunks until that shard is unpacked, then deletes them. So the most disk it uses is while the second large shard unpacks: the chunks not yet unpacked plus the two large shards already unpacked, about 770 GB. `fetch-snapshot` refuses to start with less than 800 GB, counting what it has already downloaded (`--skip-disk-check` overrides it).

The 770 GB peak is worked out from shard sizes (about 470 GB of chunks; 557 GB unpacked), not measured. Snapchain's own download keeps more on disk at once, which is why its docs ask for 1.5 to 2 TB.

### Renting a server instead

Look for 4 or more vCPUs, 16 GB of RAM and at least 1 TB of disk (a server's own disk is usually far smaller, so add a block-storage volume). Most providers bill by the hour, so a test that takes two days costs two days. One example, read from Hetzner's price page on 2026-10-21: CPX42, 8 vCPU, 16 GB RAM, 320 GB disk, €0.1122 an hour or €69.99 a month at most, excluding VAT, in its EU locations. The volume is extra; that page gives no price for it. When you are done, delete both the server and the volume.

<limit>

The stated minimum has not been tried: the test machine has more than it.

</limit>

## The snapshot

A new node does not replay Farcaster's history block by block. It starts from a *snapshot*: a copy of a node's database, published for new nodes to start from and split into chunks of about 90 MB. The one we fetched was dated 2026-10-18.

Snapchain can download the snapshot itself when it first starts, but that download cannot resume. A network gap of about 50 seconds stops the node, and the restart begins again at chunk 1. Worse, if the first shard has already been unpacked, the restart sees a non-empty database, skips the snapshot, and runs on an incomplete copy without saying so. On a home connection the download takes many hours, so `paircaster` has its own downloader, which can stop and pick up where it left off.

### Download

```run
paircaster node fetch-snapshot
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
paircaster node fetch-snapshot --jobs 8
```

```output
112 of 5691 chunks already on disk
resuming ~/snapchain/.selfcast-snapshot/shard-1/chunk_0115.bin.part at 11.5 MB
resuming ~/snapchain/.selfcast-snapshot/shard-1/chunk_0114.bin.part at 56.2 MB
…
```

`--jobs` is how many chunks download at once; the default is 4. On the test machine 8 was less than 10% faster, because the line, not the server, was the limit.

### Download in the background

To close the terminal while it runs, use `pause` and `resume` instead. `resume` starts the download in the background and appends its output to `~/snapchain/paircaster-fetch.log`; `pause` stops it, keeping everything downloaded.

```run
paircaster node resume
paircaster node pause
```

What `resume` writes to the log:

```output
=== resume 2026-10-19T11:02:01Z --jobs 4
2649 of 5691 chunks already on disk
resuming ~/snapchain/.selfcast-snapshot/shard-1/chunk_2652.bin.part at 60.0 MB
…
```

Only one download runs per install directory: a second `fetch-snapshot` or `resume` while one is running does not start another.

### Unpack

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

## Start the node

```run
cd ~/snapchain && ./snapchain.sh up
```

The node starts from the unpacked database without downloading again. `up` then follows the node's log; Ctrl-C stops following it and leaves the node running.

## Watch it

`paircaster node status` works in every phase: downloading the snapshot, unpacking, stopped, syncing and ready. It also reports free disk. While the node is behind:

```run
paircaster node status
```

```output
Syncing (Snapchain 0.14.3): 47h07m behind at most.
  shard 0: height 48391469, 23h49m behind
  shard 1: height 49002681, 41h55m behind
  shard 2: height 48819789, 47h07m behind
Disk: 3165 GB free of 3996 GB.
```

Snapchain splits the network into *shards*, each its own chain of blocks. A shard's *height* is the number of its newest block, and “behind” is how old that block is. The node reports *ready* once every shard is within a minute of its newest block. Accounts' casts, reactions, follows and profiles are on shards 1 and 2; shard 0 holds the blocks. So the first line's lag leaves shard 0 out, and when only shard 0 is behind, it says the shards that hold accounts are within a minute: reads are then current. Your node's own numbers:

```run
paircaster info
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

Shards 1 and 2 hold about 1.68 million accounts each and nearly all the messages; shard 0 holds no accounts, and `numShards` counts only the two that do. The snapshot has all three. The same data comes straight from Snapchain's API with `curl http://localhost:3381/v1/info`, unformatted.

## Why sync is slow

The test node started 25 to 48 hours behind. What we saw while it caught up:

- While the machine was awake and the node was catching up, it gained about 2 seconds of the network's time per second, in samples of 35 s to 2 min.
- For over two hours, the furthest-behind shard made no net progress, with the container up the whole time. We have not found why.
- CPU and bandwidth were not the limit: over 30 s the node used about 3% CPU and received about 20 KB/s.

Snapchain v0.14.3 syncs through the Malachite library, pinned at commit `13bca14`, which asks a peer for one block at a time and waits for the answer before asking for the next (`code/crates/sync/src/handle.rs`, `request_value`). From the test machine a peer was about 250 ms away, which fits about 3 blocks per second per shard. Newer Malachite can ask for several blocks at once; no Snapchain release uses it, as of v0.14.3.

<limit>

We have not shown that distance to the peers is what limits sync, and we have not run a node close to them. This page gives no sync time because the one estimate we had did not hold.

</limit>

## Ports, and who can reach your node

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

Then restart the node, and once `up` is showing the log, press Ctrl-C and check:

```run
cd ~/snapchain && ./snapchain.sh down && ./snapchain.sh up
docker port snapchain-snapchain-1
```

```output
3381/tcp -> 127.0.0.1:3381
3382/udp -> 0.0.0.0:3382
3382/udp -> [::]:3382
3383/tcp -> 127.0.0.1:3383
```

With this in place, the HTTP API answered on `localhost` and refused a connection to the machine's LAN address.

## Logs

The `RUST_LOG` line in the file above keeps warnings and errors only. Without it, the sync library logs a “SYNC REQUIRED” warning for every block while the node catches up: about 0.9 MB of log a minute on the test machine, against about 6 KB with it. To read the log:

```run
docker logs --tail 4 snapchain-snapchain-1
```

```output
2026-10-20T10:06:08.093084Z  WARN snapchain::network::gossip: Failed to publish gossip message: InsufficientPeers ("read-node-peers")
2026-10-20T10:06:17.991223Z  WARN snapchain::network::gossip: Failed to publish gossip message: InsufficientPeers ("read-node-peers")
…
```

The log lives with the container and goes when the container is removed: by `./snapchain.sh down`, or when it is recreated after a change to its ports.

## Sleep, shutdown and logout

With OrbStack set to pause in sleep and the lid closed for 40 minutes, the node wrote nothing, the container did not restart, and on waking it reconnected to peers and caught up at the same rate as before. It came back 40 minutes further behind. A node on a laptop is only syncing while the laptop is awake.

After a restart with the node running, it came back without being started, and had caught up within minutes. Two settings do this: OrbStack's `app.start_at_login` (`orb config show`) starts Docker when you log in, and Snapchain's `docker-compose.yml` gives the node's container `restart: always`, so Docker starts it again. OrbStack is an app, so the node is not expected to start before someone logs in. With another Docker setup, check that Docker itself starts at boot or login. A container removed by `./snapchain.sh down` is not there to restart; start it with `./snapchain.sh up`.

## Upgrades

The bootstrap script adds a weekly cron job that upgrades Snapchain, unless `~/snapchain/.env` contains `SKIP_CRONTAB=true`. The quickstart adds it, so upgrades happen by hand. It also lets you leave the operator FID blank: without it, a blank FID stops the script before it starts the node, because it needs the FID to pick a day for the weekly job ([snapchain #1030](https://github.com/farcasterxyz/snapchain/issues/1030)). The weekly job also runs `docker system prune --volumes -f` after each upgrade (the `autoupgrade` branch of `snapchain.sh`), which deletes every stopped container and unused volume on the machine, not only Snapchain's. On a server used only for the node that is harmless; on your own computer it is not.

Upgrade when the network moves to a new Snapchain version. A node still on the old version stops following a shard at the first message the new rules allow and the old ones refuse: `node status` shows that shard falling behind while the others keep up, and `docker logs snapchain-snapchain-1` shows a panic with `merkle trie root hash mismatch`. The node's version is in `node status`; the newest is the top tag on [Docker Hub](https://hub.docker.com/r/farcasterxyz/snapchain/tags). To upgrade:

```run
cd ~/snapchain && docker compose pull snapchain && docker compose up -d snapchain
```

The compose file names the image `latest`, so this fetches the newest and restarts the node on it; the shard that stopped resumes. Snapchain's own `./snapchain.sh upgrade` also updates its script and compose file, but on an install like this guide's, with no Grafana running, it waits for Grafana forever before it restarts the node.

---

Next: [Using the client](client.md)
