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
It was a quiet week. dwr (Dan Romero, fid 3) posted 10 casts, all between Oct 15 and Oct 17, and nothing since.

These numbers come from your own node, which is almost caught up: two shards are 1 second behind and the third is about 2 minutes behind. So the silence since Oct 17 is real, not data the node hasn't received yet.

**From Oct 15 to Oct 22 (UTC):**
- **10 casts:** 7 new posts and 3 replies. None were in a channel. All were sent from the Farcaster app (app fid 9152).
- **Most active day:** the evening of Oct 15 (just after midnight UTC on Oct 16). In about 15 minutes he posted:
  - two wishlists:
    - **Client:** notifications from mutuals only; no channels, wallet, mini apps or DMs; a reverse-chronological feed of followed accounts only; "have Claude make it super super fast".
    - **Snapchain node:** sync only part of the network, and keep no more than 7 days of content.
  - a reminder that "this guide can help your AI agent get a working version of the Farcaster client running on your machine", quoting an earlier cast of his own.
  - a quote of a cast by fid 37: "Looking forward to finally having a social network… Also cool that you can build your own client."
- **Oct 15:** "Excited for the next chapter!", quoting a cast by rish (fid 194) and mentioning fid 37, plus a reply to rish: "Great run!"
- **Oct 17:** "Neat", quoting another cast by fid 37, and one image post with no text.
- **Replies:** one each to rish (fid 194), jd 🌺 (jdl, fid 10695: "I just want it for me.") and himself, continuing his wishlist thread.

**Account:** he follows 79 accounts and has at least 10,000 followers. That number is a floor, because the quick count stops at 10,000; I can run the full count if you want the exact figure.

**What I can't tell you:** which casts did best or who interacted with him. Selfcast can't read likes, recasts or replies to a cast yet.
```

It checked the node before answering, said the count it gave was a floor, and said what it could not see.

## A digest of a feed

```run
claude -p "Give me a short digest of what the accounts fid 3 follows posted in the last 24 hours." --allowedTools "Bash(selfcast:*)" "Bash(jq:*)"
```

Its first and last lines; between them it summed up 163 casts by topic, naming who said what:

```output
Fid 3 (@dwr) follows 79 accounts, and 15 of them posted 163 casts in the last 24 hours. This is read from your own node, which is at most about 2 minutes behind the network, so the window is effectively complete.
…
Selfcast can't read likes or replies yet, so this digest can't rank which casts got the most engagement.
```

## Keep or drop one app's casts

Each cast carries the app that posted it, so a feed can leave out one app or show only another: [Filtering by client](client.md#filtering-by-client).

---

Next: [Safety](safety.md)
