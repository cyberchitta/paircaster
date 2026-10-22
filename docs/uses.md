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
fid 3 is **dwr (Dan Romero)**, and his week was short. All 10 of his casts came between Oct 15 and Oct 17, and he has posted nothing in the five days since. This is from your own node, which is fully caught up (every shard is within 4 seconds of the network), so the gap since Oct 17 is real.

**Since 2026-10-15: 10 casts, 7 new posts and 3 replies, all from the Farcaster app.**

- **Oct 15:** "Excited for the next chapter!" with an embed, and a reply to **@rish**: "Great run!"
- **Oct 16, around 00:15–00:31 UTC:** a burst about running your own Farcaster:
  - "Looking forward to finally having a social network… Also cool that you can build your own client." (8 replies)
  - "Reminder this guide can help your AI agent get a working version of the Farcaster client running on your machine." (4 replies)
  - **Client wishlist** (16 replies, his most-replied post this week): mutuals-only notifications, no channels, no wallet, a reverse-chronological feed of followed accounts only. In a follow-up reply he added: no mini apps, no DMs, "have Claude make it super super fast."
  - **Snapchain node wishlist** (10 replies): sync only a subset of the network, and keep no more than 7 days of content. When **@jdl** replied, he answered: "I just want it for me."
- **Oct 17:** two posts that are mostly embeds, one captioned "Neat".

**Account:** he follows 79 accounts. The follower count stopped at its 10,000 cap, so it only says "at least 10,000"; a full count on 2026-10-21 found about 630k.

The Oct 16 wishlists are close to what Selfcast does. His "reverse chronological following only feed" is `selfcast feed`, and the node wishlist describes a lighter version of the node you're running.

Selfcast can't read likes or recasts yet, so I can't say which post did best by reactions. The reply counts above are the only measure of engagement it has.
```

It checked the node before answering, said the count it gave was a floor, and said what it could not see.

## A digest of a feed

```run
claude -p "Give me a short digest of what the accounts fid 3 follows posted in the last 24 hours." --allowedTools "Bash(selfcast:*)" "Bash(jq:*)"
```

Its first and last lines; between them it summed up 174 casts by topic, naming who said what:

```output
fid 3 is @dwr, who follows 79 accounts. Your node is fully synced, so this covers everything up to now (08:30 UTC). In the last 24 hours, 16 of those 79 accounts posted 174 casts, mostly replies. Here is what they said.
…
I can't rank these by likes because the CLI doesn't read reactions yet.
```

## Keep or drop one app's casts

Each cast carries the app that posted it, so a feed can leave out one app or show only another: [Filtering by client](client.md#filtering-by-client).

---

Next: [Safety](safety.md)
