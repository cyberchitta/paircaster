# Example: account stats

The user asks: "How was dwr's week on Farcaster?"

Run against a node that was caught up. The outputs below are what it
printed. The user named the account, so the first read takes the name; the
reads after it use the FID that read returned.

## 1. Check the node

```
paircaster node status --json
```

`phase` was `ready`, largest `seconds_behind` 1. On a node still
`syncing`, the week ends where the node's data does: say so, with the lag.

## 2. Profile

```
paircaster profile dwr --json
```

Gives `fid` 3, `display_name` `Dan Romero`, `following` 79, `followers`
10000 with `followers_exact` `false`: at least 10,000. Use `fid` 3 from
here on.

## 3. The account's recent casts, filtered to the week

Ask for more than a week is likely to hold, then filter on `timestamp`:

```
paircaster casts 3 --limit 200 --json | jq -c --arg since 2026-10-16T00:00:00Z '[.casts[] | select(.timestamp >= $since)] | {casts: length, replies: map(select(.parent.cast)) | length, top_level: map(select(.parent == null)) | length, newest: (.[0].timestamp), apps: (map(.app_fid) | group_by(.) | map({app_fid: .[0], casts: length}))}'
```

```json
{"casts":8,"replies":2,"top_level":6,"newest":"2026-10-17T02:30:02Z","apps":[{"app_fid":9152,"casts":8}]}
```

If `jq` is not installed, read the JSON yourself; the filter is only a
count by field. To find who the account replied to most, group the
replies by `parent.cast.fid` and look each FID up with `profile`.

## 4. Which casts did best

`casts` carries no reactions, so run `reactions` on each of the week's
casts, by the `fid` and `hash` that `casts` gave:

```
paircaster reactions 3:0x0256ae66308d546514fa666cda44052abbfea066 --json | jq -c '{likes: (.likes | length), recasts: (.recasts | length)}'
```

```json
{"likes":222,"recasts":27}
```

The other seven gave 109/12, 102/5, 77/7, 60/5, 58/6, 19/0 and 4/0. These
are the totals the node holds now, not what each cast got during the week.

## 5. Answer

> From your node, which is caught up: dwr (FID 3) posted 8 casts since
> 2026-10-16, 6 new posts and 2 replies, all from the Farcaster app, the
> newest on 2026-10-17 at 02:30 UTC. The best received was that last one,
> an image with no text: 222 likes and 27 recasts. The account follows 79
> accounts and has at least 10,000 followers.

Say what the node cannot show rather than inferring it: here, what the
image shows, and how the likes built up over the week.
