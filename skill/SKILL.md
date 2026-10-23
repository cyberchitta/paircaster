---
name: selfcast
description: Use the selfcast CLI to run the user's own Snapchain (Farcaster) node and read from it. Covers checking which phase the node is in (snapshot download, unpack, stopped, syncing, ready), pausing or resuming the snapshot download, reading profiles (by FID or username), verified addresses, follower and following lists, feeds, accounts' casts, a channel's casts, threads, a cast's likes and recasts, and the network's recent events (for counting, and for finding the last three days' casts by their text) from the node, showing a thread to the user in the viewer page (in a browser or Shala), with worked examples (account stats, a feed of a list of accounts), and writing (casts, deletes, reactions, follows) once the user enables it, with the CLI's own confirmation text shown verbatim and harness permission rules that gate each write. Load before running any `selfcast` command.
---

# selfcast

`selfcast` is a command-line client for Farcaster that talks to the user's
own Snapchain node, by default at `http://localhost:3381`. No API keys, no
hosted service.

## How to call it

- Add `--json` to every command that supports it, and read the JSON, not the
  human output. The JSON of `node status`, `profile`, `feed` and `casts`
  carries `"version"`; this skill describes version 1 of each. If you see
  another version, tell the user this skill is out of date rather than
  guessing at the fields. (`info --json` is the node's own `/v1/info`
  response, passed through unversioned; prefer `node status`.)
- Exit code 0 is success. When the command runs and fails, it prints
  `error: …` to stderr and exits 1. When it was called wrongly (an unknown
  subcommand, a missing or invalid argument), it prints `error: …` and a
  usage hint and exits 2: fix the command rather than reporting a failure.
  Report the error text as it is.
- `--node <url>` picks the node; without it, the node on this machine
  (`http://localhost:3381`). Leave it out unless the user names a node, and
  when they do, put it in every command, so the node a command uses is
  always in the command.
- Copy each `FID:HASH` whole from a command's output. Never type one from
  memory, shorten it, or finish a partial one: a wrong hash is a cast the
  node does not hold.
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

Without `--node`, `status` reports the node installed on this machine: it
asks the node first, and reads the download from the install directory only
when no node answers. With `--node`, it reports that node only, wherever it
is, with no download and `disk: null`; if that node does not answer, it is
an error. Use `--node` for a node reached through an SSH tunnel too: its
address is `localhost`, but it is not this machine's node.

If `status` fails with `not permitted to connect`, your shell's sandbox is
blocking network access, so nothing is known about the node: say so, and
tell the user `selfcast` needs network access (in Codex,
`-c sandbox_workspace_write.network_access=true`). Do not report the node
as stopped.

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
here or `--node` named a node. When `low` is `true` (under 15% free), tell
the user: the snapshot still to come and the node's growing database both
need room.

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

## Reads

Every read comes from the user's node and nowhere else. Wherever a command
takes an account (`<account>` below), give it the FID or the username:
`3`, `dwr`, `vitalik.eth`, `jesse.base.eth`. Digits alone are a FID; an
fname can be all digits, so write one as `@123`. When the user names an
account, pass the name as they gave it; don't look the FID up first. When
an account comes from a previous result, pass its `fid`: a FID never
changes, a name can, and each name is one more lookup. A name the node holds no
proof for is an error (`no account holds the username … on this node`),
exit 1. While the node is syncing it may simply not have the name yet.

### Check the node first

Before the first read in a conversation, run `selfcast node status --json`.
Reads work in `syncing` as well as `ready`, but while syncing the node
holds only what it has caught up to: nothing newer than each shard's
`seconds_behind`. Say so when you answer, with the lag in hours, for
example "from your node, which is about 46 hours behind". Example, from a
node catching up:

```json
{"version":1,"phase":"syncing","node_version":"0.14.3","shards":[{"id":0,"height":48401912,"seconds_behind":80369},{"id":1,"height":49013085,"seconds_behind":145673},{"id":2,"height":48830242,"seconds_behind":164323}],"disk":{"free_bytes":3164618342400,"total_bytes":3996276899840,"low":false}}
```

An account's data lives on one shard, and the CLI does not say which, so
quote the largest `seconds_behind`. In any other phase there is no node to
read from: report the phase instead.

