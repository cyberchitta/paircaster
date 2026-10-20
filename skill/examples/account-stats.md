# Example: account stats

The user asks: "How was fid 3's week on Farcaster?"

Run on 2026-10-20 against a node about 46 hours behind. The outputs below
are what it printed.

## 1. Check the node

```
selfcast node status --json
```

`phase` was `syncing`, largest `seconds_behind` 164323 (about 46 h). The
answer has to say the week ends where the node's data does.

## 2. Profile

```
selfcast profile 3 --json
```

Gives `username` `dwr`, `display_name` `Dan Romero`, `following` 79,
`followers` 630231. This took about 7 seconds, because of the follower
count.

## 3. The account's recent casts, filtered to the week

Ask for more than a week is likely to hold, then filter on `timestamp`:

```
selfcast casts 3 --limit 200 --json | jq -c --arg since 2026-10-13T00:00:00Z '[.casts[] | select(.timestamp >= $since)] | {casts: length, replies: map(select(.parent.cast)) | length, top_level: map(select(.parent == null)) | length, newest: (.[0].timestamp), apps: (map(.app_fid) | group_by(.) | map({app_fid: .[0], casts: length}))}'
```

```json
{"casts":10,"replies":3,"top_level":7,"newest":"2026-10-17T02:30:02Z","apps":[{"app_fid":9152,"casts":10}]}
```

If `jq` is not installed, read the JSON yourself; the filter is only a
count by field. To find who the account replied to most, group the
replies by `parent.cast.fid` and look each FID up with `profile`.

## 4. Answer

> From your node (about 46 hours behind, so nothing after 2026-10-17 02:30
> UTC is in yet): dwr (fid 3) posted 10 casts since 2026-10-13, 7 of them
> new posts and 3 replies, all from the Farcaster app. The account has
> 630,231 followers and follows 79 accounts.
>
> I can't tell you which casts did best or who reacted most: selfcast does
> not read reactions to a cast yet.

Say what the node cannot show rather than inferring it.
