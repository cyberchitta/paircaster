---
title: Giving your AI agent Farcaster
---

The repository's `skill/` directory is an agent skill: instructions an AI agent loads before it runs `selfcast`. It calls the same commands as you do, with `--json`.

In Claude Code a skill is a directory under `~/.claude/skills/`: copy `skill/` there as `~/.claude/skills/selfcast`. The writes in [Using the client](client.md#writing:-casts,-reactions,-follows) were sent by an agent following the skill's write steps.

What the skill tells the agent:

- **Check the node first.** Run `selfcast node status` before reading, and say how far behind the node is in every answer.
- **Reads only, until you say otherwise.** It runs no write until you enable writes in that conversation, and that lasts for the conversation only.
- **You approve Selfcast's text, not the agent's.** For each write it runs the command with `--dry-run`, shows you the text `selfcast` printed, exactly as printed, and waits for your yes before running it again with `--yes`. `selfcast` writes that text from the message it signs, so within one run it cannot show one thing and send another. The two runs are separate commands, though: the `--yes` run prints the text again from what it actually sent, and the skill shows you that too, so check that it matches what you approved.
- **Never a recovery phrase.** It never runs `key add`, never asks for a phrase, and refuses one if it is pasted in: you run `key add` in your own terminal.

Each write is its own command so that your agent's harness can gate it apart from the reads: let reads run, ask you before each write, and refuse `selfcast key` to the agent. How to say that is up to the harness. In Claude Code, for example:

```file .claude/settings.local.json
{
  "permissions": {
    "ask": ["Bash(*selfcast cast *)", "Bash(*selfcast delete *)", "Bash(*selfcast react *)",
            "Bash(*selfcast unreact *)", "Bash(*selfcast follow *)", "Bash(*selfcast unfollow *)",
            "Bash(*selfcast resend*)"],
    "deny": ["Bash(*selfcast key *)"]
  }
}
```

Tried in Claude Code's auto mode: the `deny` rule held, refusing the agent's `selfcast key --help`, and also a command that only contained the words, in text the agent was writing into this site. The `ask` rules did not prompt: with `--node` between `selfcast` and `cast` the pattern cannot match, and auto mode ran the writes without asking.

<limit>

Rules like these match the command's text, so another spelling (options first, another path, `sh -c`) or a mode that skips prompts gets past them, and harmless text can trip them. They are a convenience, not a boundary. On your machine an agent with a shell can send anything the app key allows. What holds is the key's scope: casts, deletes, reactions and follows, never the account, its funds or its keys; and `key add` reads the phrase only from a terminal, which an agent's shell does not have.

</limit>

What the agent cannot read yet: the reactions or replies to a cast (so no “most liked”), search results, channels, usernames (accounts are named by FID), and anything older than an account's newest 1000 messages.

---

Next: [Safety](safety.md)
