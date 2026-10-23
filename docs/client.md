---
title: Using the client
---

Every read below goes to your own node's HTTP API and nowhere else. The examples come from the test node while it was still about two days behind the network, so the newest casts in them are from mid-October 2026.

## Where to run selfcast

`selfcast node fetch-snapshot`, `pause`, `resume` and `status` are about the Snapchain installed on the machine you run them on: they read its install directory. Everything else, reads, writes and `key add`, only talks to a node's HTTP API, so it can run anywhere that reaches one. Without `--node`, that is the node on this machine, at `http://localhost:3381`.

If the node is on a server, keep its API on the server's loopback (see [Ports](node.md#ports,-and-who-can-reach-your-node)) and reach it from your own machine through SSH: `ssh -N -L 3381:localhost:3381 you@your-server` makes the server's API answer at `localhost:3381` on your machine for as long as it runs. Reads and writes then work as they do on the server, and your app key stays on your own machine; the server holds no key. To ask how that node is doing from your machine, name it: `selfcast node status --node http://localhost:3381`. With `--node`, `status` reports only the node named, with no disk; without it, `status` describes this machine's own install. The server's download and disk are reported by `selfcast node status` run on the server.

```run
ssh -N -L 3381:localhost:3381 you@your-server
selfcast node status --node http://localhost:3381
```

```output
Syncing (Snapchain 0.14.3): 8h31m behind at most.
  shard 0: height 48554265, 49m behind
  shard 1: height 49221050, 3h24m behind
  shard 2: height 49038286, 8h31m behind
```

Reads work the same way through the tunnel. If the node behind it does not answer, `node status --node` says it could not reach the node.

## Accounts and FIDs

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

The name, bio and username are the account's own *user data* messages (`/v1/userDataByFid`). The username, `dwr`, is an *fname*: one of those messages, not part of the account itself. A follow is a *link* message from one account to another: `following` counts the account's own (`/v1/linksByFid`), `followers` counts the ones aimed at it (`/v1/linksByTargetFid`). The node keeps no follower total, so `selfcast` reads followers 1000 at a time and stops at 10,000. `selfcast profile 3 --all-followers` reads every one: 630,232 for this account, in 7.6 seconds, or 36 the first time, before the node had them cached.

An FID is issued on-chain, by Farcaster's ID registry contract on OP Mainnet (chain 10), to an Ethereum address: the account's *custody address*. The key behind that address comes from the account's recovery phrase and owns the account ([Adding an app key](#adding-an-app-key)). Your node records each registration, so it can say which FID an address owns. For the test account `@selfcast`, used in the examples below:

```run
curl -s 'http://localhost:3381/v1/onChainIdRegistryEventByAddress?address=0x337596dD506856c952a89554DE28064146b901D0' | jq '{type, chainId, fid, custody: .idRegisterEventBody.to}'
```

```output
{
  "type": "EVENT_TYPE_ID_REGISTER",
  "chainId": 10,
  "fid": 3356360,
  "custody": "0x337596dd506856c952a89554de28064146b901d0"
}
```

## Casts, and what a message is

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
{"version":1,"fids":[3],"casts":[{"hash":"0x0256ae66308d546514fa666cda44052abbfea066","fid":3,"timestamp":"2026-10-17T02:30:02Z","text":"","display_text":"","mentions":[],"embeds":[{"url":"https://imagedelivery.net/BXluQx4ige9GuW0Ia56BHw/9f4bda20-a039-4770-5605-8eb24d259e00/original"}],"parent":null,"signer":"0xc887f5bf385a4718eaee166481f1832198938cf33e98a82dc81a0b4b81ffe33d","app_fid":9152}]}
```

A mention is not in a cast's `text`: the message carries the mentioned account's FID and a byte position, and a client writes the name in. `selfcast` looks the names up and writes them in for you, as `display_text` in JSON (`@fid:N` for an account with no username):

```run
selfcast replies 37:0x50184b62b4c52fd76628713562e1eda3dccaf63d --json | jq -c '.casts[] | select(.hash | startswith("0xeb68dbec")) | {text, display_text, mentions}'
```

```output
{"text":" can you post the four URLs you intended to reference?","display_text":"@balajis.eth can you post the four URLs you intended to reference?","mentions":[{"fid":37,"username":"balajis.eth","position":0}]}
```

## The feed

```run
selfcast feed 2 --limit 5
```

The same as `casts`, over every account FID 2 follows: what that account would see. It reads the follow list, then each followed account's casts, eight accounts at a time. Casts dated in the future are left out.

## Replies

```run
selfcast replies 3356360:0x3820fb679133cd50df09d95beb8a5fbe2553069d
```

```output
2026-10-21T19:04:46Z  fid 3356360  0x4853a7d5837632037621a83f1068c8b57ba5ed24  app 3356360
  How:
  selfcast node fetch-snapshot
  cd ~/snapchain && ./snapchain.sh up
  selfcast key add
  selfcast cast "…"
  
  The phrase is typed once at a terminal and never stored. This reply was sent by an AI agent through the Selfcast skill, after I approved the CLI's own confirmation text.
  
  github.com/cyberchitta/selfcast
