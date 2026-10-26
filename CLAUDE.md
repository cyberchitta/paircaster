# Paircaster: working context

Paircaster is a Farcaster read node plus an agent-ready client, run on your own
infrastructure with no API keys from anyone: a provision script, a wrapper
around the Snapchain bootstrap, a Rust library and CLI over the node's HTTP
API, an agent skill that calls the CLI, and a microsite that teaches it.

The project publishes code and a static site only. It runs no service, holds
no one's keys, and has no default endpoint pointing at a server it runs or at
any third party's API. The default node is `http://localhost:3381`.

**To use Paircaster rather than change it** (check the node, read casts, show
a thread in the viewer), read `skill/SKILL.md` before the first `paircaster`
command. Claude Code lists that skill only when started in this directory;
started from its parent, it never sees it.

## Hard constraints

- **No API keys, no hosted APIs in the stack.** Neynar's docs may be read;
  its API is never called, not even in tests.
- **The agent never sees key material.** A seed phrase is read only from an
  interactive terminal. It is never read from argv, environment, stdin when
  stdin is not a TTY, or a file, so that it cannot land in shell history, a
  process list, or an agent transcript. It is used once to authorise an app
  key, then zeroed, and never written to disk.
- **Test accounts only** in development, CI and every example on the site.
- **The write confirmation is never the AI's.** What the user approves is
  rendered by the CLI from the message it is about to sign, not paraphrased
  by the agent. Where the harness lets the agent answer a prompt itself, say
  so as a limit and lean on the harness's own permission rules instead (each
  write is its own subcommand so it can be gated separately from reads).
- **Every command on the site works with the current code.** Each was run, and
  its output is real. Where a command changed after its output was captured,
  the site shows the current command and its opening note says so. A name or
  path the program prints may be updated by hand where the source shows the
  current string (as after the Selfcast rename); what the network holds and
  what an agent said stay as captured. What has not been run is listed once,
  in the site's "Not tried yet" list, not marked inline. The site is a guide,
  not a log: no dates of runs and no account of how a check was done; that
  record is kept out of the repo. A statement about what Paircaster does with
  no check behind it is written as a limit, not a property.
- No telemetry or phoning home. Farcaster's public nodes (`crackle` mainnet,
  `iris` testnet) may be used during development, never as a default.
- Do not copy code from projects with proprietary or unclear licences. The
  Farcaster repos are MIT; cite file and commit when porting behaviour.

## Keys, signing and the chain

The parts of this repo that touch keys or the chain are small; these rules
keep them auditable by someone who does not read the crypto.

- **Two kinds of key, never confused in types.** The *custody key* (from the
  seed phrase; Ethereum secp256k1) owns the account and can do anything. An
  *app key* (signer; Ed25519) can only post messages as the account: it
  cannot move funds or change custody. Each is its own type; no function
  takes "a key".
- Secret types implement neither `Debug` nor `Display`, are zeroed on drop
  (`zeroize`), and are never logged or put in an error message.
- App keys are stored at a documented path with mode `0600`, refused at load
  if the mode is wider.
- **Sign the bytes you send.** Encode `MessageData` once, hash and sign those
  bytes, and submit the same bytes. Never re-encode between signing and
  submitting.
- Hashing, signing and EIP-712 come from established crates (`blake3`,
  `ed25519-dalek`, `alloy`). Nothing cryptographic is written here.
- Message construction has golden vectors: fixed inputs, expected bytes,
  hash and signature. The node is the authority on validity: a message type
  ships once a node's `validateMessage` accepts it. A comparison with
  `@farcaster/hub-nodejs` is a sanity check, not a gate.
- App keys are added and removed with Snapchain's gasless `KEY_ADD` /
  `KEY_REMOVE` messages. They carry scopes and a sliding expiry (at most 90
  days, refreshed by use); say so wherever keys are explained. Since
  Snapchain v0.10.0, revoking a key does **not** remove the messages it
  signed: check this against the current engine before the tutorial states
  either way.
- Snapchain's `.proto` files are GPL-3.0 and this repo is MIT. Never vendor
  them or code generated from them; hand-write the prost structs we need,
  citing the proto file and commit they mirror.
- Anything that sends a chain transaction or spends money is out of scope
  unless the brief is changed to include it.

## Code style

Code follows the global `code-style` skill, Rust also `rust-style` and the
viewer's TypeScript also `typescript-style`; load them before writing code.
What this repo adds:

- `paircaster-core` keeps its logic pure (feed and profile assembly, signer
  attribution, message construction) and confines network access to the
  `node` module, so the logic is tested without a node.
- Snapchain's HTTP responses are a third-party format that will grow: parse
  only the fields we use, and allow unknown ones (no `deny_unknown_fields`
  there). A missing field we need is still an error, never a default.
- Farcaster timestamps (seconds since 2021-01-01 UTC) are converted at the
  boundary into one type; nothing past `node` sees a raw protocol timestamp.
- `--json` output is an external format, versioned from the first release.
  The skill depends on its shapes.
- `unsafe` is forbidden.

## Checks

- `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo test --workspace` before every commit.
- For a commit touching `viewer/`, also, in `viewer/`: `bun run lint`
  (Biome), `bun run check` (`tsc` over the source, the tests and `build.ts`) and
  `bun test`.
- Discrepancies between farcasterxyz docs and observed behaviour go in
  `docs/upstream-fixes.md` as they are found. Each is a candidate PR.