### `selfcast profile <account> --json`

```json
{"version":1,"fid":3,"username":"dwr","display_name":"Dan Romero","bio":"Interested in technology and other stuff.","pfp_url":"https://imagedelivery.net/…/original","url":"https://danromero.org","location":null,"twitter":"dwr","github":null,"banner_url":"https://imagedelivery.net/…/original","following":79,"followers":10000,"followers_exact":false}
```

(Image URLs shortened here.) Each text field is `null` when not set. An
FID the node has no registration for is an error (`is not registered on
this node`), exit 1: either no such account, or one newer than what a
syncing node has caught up to. Check `node status` before saying which.
`following` and `followers` are counts as the node holds them. The node
keeps no follower total, so followers are read 1000 at a time, and counting
stops at 10,000: then `followers_exact` is `false` and `followers` is a
lower bound ("at least"). Say so when you report it. `--all-followers`
counts every one, which is slow for a large account: for dwr, 630,231
followers took 5 to 8 seconds once the node had them cached and 26 on a
first, cold read, and balajis.eth on `crackle` did not finish in 120
seconds.
Use it only when the user wants the exact number, once per account.

### `selfcast followers <account> --json`, `selfcast following <account> --json`

The accounts that follow `<account>`, or that it follows, newest follow
first:

```json
{"version":1,"fid":2,"following":[{"fid":1099392,"timestamp":"2026-01-14T17:12:12Z"},{"fid":639226,"timestamp":"2026-01-09T10:22:44Z"}],"accounts":[{"fid":639226,"username":"iamtherealyakuza.eth","display_name":"Yakuza","pfp_url":"https://…/original"},{"fid":1099392,"username":"kajuthenut","display_name":"Kaju","pfp_url":"https://…/original"}]}
```

The list is `followers` or `following`, each `{fid, timestamp}` with
`timestamp` when the follow was made. `accounts` is as in `thread`.
`--limit` (default 100, at most 10,000) keeps the newest; `profile` gives
the totals, so say when a list is not all of them. An account the node has
no registration for exits 1.

### `selfcast addresses <account> --json`

The wallet addresses an account has verified, oldest first:

```json
{"version":1,"fid":5650,"addresses":[{"address":"0xd8da6bf26964af9d7eed9e03e53415d37aa96045","protocol":"ethereum","chain_id":0,"timestamp":"2023-06-16T15:58:40Z","primary":false},{"address":"0x96b6bb2bd2eba3b4fbefd7dbac448ad7b6cbf279","protocol":"ethereum","chain_id":0,"timestamp":"2025-03-22T04:03:48Z","primary":true}]}
```

(Shortened: this account has four.) `protocol` is `ethereum` or `solana`;
`primary` marks the one the account names as primary for that protocol;
`chain_id` is as the verification gives it. A verified address is a wallet
the account proved it controls, not the address that owns the account (its
custody address). An account with none gives `"addresses":[]`; one the
node has no registration for exits 1.

### `selfcast casts <account>... --json`

The newest casts of one or more accounts, merged, newest first. Accounts
are separate arguments or comma-separated. `fids` in the output lists each
account's FID, in the order given, names already looked up. `accounts`
holds each author and each mentioned account once, as in `thread`, so name
the authors from it rather than running `profile` per FID.

```json
{"version":1,"fids":[2,3],"casts":[{"hash":"0x0256ae66308d546514fa666cda44052abbfea066","fid":3,"timestamp":"2026-10-17T02:30:02Z","text":"","display_text":"","mentions":[],"embeds":[{"url":"https://imagedelivery.net/…/original"}],"parent":null,"signer":"0xc887f5bf385a4718eaee166481f1832198938cf33e98a82dc81a0b4b81ffe33d","app_fid":9152}],"accounts":[{"fid":3,"username":"dwr","display_name":"Dan Romero","pfp_url":"https://…/original"}]}
```

Per cast:

- `hash` and `fid` identify it. `timestamp` is UTC, ISO 8601.
- `text` may be empty when the cast is only an embed.
- `text` is as posted, without the names of the accounts it mentions.
  `display_text` has each one written in as `@username`, or `@fid:N` for an
  account with no username; quote this one. It is `null` in the rare case a
  mention's position falls inside a character; then use `text` and say the
  names are missing.
