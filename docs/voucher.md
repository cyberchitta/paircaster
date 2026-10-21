---
title: Checking a voucher
---

A voucher says they ran the guide. Part of that can be checked by anyone, on any node. We ask vouchers to post the vouching cast with `selfcast cast`, so it is signed by the app key Selfcast added for them. Look the account's keys up:

```run
curl -s "http://localhost:3381/v1/signersByFid?fid=FID" | jq '.signers[] | select(.source == "SIGNER_SOURCE_OFFCHAIN")'
```

A key added by `selfcast key add` is added without gas, names the account itself as the app (`requestFid` is the account's own FID), has a `ttl` of 7776000 seconds (90 days), and has six scopes: casts and their removal, reactions and their removal, follows and unfollows. The vouching cast's `signer` field should be that key. The test account's key:

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

<limit>

This is evidence, not proof. Another client can make a key with the same settings, and at least one other client uses the same six scopes with a shorter expiry. Nothing on chain shows which node a message went through, so the node half of a vouch rests on the voucher's report.

</limit>

---

Back to [the start](index.md)
