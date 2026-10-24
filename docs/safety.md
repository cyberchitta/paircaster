---
title: Safety
---

Your recovery phrase controls your account and everything in it.

- **Read the code before you type a phrase into it**, as with any code from the internet: yourself, someone you trust, or an AI agent, which helps but can miss things. The phrase passes through five files: `crates/cli/src/tty.rs` reads it from the terminal, `crates/core/src/custody.rs` turns it into the custody key, `crates/core/src/key_add.rs` and `crates/cli/src/key_add.rs` sign and send the new key's authorisation, and `crates/cli/src/keystore.rs` writes the app key to disk. Network access is only in `crates/core/src/node.rs` (the node's API), `crates/cli/src/snapshot.rs` and `snapshot/` (the snapshot download), and `crates/cli/src/viewer.rs` with the `pair-ui` library it uses (the viewer's server on `127.0.0.1`). Review the commit you build (`git rev-parse --short HEAD`). This does not cover the libraries that do the cryptography (`bip39`, `bip32`, `k256`, `alloy`, `ed25519-dalek`).
- **Use a test account.** Try this guide, and any software that asks for a recovery phrase, with an account you can afford to lose. Paste a phrase only into software from developers you trust, and prefer apps that ask your wallet to approve a key over ones that ask for the phrase.
- **The phrase is read from the terminal only.** `paircaster key add` opens the terminal device itself, so the phrase cannot come from a pipe, a file, an argument or an environment variable, and does not end up in your shell history or an agent's transcript. The phrase and the custody key are held in memory that is wiped when they are dropped, and only the app key is written to disk. That is how the code is written; no test checks the memory.
- **An app key cannot take the account.** It can sign casts, deletes, reactions and follows and their undos. It cannot move funds, change the account's owner, or add other keys. A key file that others can read is refused when `paircaster` loads it.
- **Keys expire.** A key added by `paircaster` expires 90 days after it was last used; each use pushes that back (seen in [Checking a voucher](voucher.md): `expiresAt` is `lastUsedAt` plus the `ttl`).

<limit>

**Removing a key keeps what it signed.** On Snapchain since 0.10.0 (October 2025), removing an app key does not delete the casts, reactions or follows it made; delete those first if that is what you want. This is from Snapchain's source (0.14.3). Some Farcaster documentation still says the opposite. `paircaster` has no command to remove a key yet.

</limit>

---

Next: [Testing Paircaster on a clean machine](testing.md)