- `mentions[]` are `{fid, username, position}`: `username` is `null` when the
  account has none, and `position` is a byte offset into `text`. Example:
  `{"text":" can you post the four URLs you intended to reference?","display_text":"@balajis.eth can you post the four URLs you intended to reference?","mentions":[{"fid":37,"username":"balajis.eth","position":0}]}`.
- `embeds[]` are `{"url": …}` or `{"cast": {fid, hash}}`.
- `parent` is `null` for a top-level cast, `{"cast": {fid, hash}}` for a
  reply, or `{"url": …}` for a cast in a channel.
- `app_fid` is the app whose key signed the cast (9152 is the Farcaster app,
  username `warpcast`), or `null` when unknown.

`--limit` (default 20, at most 1000) caps the result, and is also how many
of each account's newest messages are read, so a removed cast counts
against it: `casts 3 --limit 200` returned 198. There is no paging past
that. For a period, ask for enough and filter on `timestamp` yourself.
`--include-app` and `--exclude-app` take app FIDs, and filter after the
limit, so they can return fewer.

An FID with no account gives `"casts":[]` and exit 0, the same as an
account that never cast. Check with `profile` before saying an account is
quiet.

### `selfcast feed <account> --json`

The same as `casts`, over every account `<account>` follows: what that account
would see. The shape is `{"version":1,"fid":…,"casts":[…],"accounts":[…]}`,
with casts and accounts as above. It reads each followed account, eight at a time, so an account
following thousands is slow. For a short named list, use `casts`.

### `selfcast channel <URL> --json`

The newest casts posted in a channel, newest first. A channel is the URL
its casts name as `parent` (`{"url": …}`), so take it from a cast posted
there; the CLI cannot look a channel up by its name. Many are
`https://warpcast.com/~/channel/<name>`, not all: a URL nobody posted to
gives `"casts":[]` and exit 0, so do not tell the user a channel is quiet
on a guessed URL. The shape is
`{"version":1,"channel":"https://…","casts":[…],"accounts":[…]}`, with casts
and accounts as above.
Replies in the channel are not included (their parent is a cast): use
`replies` or `thread` on a cast. `--limit` (default 20, at most 1000) and
the app filters work as for `casts`.

### `selfcast events --json`

What the node's shards merged, pruned and revoked, block by block: the
network's recent activity, for counting, and the only way to find casts by
what they say. One JSON object per line, not one document; use `jq -s` to
count. A message event:

```json
{"version":1,"shard":2,"id":806368559105,"block":49216831,"block_time":"2026-10-23T03:00:03Z","event":"merge","type":"cast_add","fid":3356482,"hash":"0xe89cce326e9ceaed7cd01bd321e13cd3c87388fd","timestamp":"2026-10-23T03:00:03Z","signer":"0x33a5…a43e"}
```

