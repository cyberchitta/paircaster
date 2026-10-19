---
name: selfcast
description: Use the selfcast CLI to run the user's own Snapchain (Farcaster) node and read from it. Covers checking which phase the node is in (snapshot download, unpack, stopped, syncing, ready) and pausing or resuming the snapshot download. Load before running any `selfcast` command.
---

# selfcast

`selfcast` is a command-line client for Farcaster that talks to the user's
own Snapchain node, by default at `http://localhost:3381`. No API keys, no
hosted service.

## How to call it

- Add `--json` to every command that supports it, and read the JSON, not the
  human output. `node status --json` carries `"version"`; this skill
  describes version 1. If you see another version, tell the user this skill
  is out of date rather than guessing at the fields.
- Exit code 0 is success. On failure the command prints `error: …` to
  stderr and exits 1. Report the error text as it is.
- `--node <url>` (or `SELFCAST_NODE`) picks the node. Leave it unset unless
  the user names a node.
- Never ask for, accept, or type a recovery phrase. Commands that need one
  read it from the terminal themselves, and the user types it.

## Node

The node starts from a snapshot of the network: hundreds of gigabytes in
thousands of chunks, so the download takes hours. After the snapshot is
unpacked, the user starts the node and it syncs to the network's newest
blocks.

`--dir <path>` (or `SNAPCHAIN_DIR`) is the Snapchain install directory,
`~/snapchain` by default. Pass it to every `node` command when the user's
install is elsewhere.

### Checking: `selfcast node status --json`

Read-only. Use it whenever the user asks how the node is doing, and after
every `pause` or `resume`. Do not watch the download with `ps`, `du` or the
log file: `status` reads the same state. The download takes hours, so do not
poll it in a loop. Check when the user asks, or at long intervals if they
asked you to keep an eye on it.

`status` asks the node first and reads the download from disk only when no
node answers. If `--node` or `SELFCAST_NODE` points at another machine's
node, `status` reports that node, not a download on this machine.

`phase` says which fields are present. Every phase also has `disk`.
Where the table says *offer*, suggest the command; run it only if the user
asked for that outcome ("resume it", "fix it").

| `phase` | Fields | What it means, and what to offer |
|---|---|---|
| `not_started` | `running` | Nothing saved yet. `running: true`: a download has just started; check again shortly. `running: false`: offer to start it with `node resume`. |
| `downloading` | `running`, `chunks_done`, `chunks_total`, `bytes_per_second`, `eta_seconds`, `seconds_since_write` | `running: false`: paused or interrupted; offer `node resume`. `running: true` with `seconds_since_write` over 90: waiting for the network; the download carries on when it returns. `bytes_per_second` and `eta_seconds` are `null` until a few chunks have finished, and while not running. While not running, `seconds_since_write` is only how long ago it stopped. |
| `unpacking` | `running`, `shards_done`, `shards_total`, `seconds_since_write` | Local work after the download; it cannot be paused. `running: false`: interrupted; `node resume` finishes it, and the shard in progress starts over. |
| `node_stopped` | (none) | The snapshot is in place but no node answers. The node is started with Snapchain's own script, `cd <dir> && ./snapchain.sh up`, which can take a minute to answer. |
| `syncing` | `node_version`, `shards` | The node is running and catching up. Each shard has `id`, `height` and `seconds_behind`. |
| `ready` | `node_version`, `shards` | Every shard is within 60 seconds of its newest block. |

`disk` is `{ "free_bytes", "total_bytes", "low" }` for the filesystem
holding the install directory, or `null` if that directory does not exist
here. When `low` is `true` (under 15% free), tell the user: the snapshot
still to come and the node's growing database both need room.

Example, mid-download:

```json
{"version":1,"phase":"downloading","running":true,"chunks_done":3536,"chunks_total":5691,"bytes_per_second":18015894,"eta_seconds":10538,"seconds_since_write":0,"disk":{"free_bytes":3435320586240,"total_bytes":3996276899840,"low":false}}
```

When you report, give the user the numbers that matter: the phase, how far
along (chunks or shards, as a percentage), the speed in MB/s
(`bytes_per_second` / 1,000,000) and the time left, and anything to act on.

### Pausing and resuming: `selfcast node pause`, `selfcast node resume`

These change what runs on the user's machine. Run them only when the user
asks. Both print a line of plain text, not JSON. Follow either with
`node status --json` to see the result.

- `node resume` starts the download in the background, or picks it up where
  it stopped, and prints the process ID and the log path,
  `<dir>/selfcast-fetch.log`. It also finishes an interrupted unpack. If a
  download is already running, or the snapshot is already unpacked, it says
  so and changes nothing (exit 0). `--jobs <n>` sets how many chunks download
  at once (default 4).
- `node pause` stops the download. Everything downloaded stays on disk,
  including the part of each chunk in flight. With nothing running it says
  so (exit 0). During an unpack it refuses with an error.
- Only one download runs per install directory. A second `resume` or
  `fetch-snapshot` is refused while one runs.

**Limit:** whether a download started by `node resume` keeps running after
the terminal or agent session that started it closes has not been tested.
In a new session, check with `node status`, and offer `node resume` if it
shows `running: false`.

### Do not run

- `selfcast node fetch-snapshot`. It is the same download in the
  foreground, and it runs for hours. Use `node resume`.
- `fetch-snapshot --fresh`. It discards every chunk downloaded so far and
  starts from the newest snapshot. If the user wants to start over, say
  what it discards (a slow download is usually the network, which starting
  over does not fix), and give them the command to run in their own
  terminal: `selfcast node pause`, then `selfcast node fetch-snapshot
  --fresh`. To continue it later, use `node resume`, not `--fresh` again,
  which starts over each time.
- `./snapchain.sh up`, or anything else that starts or stops the node,
  unless the user asks. Tell them the command instead.