```

The direct replies to one cast, oldest first (`/v1/castsByParent`). A reply to that reply is not listed: run `replies` on it. `--include-app` and `--exclude-app` work as for `casts`, over every reply. A busier thread, FID 37's cast `0x50184b62…`, has 48 replies, and 6 of them come from apps other than the Farcaster app:

```run
selfcast replies 37:0x50184b62b4c52fd76628713562e1eda3dccaf63d --exclude-app 9152 --json | jq -c '{replies: (.casts | length), apps: (.casts | map(.app_fid) | unique)}'
```

```output
{"replies":6,"apps":[2904,11279,16823,1351215,3356360]}
```

The node's pages of replies repeat: a shard that runs out starts again from its first reply, and the last page leads back to the first. So `selfcast` reads pages until one brings no reply it has not seen.

## Threads

```run
selfcast thread 3356360:0x3820fb679133cd50df09d95beb8a5fbe2553069d
```

```output
--- the cast ---
2026-10-21T11:32:08Z  fid 3356360  0x3820fb679133cd50df09d95beb8a5fbe2553069d  app 3356360
  This cast went out through my own Snapchain node, signed by an app key I added with a gasless KEY_ADD message. No API key, no hosted service, no one else's server.
  
  Selfcast: run your own Farcaster read node, plus a CLI and an agent skill that use it.

--- 1 replies ---
2026-10-21T19:04:46Z  fid 3356360  0x4853a7d5837632037621a83f1068c8b57ba5ed24  app 3356360
  How:
  selfcast node fetch-snapshot
  cd ~/snapchain && ./snapchain.sh up
  selfcast key add
  selfcast cast "…"
  
  The phrase is typed once at a terminal and never stored. This reply was sent by an AI agent through the Selfcast skill, after I approved the CLI's own confirmation text.
  
  github.com/cyberchitta/selfcast
