---
title: What you can do with it
---

Each example below ran against the test node, by then within a few minutes of the network. The agent answers are Claude Code's, started in the repository with the [skill](agent.md), and are shown as it wrote them. The two `claude -p` commands now allow only `paircaster`'s read commands; their answers were captured when they allowed every `paircaster` command, writes included. Paircaster was called Selfcast; output captured before the rename shows the old name.

<note>

An agent's model runs on its provider's servers, so whatever the agent reads goes there too. What no hosted Farcaster API sees is which accounts you read, and when.

</note>

## Count what you like

Your node has no API key to meter and no rate limit, so a question that pages through hundreds of thousands of messages is just a slow command. Every follower of `dwr`, counted one by one, in 26 seconds the first time (5 to 8 once the node has them cached):

```run
paircaster profile dwr --all-followers --json | jq .followers
```

```output
630231
```

## A feed of the accounts you choose

A list you keep, a team, a club, a conference's speakers, is a list of accounts. `casts` takes several:

```run
paircaster casts v,dwr --limit 4
```

```output
2026-10-17T02:30:02Z  @dwr  fid 3  0x0256ae66308d546514fa666cda44052abbfea066  app 9152

2026-10-17T02:23:28Z  @dwr  fid 3  0xec0c8a227d7f34ad3e2243ae098b46459b7d1847  app 9152
  Neat

2026-10-16T00:31:40Z  @dwr  fid 3  0x40a0d26b7ea3c4ef79b1f574ff2261a535cb84c6  app 9152
  I just want it for me.

2026-10-16T00:30:24Z  @dwr  fid 3  0x35f104e944de03c93cc4eb513e8b84cef58d9b77  app 9152
  Snapchain node wishlist
  
  1. I want to sync only a subset of the network. 
  
  2. I don’t want to keep more than 7 days worth of content.
```

All four are `dwr`'s: `v` has not cast since May. The skill's [worked example](https://github.com/cyberchitta/paircaster/blob/main/skill/examples/community-feed.md) has an agent build the same feed and say when an account in the list has gone quiet.

## Ask your agent about an account

```run
claude -p "How was dwr's week on Farcaster?" --allowedTools "Bash(paircaster info:*)" "Bash(paircaster node status:*)" "Bash(paircaster profile:*)" "Bash(paircaster casts:*)" "Bash(paircaster feed:*)" "Bash(paircaster channel:*)" "Bash(paircaster addresses:*)" "Bash(paircaster followers:*)" "Bash(paircaster following:*)" "Bash(paircaster replies:*)" "Bash(paircaster thread:*)" "Bash(paircaster reactions:*)" "Bash(paircaster events:*)" "Bash(jq:*)"
```

```output
dwr (Dan Romero, FID 3) had a short, busy burst and then nothing. Everything below comes from your own node, which is fully caught up (every shard about 1 second behind), so the quiet stretch is real and not sync lag.

**This week (2026-10-16 to 2026-10-23): 8 casts, all within about 26 hours, and none since 2026-10-17 02:30 UTC.** Six were new posts and two were replies. All came from the Farcaster app, and none were posted in a channel.

The main theme was building your own Farcaster setup:
- **"Snapchain node wishlist":** sync only part of the network, and keep at most 7 days of content. 102 likes, 5 recasts, 10 replies.
- **"Client wishlist":** notifications from mutuals only, no channels, no wallet, a reverse-chronological feed of only the people you follow. He replied to himself to add: no mini apps, no DMs, "Have Claude make it super super fast." 60 likes and 16 replies on the first post.
- He replied to @jdl: "I just want it for me."
- He quoted his own earlier post about a guide that helps "your AI agent get a working version of the Farcaster client running on your machine." 58 likes.
- He quoted @balajis.eth: "Looking forward to @balajis.eth finally having a social network… Also cool that you can build your own client." 109 likes, 12 recasts.

**His best-received cast** was the last one, an image with no text, on 2026-10-17: 222 likes, 27 recasts, 24 replies. "Neat", quoting a Balaji cast, got 77 likes.

**Account:** follows 79 accounts and has at least 10,000 followers. I stopped at the default count limit; a run earlier this week counted about 630,000.

Before the 16th he had been quiet: two casts on 2026-10-15, and before that nothing since 2026-10-01.

These are the like and recast counts the node holds right now, not how they built up over time. The node can't tell what was in the image post.

His node and client wishlists are close to what Selfcast does: your own read node and client, no hosted API.
```

It checked the node before answering, said the follower count it gave was a floor, and said the likes and recasts are today's totals, not the week's.

## A digest of a feed

```run
claude -p "Give me a short digest of what the accounts dwr follows posted in the last 24 hours." --allowedTools "Bash(paircaster info:*)" "Bash(paircaster node status:*)" "Bash(paircaster profile:*)" "Bash(paircaster casts:*)" "Bash(paircaster feed:*)" "Bash(paircaster channel:*)" "Bash(paircaster addresses:*)" "Bash(paircaster followers:*)" "Bash(paircaster following:*)" "Bash(paircaster replies:*)" "Bash(paircaster thread:*)" "Bash(paircaster reactions:*)" "Bash(paircaster events:*)" "Bash(jq:*)"
```

Its first and last lines; between them it summed up 140 casts by account and topic, naming who said what:

```output
Dan Romero (dwr) follows 79 accounts. Your node is fully caught up (about 1 second behind), so this covers everything they posted from 05:41 UTC on 22 Oct to 05:41 UTC on 23 Oct. Only 11 of the 79 posted in that time, 140 casts in all.
…
The other 68 accounts posted nothing in that window. This digest only covers casts and replies. The CLI doesn't attach likes or recasts to a feed, so it can't show which posts got the most attention.
```

## Keep or drop one app's casts

Each cast carries the app that posted it, so a feed can leave out one app or show only another: [Filtering by client](client.md#filtering-by-client).

---

Next: [Safety](safety.md)