`event` is `merge` (stored), `prune` (dropped: the account was over its
storage), `revoke` (dropped: a username that moved to another account,
or a primary address no longer verified; removing an app key revokes
nothing), `username_proof` (`type` `fname`, `ens_l1`, …, with `fid` and
`name`), `onchain` (`type`
`id_register`, `signer`, `storage_rent`, …, with `fid`) or `other` (`type`
is the node's name for it). Message `type`s include `cast_add`,
`cast_remove`, `reaction_add`, `link_add` (a follow), `link_remove`,
`user_data_add`, `verification_add_eth_address`. `block_time` is when the
shard merged it; `timestamp` is what the message claims. `--since` and
`--until` take UTC, `2026-10-22` or `2026-10-22T06:00:00Z`, and select by
`block_time`.

A `cast_add` also carries the cast's `text`, `mentions` (`{fid, position}`),
`embeds` and `parent`, as in `casts` but with no usernames: `text` is
without the mentioned names, and there is no `display_text`. Name the
casts you keep with `profile`, one FID at a time (`casts` with several
FIDs and `--limit 1` returns one cast in all, not one per account), or show
them with `thread`. A cast found here may have been deleted since: the
node then no longer holds it, and `viewer open` refuses a list that
includes it, naming the cast. Drop it and open the rest.

The node keeps about three days of events, so this answers "this week"
only for those days: say which days the count covers (the first line's
`block_time`). One day is about 55,000 lines, read in a few seconds; three
days, about 176,000 lines and 54 MB. Count a day, for example:

```
selfcast events --since 2026-10-22 --until 2026-10-23 --json | jq -s -c 'map(select(.event == "merge" and .type == "cast_add")) | {casts: length, accounts: (map(.fid) | unique | length)}'
```

gave `{"casts":9421,"accounts":2427}`. Events carry no `app_fid`: the
CLI does not say which app sent an event's message.

To find casts on a subject, filter the `cast_add` merges on their text in
the same pipe. Three days took under five seconds:

```
selfcast events --since 2026-10-20T11:04:33Z --json | jq -c 'select(.event == "merge" and .type == "cast_add" and (.text | test("singapor"; "i"))) | {fid, hash, text}'
```

gave 1,444 lines, most of them one templated vote post: group by text
before reading them. It matches words, not meaning, and misses a cast that
names the subject only in an image or a link. Do not fetch casts one by one
from the node's HTTP API instead: it drops requests under load, and the
text is already here.

### `selfcast replies <FID:HASH> --json`

The direct replies to one cast, oldest first. The shape is
`{"version":1,"parent":{"fid":…,"hash":"0x…"},"casts":[…],"accounts":[…]}`,
with casts and accounts as above. Example, the replies from one app (here the test account's own key):

```json
{"version":1,"parent":{"fid":37,"hash":"0xcd34bb7404d899df69fda5e8ca17d3f1aa1e9268"},"casts":[{"hash":"0xcaec87736a1eee7fcd0351cf0b1e9c4e4defa505","fid":3356360,"timestamp":"2026-10-22T06:51:30Z","text":"Two of these claims can be measured from a read node. …","display_text":"Two of these claims can be measured from a read node. …","mentions":[],"embeds":[],"parent":{"cast":{"fid":37,"hash":"0xcd34bb7404d899df69fda5e8ca17d3f1aa1e9268"}},"signer":"0xffac…212e","app_fid":3356360}]}
```

(Text and signer shortened here.) Every reply is read before `--limit`
(default 100, at most 1000) keeps the newest, so `--include-app` and
`--exclude-app` do not cut the list short. Replies to replies are not
included: run `replies` again on the reply. A cast with no replies, and a
cast the node does not hold, both give `"casts":[]`.

### `selfcast thread <FID:HASH> --json`

A cast in its conversation: the casts it replies to, root first, then the
cast, then its direct replies, oldest first. The shape is
`{"version":1,"top":…,"ancestors":[…],"cast":{…},"replies":[…],"accounts":[…]}`,
with casts as above and `accounts` as
`{"fid","username","display_name","pfp_url"}` for every author and mention
(`null` where not set). `top` says where `ancestors` stops: `"root"` at a
cast that replies to no cast, `{"missing":{"fid","hash"}}` at a parent this
node does not hold, `{"cut":{"fid","hash"}}` after 50 casts, naming the next
one up (run `thread` on it for more). `--limit` caps the replies as in
`replies`. A cast the node does not hold exits 1.

### `selfcast reactions <FID:HASH> --json`

Who liked and who recast one cast, each oldest first:

```json
{"version":1,"cast":{"fid":3356360,"hash":"0x3820fb679133cd50df09d95beb8a5fbe2553069d"},"likes":[{"fid":1926565,"timestamp":"2026-10-21T14:13:32Z"}],"recasts":[],"accounts":[{"fid":1926565,"username":"lyraseven","display_name":"Lyra-Seven","pfp_url":"https://…/face.jpg?v=4"}]}
```

One entry per account in each list, as the node holds them now: an undone
like is not there, so these are current counts, not a history. Counts are
the lengths of `likes` and `recasts`. `accounts` is as in `thread`, for
everyone who reacted. Every reaction is read, so a busy cast is a long
answer: FID 37's `0x50184b62…` has 290 likes and 65 recasts, read in well
under a second from a ready node. For "most liked" among several casts, run
it once per cast. A cast the node does not hold exits 1.

### What reads cannot do yet

- No reactions in `casts`, `feed`, `replies` or `thread`: run `reactions`
  per cast. No reactions to a channel URL.
- No search command: `events` finds casts by their text, for the three
  days or so it covers. No channel by name, members or followers:
  `channel` takes the URL.
- Nothing older than an account's newest 1000 messages, and `events` only
  for the last three days or so.

### Examples

- [Account stats](examples/account-stats.md): one account's week, from its
  own casts and their likes and recasts.
- [A feed of a list of accounts](examples/community-feed.md).

## Viewer

A read-only page that shows a thread the way a Farcaster client does, beside
the user: in their browser, or in Shala if they use it. The page reads the
node itself, so the user can follow replies and go back without you. Use it
when the user wants to look at a conversation with you, not to read one
yourself: for that, `thread --json` is the read.

**While a page is open, casts go on the page, not in your reply.** `viewer
state` succeeds when one is; then "show me" means `viewer open`. Don't
reprint the casts you put up: the user is reading them there. Your reply
says what the page now shows and why you chose it, and carries what only
you can add: the summary, the ranking's reasons, the answer to the
question. Name a cast by its author and a few of its words, so the user
can find it on the page. Print casts in the reply only when no page is
open, or when the user asks for the text.

```
selfcast viewer serve [--images]   # host the page on 127.0.0.1 until stopped; prints its address
selfcast viewer open <FID:HASH>    # show that cast's thread; prints what it opened
selfcast viewer open <MONIKER>     # show what a selfcast moniker names (below)
selfcast viewer state              # what the page shows: thread or list, marked casts, Back trail
```

A moniker names one view of the page, as one token. Each `<cast>` is a
`FID:HASH`, and a list is casts separated by commas, at most 50:

| Moniker | Shows |
|---|---|
| `selfcast://selfcast/thread/<cast>` | the cast's thread, as `open <cast>` does |
| `selfcast://selfcast/thread/<cast>?only=<reply>,…` | the cast and the casts above it, and only those of its direct replies, in the order given; the rest fold into a "Show all" button |
| `selfcast://selfcast/casts/<cast>,…` | those casts, from anywhere, in the order given; each opens its thread when clicked |

Any of them may end `#cast=<cast>,…`, which marks those casts and scrolls
to the first. Use `?only=` to show the user the replies an answer is
about, ranked as you ranked them, and `casts/` for casts from different
threads. The page refuses a moniker it cannot show in full (a reply that
is not one, a cast the node does not hold, a marked cast not on the page)
and changes nothing. Quote the moniker in single quotes: `?` and `#` mean
something to the shell.

- **`serve` keeps running.** Run it in the background, or ask the user to
  run it in a terminal, and give them the address it prints to open. It
  reads from `--node`, and the page can reach only that node. A selfcast
  built without the `viewer` feature says so and serves nothing.
- `open` and `state` go to the page `serve` hosts when it is running, and
  otherwise to Shala. Exit 1, with the page's or the host's reason, when it
  could not show the cast or no page is open.
- **What the user points at is `user_selected` in `viewer state`**: the
  cast they last clicked or opened, as a moniker: the view they were on,
  with `#cast=<FID:HASH>` for a cast selected in it. What `viewer open`
  shows never changes it, so the rest of the state says what is on screen
  (`view` is `thread`, with `cast` and `only` when filtered, or `casts`,
  with `casts`; `selected` lists the marked casts or is `null`; `back` holds
  what Back returns to, each as `open` takes it), and `user_selected` what
  the user chose. In Shala, when `shala selection` says the user was last in
  selfcast's app, this is where to look.
- `viewer state` cannot tell whether the user is looking, or who chose what
  it shows.
- **Profile pictures only with `serve --images`.** Each then loads from
  the server its account chose, which learns the user's address and when
  they looked. Off by default: turn it on only when the user asks. Without
  it, or where a picture fails, the page shows initials. Shala's page
  policy refuses remote images, so in Shala there are none.
- Each card shows its likes and recasts once the page has read them,
  current counts as `reactions` gives them.
- The page shows no images in casts: an image is a link labelled
  `image`. In Shala it opens no links: a link shows its address.

## Writes

**Run once, on Farcaster's public node.** On 2026-10-21 a test account
sent a cast, a like, an unlike, a delete, a follow and an unfollow through
`crackle`, each run by an agent as described below, and each read back from
`crackle` afterwards. One cast has since gone through the user's own node
while it was hours behind on the account's shard: it reached the network,
but the node did not show it, so it stayed in the outbox.
`resend --node https://crackle.farcaster.xyz:3381` then cleared it: same
message, no second cast.

### Read-only until the user enables writes

Do not run any write command until the user has said, in this
conversation, that you may write. Being asked for one write is not
permission for the next. "Enable writes" lasts for this conversation only.

The write commands are `cast`, `delete`, `react`, `unreact`, `follow`,
`unfollow` and `resend`. Each is its own subcommand, so the user's harness
can gate each one separately (*Gating writes in the harness*, below).

### The app key: `selfcast key add` is the user's, not yours

A write is signed with an **app key**, which can only post as the account:
casts, reactions and follows, each with its undo. It cannot move funds,
change the account's profile, or change who owns the account. The key that
owns the account comes from the **recovery phrase** (seed phrase), and it
can do all of those.

- **Never run `selfcast key add`.** Tell the user to run it in their own
  terminal. It reads the recovery phrase from the keyboard only, never from
  a pipe, a file, an argument or the environment.
- **Never ask for a recovery phrase, and never accept one.** If the user
  pastes one into the conversation, do not repeat it, store it, or pass it
  to any command. Tell them it is now in this transcript, that they should
  treat that account as exposed, and that they should only use a test
  account with Selfcast.
- Do not open, read, copy or list the key files under
  `~/.config/selfcast/keys` (or `$XDG_CONFIG_HOME/selfcast/keys`).

What `key add` sets up, as built:

- The key may sign casts, cast deletes, reactions and their undo, and
  follows and their undo. Nothing else.
- It expires 90 days after it was last used, and each use pushes that back.
- It is registered with a gasless `KEY_ADD` message, not a chain
  transaction.
- The key is saved at mode 0600, and a key file with wider permissions is
  refused. The recovery phrase is never written anywhere.
- It works on mainnet only.

**Removing a key keeps what it signed.** The node does not delete the
casts, reactions or follows an app key made when the key is removed
(from Snapchain 0.14.3's source; not yet seen on a node). Delete those
first if that is the goal. Selfcast has no `key remove`
command yet.

### What each write does

All take `--json`, `--dry-run` and `--yes`, and `--fid <fid>` when keys
for several accounts are saved. `cast` also takes `--reply-to` and
`--embed`.

| Command | Does |
|---|---|
| `cast <text>` | Posts `<text>` exactly as given. `--reply-to <FID:HASH>` makes it a reply. `--embed <URL or FID:HASH>` adds a link or quotes a cast (repeatable). `@name` is **not** turned into a mention. Up to 1024 bytes of UTF-8 (not characters); over 320 it goes as a long cast, and past 1024 it is refused before anything is signed. |
| `delete <hash>` | Deletes one of the account's own casts. |
| `react like\|recast <FID:HASH>` | Likes or recasts a cast. |
| `unreact like\|recast <FID:HASH>` | Undoes a like or a recast. |
| `follow <account>`, `unfollow <account>` | Follows or unfollows an account, by FID or username. The confirmation names the FID a username resolved to. |
| `resend` | Sends again any message an earlier write saved but did not see arrive. It does not ask: each one was approved when first written. |

A write counts as done only once the node is seen to hold it. Before it is
sent, the signed message is saved to an outbox (`~/.config/selfcast/outbox`).

`--json` output, version 1:

```json
{ "version": 1, "status": "sent", "kind": "cast", "fid": 1234, "hash": "0x…" }
```

`status` is `sent`, or with `--dry-run` `valid` or `invalid` (nothing
sent). `kind` is one of `cast`, `delete`, `like`, `recast`, `unlike`,
`unrecast`, `follow`, `unfollow`. `resend --json` prints
`{"version":1,"messages":[…]}`, one entry per message sent. This shape is
from the source; no write has printed it against a node yet.

Errors you will see, and what to do:

- `no app key saved; run … key add first`: the user runs `key add`.
- `… is not registered to FID … on this node`: the user's `key add` did
  not finish, or `--node` points at a different node.
- `… message(s) from an earlier write are saved but not seen sent; run …
  resend first`: new writes are refused until `resend` has run. Ask the
  user before running it, and say what it will send (it prints each one).
  When it ends `resend once the node shows them`, a next line `Shard …,
  which holds FID …'s messages, is … behind the network` says `resend`
  cannot succeed until that shard catches up (`node status`). The time is
  the shard's lag, not how long catching up takes. Tell the user, and
  offer to wait, or to run `resend --node
  https://crackle.farcaster.xyz:3381`, which sends the same message to
  Farcaster's public node and checks there. Run that only if the user
  chooses it.
- `sent, but the node did not show it within a minute …`: run `resend`
  later to check and send it again. Do not write the same thing again:
  that would be a second message. The same `Shard … is … behind the
  network` line may follow, with the same meaning.
- `the node says it would not accept this message; nothing was sent`.
- `this needs a terminal …`: you ran a write without `--yes` or
  `--dry-run`. Follow the steps below instead.

### Confirming a write: show the CLI's text, not yours

What the user approves must be what the CLI is about to sign, rendered by
the CLI from the signed message itself. Do not paraphrase it, shorten it,
or translate it.

For every write:

1. Run it with `--dry-run --json`, and the subcommand first:
   `selfcast cast "gm" --dry-run --json`. This signs the message, prints
   what it does to stderr, and asks the node whether it would accept it.
   Nothing is saved or sent.
2. Show the user the stderr text **exactly as printed**, in a code block.
   It starts `Send as @<username> (FID <fid>), signed by app key <key>,
   dated <time>:`, and then gives the cast text (each line after `| `,
   with control characters escaped) or the one action, for example `Like
   cast <FID:HASH>`. For a follow or a reaction, a last line names the
   other account as the node knows it: `FID 37 is @balajis.eth on this
   node.`
3. Ask the user to confirm. Stop if they don't.
4. Run the same command with `--yes` in place of `--dry-run`. With
   `--yes` the CLI prints the text again to stderr, from the message it
   sends, and sends without asking at the terminal. Show that text exactly
   as printed, then its JSON result, so the user can check it matches what
   they approved.

The user may say they don't need steps 1 to 3 for the rest of this
conversation. Then run the write with `--yes` directly and show the text it
prints. Never drop the harness's prompt yourself (*below*).

**Limits:**

- The CLI's own confirmation is a question it asks at the terminal. An
  agent's shell usually has none, so `--yes` skips it. What the user then
  approves is your relay of the CLI's text and the harness's prompt, which
  shows the command line, not the CLI's rendering. You could show one
  thing and run another. The harness's prompt is the check on that, so the
  user should read the command in it.
- Step 4 signs again, with a new timestamp, so its hash differs from the
  dry run's. The content is the same only if the command is.
- Where the harness lets you answer its prompts yourself, or runs commands
  without asking, nothing between you and the node is the user's.

### Gating writes in the harness

Each write is its own subcommand so that a harness can gate it apart from
the reads. Recommend that the user's harness let reads run, ask before each
write, and refuse `selfcast key` to the agent. Do not change any permission
setting yourself. How to express this, and whether it holds in every mode,
is the harness's own; check its documentation.

Tried once in Claude Code, on 2026-10-21, with these rules in
`.claude/settings.local.json`:

```json
{
  "permissions": {
    "ask": ["Bash(*selfcast cast *)", "Bash(*selfcast delete *)", "Bash(*selfcast react *)",
            "Bash(*selfcast unreact *)", "Bash(*selfcast follow *)", "Bash(*selfcast unfollow *)",
            "Bash(*selfcast resend*)"],
    "deny": ["Bash(*selfcast key *)"]
  }
}
```

- The `deny` rule held: `target/release/selfcast key --help` was refused.
- The `ask` rules did not prompt. With `--node` between `selfcast` and
  `cast` the pattern cannot match, so always write `selfcast`, then the
  subcommand, then its arguments and flags. The session was also in auto
  mode, which ran the write without a prompt. Whether the `ask` rules
  prompt outside auto mode was not tried.

**Limits:** these rules match the command's text, and a different spelling
(options first, another path, `sh -c '…'`) or a mode that skips prompts
gets past them. They are a convenience, not a boundary. On the user's own
machine an agent with a shell can send anything the app key allows. What
holds is the key's scope: casts, deletes, reactions and follows, never the
account, its funds or its keys; and `selfcast key add` reads the phrase
only from a terminal, which an agent's shell does not have.
