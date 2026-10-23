---
title: What you can do with it
---

Each example below ran against the test node, by then within a few minutes of the network. The agent answers are Claude Code's, started in the repository with the [skill](agent.md), and are shown as it wrote them.

<note>

An agent's model runs on its provider's servers, so whatever the agent reads goes there too. What no hosted Farcaster API sees is which accounts you read, and when.

</note>

## Count what you like

Your node has no API key to meter and no rate limit, so a question that pages through hundreds of thousands of messages is just a slow command. Every follower of FID 3, counted one by one, in 18 seconds while the node was still catching up:

```run
selfcast profile 3 --all-followers --json | jq .followers
```

```output
630232
```

## A feed of the accounts you choose

A list you keep, a team, a club, a conference's speakers, is a list of FIDs. `casts` takes several:

```run
selfcast casts 2,3 --limit 4
```

```output
2026-10-17T02:30:02Z  fid 3  0x0256ae66308d546514fa666cda44052abbfea066  app 9152

2026-10-17T02:23:28Z  fid 3  0xec0c8a227d7f34ad3e2243ae098b46459b7d1847  app 9152
  Neat

2026-10-16T00:31:40Z  fid 3  0x40a0d26b7ea3c4ef79b1f574ff2261a535cb84c6  app 9152
  I just want it for me.

2026-10-16T00:30:24Z  fid 3  0x35f104e944de03c93cc4eb513e8b84cef58d9b77  app 9152
  Snapchain node wishlist
  
  1. I want to sync only a subset of the network. 
  
  2. I don’t want to keep more than 7 days worth of content.
```

All four are FID 3's: FID 2 has not cast since May. The skill's [worked example](https://github.com/cyberchitta/selfcast/blob/main/skill/examples/community-feed.md) has an agent build the same feed and say when an account in the list has gone quiet.

## Ask your agent about an account

```run
claude -p "How was fid 3's week on Farcaster?" --allowedTools "Bash(selfcast:*)" "Bash(jq:*)"
```

```output
dwr (fid 3, Dan Romero) had a quiet week with one burst. All 8 of his casts came in about 26 hours on 16–17 October (UTC), and he hasn't cast since. Your node is fully caught up, so that gap is real.

**What he posted (16–23 Oct):** 6 new posts and 2 replies, none in a channel, all from the Farcaster app.

| When (UTC) | Cast | Likes | Recasts | Replies |
|---|---|---|---|---|
| 17 Oct 02:30 | An image with no text | 222 | 27 | 24 |
| 16 Oct 00:15 | "Looking forward to @balajis.eth finally having a social network… Also cool that you can build your own client." (quotes a balajis.eth cast) | 109 | 12 | 8 |
| 16 Oct 00:30 | "Snapchain node wishlist: 1. sync only a subset of the network 2. keep no more than 7 days of content" | 102 | 5 | 10 |
| 17 Oct 02:23 | "Neat" (quotes a balajis.eth cast) | 77 | 7 | 3 |
| 16 Oct 00:23 | "Client wishlist": mutuals-only notifications, no channels, no wallet, a newest-first feed of people you follow | 60 | 5 | 16 |
| 16 Oct 00:17 | "Reminder this guide can help your AI agent get a working version of the Farcaster client running on your machine." (quotes his own cast) | 58 | 6 | 4 |
| 16 Oct 00:24 | Reply continuing the client wishlist: "5. Remove mini apps 6. Remove DMs 7. Have Claude make it super super fast" | 19 | 0 | 2 |
| 16 Oct 00:31 | Reply to @jdl: "I just want it for me." | 4 | 0 | 3 |

**The theme:** most of the burst was about running your own Farcaster node and client. That's close to what Selfcast does.

**Account:** follows 79 accounts and has at least 10,000 followers. The quick count stops at 10,000; on 21 October the full count was 630,232.

Two things to keep in mind:
- The likes, recasts and replies are the totals now, not what each cast got during the week.
- Before this burst he posted twice on 15 October, and before that on 1 October. A short burst followed by silence looks like his usual pattern.
```

It checked the node before answering, said the follower count it gave was a floor, and said the likes and recasts are today's totals, not the week's.

## A digest of a feed

```run
claude -p "Give me a short digest of what the accounts fid 3 follows posted in the last 24 hours." --allowedTools "Bash(selfcast:*)" "Bash(jq:*)"
```

Its first and last lines; between them it summed up 140 casts by account and topic, naming who said what:

```output
Here's what the accounts fid 3 (@dwr) follows posted in the last 24 hours. It's from your own node, which is fully synced (`ready`, 0 seconds behind). The window is 2026-10-22 05:34 to 2026-10-23 05:34 UTC. In that time, 11 of the accounts @dwr follows posted 140 casts. The feed read went back to Oct 16, so nothing in the window is missing.
…
Overall the day was mostly @borodutch shipping quilcast.com features, with some background noise about moderation and bans.
```

## Keep or drop one app's casts

Each cast carries the app that posted it, so a feed can leave out one app or show only another: [Filtering by client](client.md#filtering-by-client).

---

Next: [Safety](safety.md)
