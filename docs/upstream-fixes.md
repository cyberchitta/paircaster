# Upstream doc fixes

Discrepancies between farcasterxyz docs and observed behaviour, found while
building Selfcast. Each entry becomes a candidate PR to a `farcasterxyz` repo.

| Date | Doc (URL) | Says | Actually | Evidence | PR |
|---|---|---|---|---|---|
| 2026-10-18 | snapchain `site/docs/pages/reference/httpapi/httpapi.md` | paging uses `reverse=1`; errors are `{errCode, details}` | `reverse=1` → 400 "provided string was not `true` or `false`"; errors are `{error, error_detail}`, or plain text for bad query params | curl against crackle, 2026-10-18 | |
| 2026-10-18 | snapchain `httpapi/userdata.md` | `userDataByFid` takes a `user_data_type` filter | filter is ignored | snapchain v0.14.3 `src/network/http_server.rs` (unconfirmed live) | |
| 2026-10-18 | snapchain `httpapi/onchain.md` | documents `onChainSignersByFid` | `signersByFid` / `signer` undocumented; `onChainSignersByFid` deprecated | snapchain v0.14.3 router (unconfirmed live) | |
| 2026-10-18 | snapchain `README.md` vs `site/docs/pages/getting-started.mdx` | 1.5 TB disk, sync up to 2 h / 2 TB, a few hours | the two disagree; mainnet DB ≈ 549 GB (crackle `/v1/info`) | crackle `/v1/info`, 2026-10-18 | |
| 2026-10-18 | snapchain `writing-messages.mdx` | crackle is password-protected | HTTP reads are open | curl against crackle, 2026-10-18 | |
| 2026-10-18 | docs.farcaster.xyz `reference/farcaster/signer-requests` | onchain signer-request flow only | gasless `KEY_ADD` exists since engine V16 | snapchain v0.14.3 `src/core/validations/key.rs` | |
| 2026-10-18 | snapchain `scripts/snapchain.sh` (`store_operator_fid_env`) | blank operator FID prints "Not a valid FID or username" | blank is a legitimate answer (sets `HUB_OPERATOR_FID=0`); message reads like an error | ran bootstrap on macOS, 2026-10-18 | |
| 2026-10-18 | snapchain `README.md` / node docs | snapshot sync "up to 2 hours"; nothing on interruption | ~14 h at ~8 MB/s sequential; no resume, and a crash after the first shard unpacks leaves a silently incomplete DB (`src/storage/db/snapshot.rs`, `src/main.rs` `db_is_empty`) | local run on macOS, 2026-10-18 | |
