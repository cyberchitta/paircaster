---
title: Testing Paircaster on a clean machine
---

<note>

The tester call closed at 2026-10-24 00:00 UTC. This page records the terms it ran under.

</note>

This is for someone who will follow the Paircaster guide from start to finish and report what happened. It is not a request for a good review. A run that fails is a useful result if your report says where it failed and what you saw.

## Before you start

- **A clean machine** (macOS or Linux) that has not had Paircaster or Snapchain on it. Paircaster has so far run only on one Apple Silicon Mac: Linux beyond building `paircaster`, and Intel Macs, are untried. A new cloud server is fine; see [Renting a server](node.md#renting-a-server-instead).
- **Docker, a Rust toolchain, `git`, `curl` and `jq`,** installed yourself; the guide does not cover installing them. The quickstart builds `paircaster` from source and runs Snapchain in Docker; `jq` is used in a few examples.
- **A GitHub account,** to open your report as an issue.
- **Memory, disk and time.** 16 GB of RAM, 4 vCPUs, and about 800 GB of free disk while the snapshot downloads and unpacks; the node then uses 557 GB and grows ([Hardware and disk](node.md#hardware-and-disk)). The download and unpack took about 12 hours on the test machine; after that the node has to catch up with the network, and we give no time for that because the one estimate we had did not hold ([Why sync is slow](node.md#why-sync-is-slow)).
- **A test account.** Make a Farcaster account for testing, holding nothing of value, in the Farcaster app. Signing up there registered our test account and gave it one unit of storage, which an account needs before it can post. Never use your main account. The app shows two phrases: use the Farcaster recovery phrase, not the wallet one. You do not need to know the account's FID: `paircaster key add` finds it from the recovery phrase and shows it.
- **Read the code first**, or have it reviewed, before you type your recovery phrase into it ([Safety](safety.md)).
- **Your recovery phrase stays with you.** Paircaster is built to read it only from your terminal, use it once to add an app key, and never save it. If it ever asks for the phrase anywhere else, stop and report that. Never paste it into a chat, an issue, a screenshot or a report, and never send it to us. Nobody working on Paircaster will ask for it.

## Start the node now; update Paircaster before Part 2

Run the quickstart as soon as you can and leave the download going. Paircaster itself is still being updated every day until the entry is submitted. Reports are due before 2026-10-24 00:00 UTC (see [Payment](#payment)). Before Part 2, run `git pull` in your clone and `cargo install --path crates/cli --locked` again, and give the commit you tested (`git rev-parse --short HEAD`) in your report.

## The run

Follow the site, starting from the [quickstart](index.md#quickstart), and use only what it says. If you have to do something it does not mention, write it down: that is a bug in the guide.

Check each of these and note the result. The run comes in two parts, so you can report the first while you wait for the second.

**Part 1: the node and reads** (a day or more, mostly waiting).

1. The quickstart's steps run as shown, and their output looks like the output on the site.
2. The node downloads its snapshot, starts, and `paircaster node status` reports it ready. Stopping and restarting the download resumes it instead of starting over.
3. `paircaster` reads a profile, an account's casts, and the feed of an account that follows others (a new test account follows no one, so its own feed is empty), from your own node.
4. Filtering the feed to include or exclude casts from a given app.
5. The site reads well on a phone as well as a desktop.

**Part 2: writes** (an hour, after `git pull` and reinstalling, as above).

6. `paircaster key add` adds an app key to your test account. Before anything is signed, the terminal shows what you are approving.
7. A cast you post shows up when you read it back, and in another Farcaster client.
8. Reply, follow and unfollow, like and unlike, recast and undo the recast.

Each write prints its hash when it is sent (`Sent and seen on the node: cast 0x…`); copy these into your report.

A node still catching up may refuse your writes: it accepts them only from accounts it has seen. If yours refuses them, add `--node https://crackle.farcaster.xyz:3381` to `key add` and each write, so they go through Farcaster's public node, as the guide's own examples did ([Writing](client.md#writing:-casts,-reactions,-follows)). Say so in your report; such writes count the same, so a report can count even if your own node is not in sync by the cutoff. If a write waits on your node instead (`not seen sent`), `paircaster resend --node https://crackle.farcaster.xyz:3381` clears it without a second message ([Writing](client.md#writing:-casts,-reactions,-follows)).

## If you vouch

Post the vouching cast with `paircaster cast`, from the test account you used, after you open your issue, and link the issue in it. It can be the reply that reports your issue (see [Report](#report)). It is signed by the app key Paircaster added for you, which anyone can check on any node ([Checking a voucher](voucher.md)). Vouch for what you saw, including what failed. We list in our entry only vouches whose report counted (see [Payment](#payment)).

## Running it with an AI agent

You can also run the whole thing through an AI agent with the Paircaster skill ([Your AI agent](agent.md)). The rules for the agent are tighter:

- You type the recovery phrase yourself, into the terminal Paircaster asks in. The agent never sees it.
- Writes stay off until you tell the agent, in that conversation, to enable them. Each write is a separate command, which your agent tool's permission rules can gate; those rules match command text, so treat them as a convenience, not a lock ([Your AI agent](agent.md)).
- Keep the transcript. Record the agent, the model, and every time you had to step in.

## Report

Open an [issue on the Paircaster repo](https://github.com/cyberchitta/paircaster/issues/new) with this form in it, filled in where it applies, and your logs below it. Then reply to [Paircaster's tester call](https://farcaster.xyz/selfcast/0x6734c1e8) on Farcaster with a cast linking the issue, from any Farcaster app, or with `paircaster cast "<text>" --reply-to 3356360:0x6734c1e81150dc1dea7f5546a61e48014a9bafb3`. If you report Part 1 while you wait for Part 2, add Part 2 to the same issue as a comment.

```text
Tester (Farcaster account):
Run: by hand / with an agent (which agent, which model)
Paircaster commit (git rev-parse --short HEAD):
Machine, OS, CPU architecture:
Started (UTC):           Node in sync (UTC):         Finished (UTC):
Cloud cost, if any:
Steps 1-8 above: pass / fail / not tried, each
Writes sent through: your own node / crackle
Message hashes of the casts and other writes you made:
Every failure, retry, workaround, and time you had to step in:
One thing that worked well:
The worst problem:
```

Screenshots are welcome. Check that none shows a recovery phrase, a key, a token or your full config before you post it.

## Payment

These terms are also in the [tester call](https://farcaster.xyz/selfcast/0x6734c1e8), and we follow them as written there.

Testers are paid only if Paircaster wins part of the bounty, and only out of what it wins. We split what we receive (the bounty pays in USDC) into equal shares: one for each tester whose report counts, four for cyberchitta. There is no cap on the number of testers. When a share is due, we ask in your issue for an address to send it to. Server costs are yours.

One share per person, however many accounts or runs. A report is tied to the GitHub account that opened its issue. If two reports look like the same person, we may ask for more before counting either, and we decide.

A report counts if its issue is opened and its reply posted before 2026-10-24 00:00 UTC, the form is filled in, and either:

- it lists the message hashes of writes you made with Paircaster, which we find on our node, each signed by an app key added the way Paircaster adds one; or
- it stops at a failure that no earlier issue reported, and says enough for us to reproduce it.

So a Part 1 report that passes counts once Part 2 is added to it, by the same time. A vouch on its own does not count. Nor does a failure an earlier issue already reported: we close that issue as a duplicate, linking the first. Issues are ordered by when they were opened. Within those conditions, a critical report counts the same as praise, and vouching changes nothing.

If you vouch, please mention in the cast that you may get a share if we win.

Next: [Checking a voucher](voucher.md)
