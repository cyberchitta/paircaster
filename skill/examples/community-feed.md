# Example: a feed of a list of accounts

The user asks: "Make me a feed of just v and dwr."

Run on 2026-10-20 against a node about 46 hours behind. The outputs below
are what it printed.

## 1. Get the FIDs

The CLI has no username lookup. Ask the user for the FIDs, or use ones they
gave earlier. Here, v is fid 2 and dwr is fid 3. Confirm with `profile`:

```
selfcast profile 2 --json
```

`username` `v`, `display_name` `Varun Srinivasan`. Do the same for 3. A list
the user keeps (a team, a club, a conference's speakers) is just these FIDs;
remember it for the conversation and pass it each time.

## 2. Check the node

`selfcast node status --json`, as in the skill. Here it was `syncing`, with
the largest `seconds_behind` 164323.

## 3. Read the list

```
selfcast casts 2,3 --limit 5 --json | jq -c '.casts[] | {fid, timestamp, reply: (.parent != null), text}'
```

```json
{"fid":3,"timestamp":"2026-10-17T02:30:02Z","reply":false,"text":""}
{"fid":3,"timestamp":"2026-10-17T02:23:28Z","reply":false,"text":"Neat"}
{"fid":3,"timestamp":"2026-10-16T00:31:40Z","reply":true,"text":"I just want it for me."}
{"fid":3,"timestamp":"2026-10-16T00:30:24Z","reply":false,"text":"Snapchain node wishlist\n\n1. I want to sync only a subset of the network. \n\n2. I don’t want to keep more than 7 days worth of content."}
{"fid":3,"timestamp":"2026-10-16T00:24:18Z","reply":true,"text":"5. Remove mini apps\n\n6. Remove DMs\n\n7. Have Claude make it super super fast"}
```

Every cast is fid 3's. That is not a bug: fid 2's newest cast on this node
is from 2026-05-25,

```
selfcast casts 2 --limit 3 --json | jq -c '[.casts[].timestamp]'
```

```json
["2026-05-25T23:24:18Z","2026-05-11T21:52:52Z","2026-03-19T15:56:31Z"]
```

so tell the user when one account in the list has been quiet, rather than
leaving them to wonder why it is missing.

## 4. Answer

Show each cast with who posted it, when, and whether it is a reply. An
empty `text` with an `embeds` entry is an image or link post; say so rather
than showing a blank. Add where the data came from and how far behind it
is: "From your node, about 46 hours behind."

To keep only casts from one app, add `--include-app <app fid>`; to drop an
app's casts, `--exclude-app`.
