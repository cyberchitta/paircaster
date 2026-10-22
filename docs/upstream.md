---
title: Contributions upstream
---

Building Selfcast meant checking Snapchain's documentation and code against a running node. Where they disagreed, we filed it with Snapchain and the Farcaster docs.

## Pull requests

- [snapchain #1062](https://github.com/farcasterxyz/snapchain/pull/1062): the Rust example in the message reference signs the message data, but the node checks the signature over the message hash, so a message signed as shown is rejected. Also, `validateMessage` does not check that the FID is registered or that the key belongs to it, as the page says it does.
- [snapchain #1063](https://github.com/farcasterxyz/snapchain/pull/1063): corrects the HTTP API reference against a v0.14.3 node: paging takes `reverse=true`, not `1`; the shape of errors; the `user_data_type` filter that `userDataByFid` ignores; and which fields are hex and which base64.
- [snapchain #1072](https://github.com/farcasterxyz/snapchain/pull/1072): a long cast over 10,000 bytes was told it was too short, and one too short was told it was too long. The two errors are swapped back, and the test now checks which error comes back.
- [docs #471](https://github.com/farcasterxyz/docs/pull/471): removing an app key no longer deletes the messages it signed, as the key registry reference says it does.

## Issue

- [snapchain #1064](https://github.com/farcasterxyz/snapchain/issues/1064): the snapshot download starts again from the first chunk after any interruption.

## Comments

- [On #720](https://github.com/farcasterxyz/snapchain/pull/720#issuecomment-6295519511): when one shard runs out of results, its paging cursor restarts it from the beginning, so paging through `linksByTargetFid` never ends.
- [On #1050](https://github.com/farcasterxyz/snapchain/pull/1050#issuecomment-6297317196): a snapshot restore that dies partway through unpacking a shard leaves it half-written, which the fix does not catch.
- [On #1049](https://github.com/farcasterxyz/snapchain/pull/1049#issuecomment-6299312324): the new signers reference page, checked against a node: one example needs quotes, one parameter is written in a form the node rejects, and gasless keys are missing from the list of sources.
- [On #550](https://github.com/farcasterxyz/snapchain/pull/550#issuecomment-6292240396): the upstream bugs that held up Snapchain's upgrade of its consensus library look fixed.
- [On #513](https://github.com/farcasterxyz/snapchain/issues/513#issuecomment-6297063207): a smaller step towards faster sync that needs no library upgrade: keep several block requests in flight at once.

---

Back to the [guide](index.md).
