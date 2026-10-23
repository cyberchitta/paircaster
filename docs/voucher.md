---
title: Checking a voucher
---

Paircaster was called Selfcast; output captured before the rename shows the old name.

A voucher says they ran the guide. Part of that can be checked by anyone, on any node. We ask vouchers to post the vouching cast with `paircaster cast`, so it is signed by the app key Paircaster added for them. Look the account's keys up:

```run
curl -s "http://localhost:3381/v1/signersByFid?fid=FID" | jq '.signers[] | select(.source == "SIGNER_SOURCE_OFFCHAIN")'
```

A key added by `paircaster key add` is added without gas, names the account itself as the app (`requestFid` is the account's own FID), has a `ttl` of 7776000 seconds (90 days), and has six scopes: casts and their removal, reactions and their removal, follows and unfollows. The vouching cast's `signer` field should be that key. The test account's key:

```run
curl -s "https://crackle.farcaster.xyz:3381/v1/signersByFid?fid=3356360" | jq '.signers[] | select(.source == "SIGNER_SOURCE_OFFCHAIN")'
```

```output
{
  "source": "SIGNER_SOURCE_OFFCHAIN",
  "key": "0xffac75748030d214200d85986fecf5053a91d096749c4dd2cf41b7915e61212e",
  "keyType": 1,
  "fid": 3356360,
  "addedAt": 1792558984,
  "lastUsedAt": 1792563233,
  "ttl": 7776000,
  "expiresAt": 1800339233,
  "scopes": [
    "MESSAGE_TYPE_CAST_ADD",
    "MESSAGE_TYPE_CAST_REMOVE",
    "MESSAGE_TYPE_REACTION_ADD",
    "MESSAGE_TYPE_REACTION_REMOVE",
    "MESSAGE_TYPE_LINK_ADD",
    "MESSAGE_TYPE_LINK_REMOVE"
  ],
  "requestFid": 3356360,
  "nonce": 1
}
```

## Our own account

The same check works on what the test account has sent with Paircaster. On a node we do not run, every cast and delete held for it is signed by that key:

```run
curl -s "https://crackle.farcaster.xyz:3381/v1/castsByFid?fid=3356360" | jq -c '.messages[] | {type: .data.type, hash, signer}'
```

```output
{"type":"MESSAGE_TYPE_CAST_REMOVE","hash":"0xf1f1a93b7ebb7d840572fed96a9baf855edc1ae2","signer":"0xffac75748030d214200d85986fecf5053a91d096749c4dd2cf41b7915e61212e"}
{"type":"MESSAGE_TYPE_CAST_ADD","hash":"0x6734c1e81150dc1dea7f5546a61e48014a9bafb3","signer":"0xffac75748030d214200d85986fecf5053a91d096749c4dd2cf41b7915e61212e"}
{"type":"MESSAGE_TYPE_CAST_REMOVE","hash":"0xc4345d967d04c37340513285543217b833024466","signer":"0xffac75748030d214200d85986fecf5053a91d096749c4dd2cf41b7915e61212e"}
{"type":"MESSAGE_TYPE_CAST_ADD","hash":"0x3820fb679133cd50df09d95beb8a5fbe2553069d","signer":"0xffac75748030d214200d85986fecf5053a91d096749c4dd2cf41b7915e61212e"}
{"type":"MESSAGE_TYPE_CAST_ADD","hash":"0x4853a7d5837632037621a83f1068c8b57ba5ed24","signer":"0xffac75748030d214200d85986fecf5053a91d096749c4dd2cf41b7915e61212e"}
{"type":"MESSAGE_TYPE_CAST_ADD","hash":"0xbe178321497b59e9f677e4450f64b0723d5029b7","signer":"0xffac75748030d214200d85986fecf5053a91d096749c4dd2cf41b7915e61212e"}
```

| Hash | What it is |
|---|---|
| `0xf1f1a93b…` | The delete of the first test cast, from [Writing](client.md#writing:-casts,-reactions,-follows) |
| `0x6734c1e8…` | The [tester call](https://farcaster.xyz/selfcast/0x6734c1e8) |
| `0xc4345d96…` | The delete of an earlier copy of the tester call |
| `0x3820fb67…` | The first cast sent through our own node, while it was hours behind |
| `0x4853a7d5…` | A reply to it, sent through our own node by an AI agent through the skill |
| `0xbe178321…` | A reply to that, sent the same way |

A deleted cast leaves only its delete: the node keeps the `CAST_REMOVE` and drops the cast. The like, unlike, follow and unfollow in [Writing](client.md#writing:-casts,-reactions,-follows)'s table do not show here, and not in `reactionsByFid` or `linksByFid` either: those list only what is current, and each was undone.

<limit>

This is evidence, not proof. Another client can make a key with the same settings, and at least one other client uses the same six scopes with a shorter expiry. Nothing on chain shows which node a message went through, so the node half of a vouch rests on the voucher's report.

</limit>

---

Back to [the start](index.md)
