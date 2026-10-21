---
title: Safety
---

A recovery phrase is like `rm -rf`: useful, and not something to be careless with.

- **Use a test account.** Try this tutorial, and any software that asks for a recovery phrase, with an account you can afford to lose. Paste a phrase only into software from developers you trust, and prefer apps that ask your wallet to approve a key over ones that ask for the phrase.
- **The phrase is read from the terminal only.** `selfcast key add` opens the terminal device itself, so the phrase cannot come from a pipe, a file, an argument or an environment variable, and does not end up in your shell history or an agent's transcript. The phrase and the custody key are held in memory that is wiped when they are dropped, and only the app key is written to disk. That is how the code is written; no test checks the memory.
- **An app key cannot take the account.** It can sign casts, deletes, reactions and follows and their undos. It cannot move funds, change the account's owner, or add other keys. A key file that others can read is refused when `selfcast` loads it.
- **Keys expire.** A key added by `selfcast` expires 90 days after it was last used; each use pushes that back (seen in [Checking a voucher](voucher.md): `expiresAt` is `lastUsedAt` plus the `ttl`).

<limit>

**Removing a key keeps what it signed.** On Snapchain since 0.10.0 (October 2025), removing an app key does not delete the casts, reactions or follows it made; delete those first if that is what you want. This is from Snapchain's source (0.14.3). Some Farcaster documentation still says the opposite. `selfcast` has no command to remove a key yet.

</limit>

---

Next: [Checking a voucher](voucher.md)