```

A cast with the casts it replies to, root first, and its direct replies: the newest 100, oldest first (`--limit` changes the number). To see a thread in a page instead, with your agent, see [Looking at a thread together](agent.md#looking-at-a-thread-together).

## Reactions

```run
selfcast reactions 3356360:0x3820fb679133cd50df09d95beb8a5fbe2553069d --json | jq -c '{likes: (.likes | length), recasts: (.recasts | length)}'
```

```output
{"likes":1,"recasts":0}
```

Who liked and who recast one cast, each oldest first (`/v1/reactionsByCast`). Each list has one entry per account, `{fid, timestamp}`, as the node holds them now: an undone like is not listed. An `accounts` list gives each account's username, display name and picture, as in `thread`. Without `--json`, `selfcast` prints the two counts, then each like and recast with its time, username and FID. FID 37's cast `0x50184b62…` has 290 likes and 65 recasts, and 60 accounts did both:

```run
selfcast reactions 37:0x50184b62b4c52fd76628713562e1eda3dccaf63d --json | jq -c '[.likes[].fid] as $l | {likes: ($l | length), recasts: (.recasts | length), both: ([.recasts[].fid | select(IN($l[]))] | length)}'
```

```output
{"likes":290,"recasts":65,"both":60}
```

These pages repeat as the replies' do, and are read the same way.

## Writing: casts, reactions, follows

The examples are from a test account, `@selfcast` (FID 3356360). They went through Farcaster's public node `crackle` because the test node had not yet synced the new account: a node has to have seen an account to accept its writes.

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
      --node <NODE>           Node HTTP API base URL [default: http://localhost:3381, the node on this machine]
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

Everything above the last two lines is written from the signed message. The line naming `@farcaster` is the node's answer for FID 1, so you can see whom you are about to follow. A write sent by an agent, with `--yes` after the person approved the dry run's text:

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
2. It shows you what it is about to send, written from the signed message itself, starting `Send as @<username> (FID <fid>), signed by app key <key>, dated <time>:`, and asks you to type <kbd>yes</kbd> at the terminal.
3. It saves the signed message to an outbox, then submits it to your node (`POST /v1/submitMessage`). Your node checks it against what it holds and passes it on to the validators; it does not keep it.
4. It reads the message back from your node. Only then does it report the write as sent; every write above was read back. If it does not show up within a minute, `selfcast resend` sends the saved message again later.

Because your node checks a write against what it holds, it has to be synced far enough to know your app key to accept one.

**A node that is behind blocks writes after the first.** Snapchain keeps each account on one *shard*, and a node can be seconds behind on one shard and hours behind on another. Your node passes a write on at once, but shows it only once your account's shard has caught up. Until then the write stays in the outbox, and `selfcast` refuses every new write until `selfcast resend` sees the waiting one on your node. On a node still syncing, that can be hours. The error says why, and how far behind that shard is (not how long it will take to catch up), for example:

```output
Shard 2, which holds FID 3356360's messages, is 3h25m behind the network; the node shows them only once it catches up (see `selfcast node status`)
```

A write sent this way reaches the network even though your node does not show it yet. Do not send it again: `resend` sends the same message, and a new write would be a second one. To clear it without waiting, resend it through a node that has caught up, such as Farcaster's public one; it checks there instead:

```run
selfcast resend --node https://crackle.farcaster.xyz:3381
```

```output
Resending 0x3820fb679133cd50df09d95beb8a5fbe2553069d as FID 3356360: cast "This cast went out through my own Snapchain node, signed by an app key I added with a gasless KEY_ADD message. No API key, no hosted service, no one else's server.\n\nSelfcast: run your own Farcaster read node, plus a CLI and an agent skill that use it."
Resent 1 message(s); the node shows each.
```

A node that catches up faster would fix this; that is Snapchain's to change ([Why sync is slow](node.md#why-sync-is-slow)).

## Storage

An account needs *storage* to post. It is rented on-chain for a year at a time, and your node records each payment. The test account's, paid when it signed up in the Farcaster app:

```run
curl -s 'http://localhost:3381/v1/onChainEventsByFid?fid=3356360&event_type=EVENT_TYPE_STORAGE_RENT' | jq -c '.events[] | {chainId, units: .storageRentEventBody.units, paid: .blockTimestamp, expires: .storageRentEventBody.expiry}'
```

```output
{"chainId":10,"units":1,"paid":1792557391,"expires":1824093391}
```

`paid` and `expires` are Unix times, 365 days apart. What the account may keep, by kind of message, and how much of it is used:

```run
curl -s 'http://localhost:3381/v1/storageLimitsByFid?fid=3356360' | jq -c '.limits[] | {name, limit, used}'
```

```output
{"name":"CASTS","limit":300,"used":7}
{"name":"LINKS","limit":600,"used":2}
{"name":"REACTIONS","limit":600,"used":2}
{"name":"USER_DATA","limit":75,"used":6}
{"name":"VERIFICATIONS","limit":15,"used":2}
{"name":"USERNAME_PROOFS","limit":6,"used":0}
{"name":"STORAGE_LENDS","limit":1,"used":1}
```

Snapchain refuses messages from an account with no storage. Once an account is over a limit, the node drops that account's oldest messages of that kind to make room (Snapchain 0.14.3 source, `src/mempool/` and `src/storage/store/engine.rs`). This happens all the time: over the three days of network events a node keeps by default, ours recorded 13,465 casts and 45,049 reactions dropped this way.

## Adding an app key

Writes are signed with an *app key* (also called a signer), which `selfcast key add` creates and registers. Your account itself is owned by a different key, the *custody key*, which comes from your recovery phrase and can do anything with the account. An app key can only post messages as it.

```run
selfcast key add --help
```

```output
Register a new app key, authorised by the account's recovery phrase, which is read once from the terminal and never stored. Mainnet only

Usage: selfcast key add [OPTIONS]

Options:
      --node <NODE>  Node HTTP API base URL [default: http://localhost:3381, the node on this machine]
      --json         Print machine-readable JSON instead of human output
  -h, --help         Print help
```

The node has to have seen your account. A new account reaches a node still catching up only once it is in sync, so the test account's key was added through `crackle`, which had it within minutes (the recovery phrase prompt is left out):

```run
selfcast key add --node https://crackle.farcaster.xyz:3381
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

---

Next: [Giving your AI agent Farcaster](agent.md)
