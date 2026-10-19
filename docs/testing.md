# Testing Selfcast on a clean machine

This is for someone who will follow the Selfcast tutorial from start to finish
and report what happened. It is not a request for a good review. A run that
fails is a useful result if your report says where it failed and what you
saw.

## Before you start

- **A clean machine** (macOS or Linux) that has not had Selfcast or Snapchain
  on it. A new cloud server is fine.
- **Disk and time.** A mainnet node needs 16 GB of RAM, 4 vCPUs and well over
  1 TB of disk. The snapshot is about 400 GB; on a 100 Mbps line Snapchain
  estimated 14 to 20 hours for the download alone. Plan for a day before the
  node is ready.
- **A test account.** Use a Farcaster account made for testing, holding
  nothing of value. Never use your main account.
- **Your seed phrase stays with you.** Selfcast is built to read it only
  from your terminal, use it once to add an app key, and never save it. If it
  ever asks for the phrase anywhere else, stop and report that. Never paste
  it into a chat, an issue, a screenshot or a report, and never send it to
  us. Nobody working on Selfcast will ask for it.

## The run

Follow the site, starting from the quickstart, and use only what it says. If
you have to do something it does not mention, write it down: that is a bug in
the tutorial.

Check each of these and note the result:

1. The three quickstart commands run as shown, and their output looks like
   the output on the site.
2. The node downloads its snapshot, starts, and reports that it is in sync.
   Stopping and restarting the download resumes it instead of starting over.
3. `selfcast` reads your profile and your feed from your own node.
4. Importing your test account adds an app key. Before anything is signed,
   the terminal shows what you are approving.
5. A cast you post shows up when you read it back from your node, and in
   another Farcaster client.
6. Reply, follow and unfollow, like and unlike, recast and undo the recast.
7. Filtering the feed to include or exclude casts from a given app.
8. The site reads well on a phone as well as a desktop.

## Running it with an AI agent

You can also run the whole thing through an AI agent with the Selfcast
skill. The rules for the agent are tighter:

- You type the seed phrase yourself, into the terminal Selfcast asks in.
  The agent never sees it.
- Writes stay off until you turn them on. Each write is a separate command,
  so you can let your agent tool allow reads and ask you before every write.
- Keep the transcript. Record the agent, the model, and every time you had
  to step in.

## Report

Post your report publicly on Farcaster, then send us the link. Fill in what
applies:

```text
Tester (Farcaster account):
Run: by hand / with an agent (which agent, which model)
Selfcast version:
Machine, OS, CPU architecture:
Started (UTC):           Node in sync (UTC):         Finished (UTC):
Cloud cost, if any:
Steps 1-8 above: pass / fail / not tried, each
Message hashes of the casts and other writes you made:
Every failure, retry, workaround, and time you had to step in:
One thing that worked well:
The worst problem:
```

Screenshots are welcome. Check that none shows a seed phrase, a key, a token
or your full config before you post it.
