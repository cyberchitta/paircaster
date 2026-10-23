//! Turns the node's HTTP JSON into Selfcast's types. Pure, so it is tested on
//! saved responses without a node.
//!
//! Paged responses follow snapchain v0.14.3 `src/network/http_server.rs`
//! (`PagedResponse`, line 439); the last page's `nextPageToken` is `""`
//! (observed on `crackle`, 2026-10-19), which ends paging here.

use base64::prelude::*;
use serde::Deserialize;
use serde_json::Value;

use super::{Page, ShardSync, Submitted, SyncState, node_error, unexpected};
use crate::Error;
use crate::cast::{Cast, CastRef, Mention, MessageHash, Reaction, SignerKey, Target};
use crate::custody::EthAddress;
use crate::profile::UserDatum;
use crate::proto::json::message_from_json;
use crate::proto::{
    Body, CastAddBody, CastParent, CastType, EmbedTarget, LinkTarget, Message, MessageData,
    MessageType, ReactionTarget, ReactionType, UserDataType,
};
use crate::signer::Signer;
use crate::time::Timestamp;

/// A `POST /v1/submitMessage` reply to the message whose hash is `sent`.
///
/// A message the node already holds comes back as HTTP 400 with
/// `bad_request.duplicate/…` inside `error_detail` (snapchain v0.14.3
/// `src/network/server.rs` lines 1326-1340, test `server_tests.rs` line 5447),
/// and counts as sent. A success echoes the message, whose hash must be
/// `sent`.
pub fn submitted(status: u16, body: &str, sent: MessageHash) -> Result<Submitted, Error> {
    if !(200..300).contains(&status) {
        return match node_error(status, body.to_owned()) {
            Error::Node { detail, .. } if detail.contains("bad_request.duplicate/") => {
                Ok(Submitted::AlreadyKnown)
            }
            other => Err(other),
        };
    }
    #[derive(Deserialize)]
    struct Echo {
        hash: String,
    }
    let echo: Echo = serde_json::from_str(body).map_err(|_| unexpected(status, body.to_owned()))?;
    if echo.hash == sent.to_string() {
        Ok(Submitted::Accepted)
    } else {
        Err(bad(&format!("node accepted {}, not {sent}", echo.hash)))
    }
}

/// A `POST /v1/validateMessage` reply: `{ "valid": bool, "message": … }`
/// (snapchain v0.14.3 `src/network/server.rs`, `validate_message`).
pub fn validated(status: u16, body: &str) -> Result<bool, Error> {
    if !(200..300).contains(&status) {
        return Err(node_error(status, body.to_owned()));
    }
    #[derive(Deserialize)]
    struct Validated {
        valid: bool,
    }
    serde_json::from_str::<Validated>(body)
        .map(|v| v.valid)
        .map_err(|_| unexpected(status, body.to_owned()))
}

/// The hash of the one message a `castById`, `reactionById` or `linkById`
/// lookup returns.
pub fn message_hash(value: Value) -> Result<MessageHash, Error> {
    let message = message_from_json(value)?;
    MessageHash::from_bytes(&message.hash).ok_or_else(|| bad("hash is not 20 bytes"))
}

/// A `castsByFid` page. Cast removes in it are skipped.
pub fn casts(page: Value) -> Result<Page<Cast>, Error> {
    messages(page)?.try_filter_map(|m| match message_type(&m)? {
        MessageType::CastAdd => cast(&m).map(Some),
        MessageType::CastRemove => Ok(None),
        other => Err(bad(&format!("{other:?} in a page of casts"))),
    })
}

/// A `castById` response.
pub fn cast_by_id(value: Value) -> Result<Cast, Error> {
    cast(&message_from_json(value)?)
}

/// A `userDataByFid` page.
pub fn user_data(page: Value) -> Result<Page<UserDatum>, Error> {
    messages(page)?.try_filter_map(|m| {
        let (data, body) = data_and_body(&m)?;
        match body {
            Body::UserData(u) => Ok(Some(UserDatum {
                kind: enum_value::<UserDataType>(u.r#type)?,
                value: u.value.clone(),
                timestamp: Timestamp::from_farcaster(data.timestamp),
            })),
            _ => Err(bad("non-user-data message in a page of user data")),
        }
    })
}

/// A `linksByFid?link_type=follow` page, as the FIDs followed.
pub fn follows(page: Value) -> Result<Page<u64>, Error> {
    messages(page)?.try_filter_map(|m| match data_and_body(&m)?.1 {
        Body::Link(link) if link.r#type == "follow" => match link.target {
            Some(LinkTarget::Fid(fid)) => Ok(Some(fid)),
            None => Err(bad("follow without a target")),
        },
        _ => Err(bad("non-follow message in a page of follows")),
    })
}

/// A `linksByTargetFid?link_type=follow` page, as the FIDs following
/// `target`.
pub fn followers(page: Value, target: u64) -> Result<Page<u64>, Error> {
    messages(page)?.try_filter_map(|m| {
        let (data, body) = data_and_body(&m)?;
        match body {
            Body::Link(link) if link.r#type == "follow" => match link.target {
                Some(LinkTarget::Fid(fid)) if fid == target => Ok(Some(data.fid)),
                Some(LinkTarget::Fid(_)) => Err(bad("follower of another account")),
                None => Err(bad("follow without a target")),
            },
            _ => Err(bad("non-follow message in a page of followers")),
        }
    })
}

/// A `reactionsByCast` page of `kind` reactions to `cast`.
pub fn reactions_to_cast(
    page: Value,
    cast: &CastRef,
    kind: ReactionType,
) -> Result<Page<Reaction>, Error> {
    let page = reactions(page)?;
    let target = Target::Cast(*cast);
    match page
        .items
        .iter()
        .any(|r| r.kind != kind || r.target != target)
    {
        true => Err(bad("reaction of another kind or to another cast")),
        false => Ok(page),
    }
}

/// A `reactionsByFid` page.
pub fn reactions(page: Value) -> Result<Page<Reaction>, Error> {
    messages(page)?.try_filter_map(|m| {
        let (data, body) = data_and_body(&m)?;
        match body {
            Body::Reaction(r) => Ok(Some(Reaction {
                fid: data.fid,
                kind: enum_value::<ReactionType>(r.r#type)?,
                target: match &r.target {
                    Some(ReactionTarget::CastId(id)) => Target::Cast(cast_ref(id.fid, &id.hash)?),
                    Some(ReactionTarget::Url(url)) => Target::Url(url.clone()),
                    None => return Err(bad("reaction without a target")),
                },
                timestamp: Timestamp::from_farcaster(data.timestamp),
            })),
            _ => Err(bad("non-reaction message in a page of reactions")),
        }
    })
}

/// A `signersByFid` page.
pub fn signers(page: Value) -> Result<Page<Signer>, Error> {
    let raw: RawSigners = from_value(page)?;
    let items = raw
        .signers
        .into_iter()
        .map(signer)
        .collect::<Result<_, _>>()?;
    Ok(Page::new(items, raw.next_page_token))
}

/// The `currentUserNonce` of a `signersByFid` page (snapchain v0.14.3
/// `http_server.rs:1195`).
pub fn user_nonce(page: Value) -> Result<u32, Error> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Raw {
        current_user_nonce: u32,
    }
    from_value::<Raw>(page).map(|r| r.current_user_nonce)
}

/// An `onChainIdRegistryEventByAddress` reply, as the FID it registers,
/// checked to name `address` as its custody address.
pub fn id_register(event: Value, address: &EthAddress) -> Result<u64, Error> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Raw {
        fid: u64,
        id_register_event_body: RawIdRegister,
    }
    #[derive(Deserialize)]
    struct RawIdRegister {
        to: String,
    }
    let raw: Raw = from_value(event)?;
    let to = hex_bytes(&raw.id_register_event_body.to)?;
    match EthAddress::from_bytes(&to) {
        Some(a) if a == *address => Ok(raw.fid),
        _ => Err(bad("id registry event for a different custody address")),
    }
}

/// An `onChainEventsByFid` reply for `EVENT_TYPE_ID_REGISTER`, as whether
/// it holds the account's registration. A node that has not seen the
/// account answers with an empty `events` (observed on our node while it
/// synced, 2026-10-21).
pub fn has_id_register(page: Value) -> Result<bool, Error> {
    #[derive(Deserialize)]
    struct Raw {
        events: Vec<Value>,
    }
    from_value::<Raw>(page).map(|r| !r.events.is_empty())
}

/// Whether `e` is the node's "no such record" reply: HTTP 400 with a gRPC
/// `NotFound` status inside `error_detail` (observed on `crackle`,
/// 2026-10-19, for `onChainIdRegistryEventByAddress`).
pub fn is_not_found(e: &Error) -> bool {
    matches!(e, Error::Node { status: 400, detail, .. } if detail.contains("status: NotFound"))
}

/// A `GET /v1/info` reply. `blockDelay` is the node's clock minus the
/// shard's newest block time, in seconds (snapchain v0.14.3
/// `src/network/server.rs` lines 1588 and 1637).
pub fn sync_state(value: Value) -> Result<SyncState, Error> {
    let raw: RawInfo = from_value(value)?;
    let shards = raw
        .shard_infos
        .into_iter()
        .map(|s| ShardSync {
            id: s.shard_id,
            height: s.max_height,
            seconds_behind: s.block_delay,
        })
        .collect();
    Ok(SyncState {
        version: raw.version,
        shards,
    })
}

/// `InfoResponse` at `http_server.rs:455`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawInfo {
    version: String,
    shard_infos: Vec<RawShardInfo>,
}

/// `ShardInfo` at `http_server.rs:480`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawShardInfo {
    shard_id: u32,
    max_height: u64,
    block_delay: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPage {
    messages: Vec<Value>,
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSigners {
    signers: Vec<RawSigner>,
    next_page_token: Option<String>,
}

/// `Signer` at `http_server.rs:1143`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSigner {
    source: String,
    key: String,
    fid: u64,
    request_fid: Option<u64>,
    on_chain_event: Option<RawOnChainEvent>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawOnChainEvent {
    signer_event_body: RawSignerEventBody,
}

/// `SignerEventBody` at `http_server.rs:1010`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSignerEventBody {
    metadata: String,
    metadata_type: u32,
}

impl<T> Page<T> {
    fn new(items: Vec<T>, next_page_token: Option<String>) -> Self {
        Page {
            items,
            next_page_token: next_page_token.filter(|t| !t.is_empty()),
        }
    }
}

impl Page<Message> {
    fn try_filter_map<T>(
        self,
        f: impl Fn(Message) -> Result<Option<T>, Error>,
    ) -> Result<Page<T>, Error> {
        let items = self
            .items
            .into_iter()
            .filter_map(|m| f(m).transpose())
            .collect::<Result<_, _>>()?;
        Ok(Page {
            items,
            next_page_token: self.next_page_token,
        })
    }
}

fn messages(page: Value) -> Result<Page<Message>, Error> {
    let raw: RawPage = from_value(page)?;
    let items = raw
        .messages
        .into_iter()
        .map(message_from_json)
        .collect::<Result<_, _>>()?;
    Ok(Page::new(items, raw.next_page_token))
}

fn cast(m: &Message) -> Result<Cast, Error> {
    let (data, body) = data_and_body(m)?;
    let Body::CastAdd(add) = body else {
        return Err(bad("not a cast add"));
    };
    Ok(Cast {
        id: cast_ref(data.fid, &m.hash)?,
        timestamp: Timestamp::from_farcaster(data.timestamp),
        kind: enum_value::<CastType>(add.r#type)?,
        text: add.text.clone(),
        mentions: mentions(add)?,
        embeds: add.embeds.iter().map(embed).collect::<Result<_, _>>()?,
        parent: add.parent.as_ref().map(parent).transpose()?,
        signer: SignerKey::from_bytes(&m.signer).ok_or_else(|| bad("signer is not 32 bytes"))?,
    })
}

fn mentions(add: &CastAddBody) -> Result<Vec<Mention>, Error> {
    if add.mentions.len() != add.mentions_positions.len() {
        return Err(bad("mentions and mentionsPositions differ in length"));
    }
    // Snapchain refuses either (v0.14.3 `src/core/validations/cast.rs`).
    let text_len = add.text.len();
    if add
        .mentions_positions
        .iter()
        .any(|&p| p as usize > text_len)
    {
        return Err(bad("a mention position is past the end of the text"));
    }
    if add.mentions_positions.windows(2).any(|w| w[0] > w[1]) {
        return Err(bad("mention positions are not in ascending order"));
    }
    Ok(add
        .mentions
        .iter()
        .zip(&add.mentions_positions)
        .map(|(fid, position)| Mention {
            fid: *fid,
            position: *position,
        })
        .collect())
}

fn embed(e: &crate::proto::Embed) -> Result<Target, Error> {
    match &e.embed {
        Some(EmbedTarget::Url(url)) => Ok(Target::Url(url.clone())),
        Some(EmbedTarget::CastId(id)) => Ok(Target::Cast(cast_ref(id.fid, &id.hash)?)),
        None => Err(bad("empty embed")),
    }
}

fn parent(p: &CastParent) -> Result<Target, Error> {
    match p {
        CastParent::CastId(id) => Ok(Target::Cast(cast_ref(id.fid, &id.hash)?)),
        CastParent::Url(url) => Ok(Target::Url(url.clone())),
    }
}

fn hex_bytes(text: &str) -> Result<Vec<u8>, Error> {
    text.strip_prefix("0x")
        .and_then(|h| hex::decode(h).ok())
        .ok_or_else(|| bad(&format!("not 0x-prefixed hex: {text:?}")))
}

fn cast_ref(fid: u64, hash: &[u8]) -> Result<CastRef, Error> {
    let hash = MessageHash::from_bytes(hash).ok_or_else(|| bad("hash is not 20 bytes"))?;
    Ok(CastRef { fid, hash })
}

fn signer(raw: RawSigner) -> Result<Signer, Error> {
    let key_bytes = hex_bytes(&raw.key)?;
    let key = SignerKey::from_bytes(&key_bytes).ok_or_else(|| bad("signer key is not 32 bytes"))?;
    let app_fid = match (raw.source.as_str(), raw.request_fid, raw.on_chain_event) {
        ("SIGNER_SOURCE_OFFCHAIN", Some(fid), _) => Some(fid),
        ("SIGNER_SOURCE_OFFCHAIN", None, _) => {
            return Err(bad("gasless signer without requestFid"));
        }
        ("SIGNER_SOURCE_ONCHAIN", _, Some(event)) => onchain_app_fid(&event.signer_event_body)?,
        ("SIGNER_SOURCE_ONCHAIN", _, None) => {
            return Err(bad("onchain signer without onChainEvent"));
        }
        (other, _, _) => return Err(bad(&format!("signer source {other:?}"))),
    };
    Ok(Signer {
        fid: raw.fid,
        key,
        app_fid,
    })
}

/// Metadata type 1 is a `SignedKeyRequest`, which names the requesting app;
/// no other type does (snapchain v0.14.3
/// `src/connectors/onchain_events/mod.rs:239-252`).
fn onchain_app_fid(body: &RawSignerEventBody) -> Result<Option<u64>, Error> {
    if body.metadata_type != 1 {
        return Ok(None);
    }
    let metadata = BASE64_STANDARD
        .decode(&body.metadata)
        .map_err(|e| bad(&format!("signer metadata is not base64: {e}")))?;
    request_fid(&metadata).map(Some)
}

/// `requestFid` from ABI-encoded `SignedKeyRequestMetadata`, as the
/// Farcaster contracts encode it with `abi.encode(metadata)`:
/// `struct { uint256 requestFid; address requestSigner; bytes signature;
/// uint256 deadline; }` (snapchain v0.14.3
/// `src/connectors/onchain_events/mod.rs:113-123`, mirroring
/// `SignedKeyRequestValidator.sol`). The struct holds `bytes`, so the
/// encoding opens with a 32-byte word giving the struct's offset, and
/// `requestFid` is the struct's first 32-byte word, big-endian.
pub fn request_fid(metadata: &[u8]) -> Result<u64, Error> {
    let offset = word(metadata, 0)
        .and_then(u64_from_word)
        .and_then(|o| usize::try_from(o).ok());
    let fid_word = offset.and_then(|o| word(metadata, o));
    fid_word
        .and_then(u64_from_word)
        .ok_or_else(|| bad("signer metadata is not a SignedKeyRequestMetadata"))
}

fn word(bytes: &[u8], at: usize) -> Option<&[u8]> {
    bytes.get(at..at.checked_add(32)?)
}

fn u64_from_word(word: &[u8]) -> Option<u64> {
    let (high, low) = word.split_at(24);
    let low: [u8; 8] = low.try_into().ok()?;
    high.iter()
        .all(|b| *b == 0)
        .then(|| u64::from_be_bytes(low))
}

fn data_and_body(m: &Message) -> Result<(&MessageData, &Body), Error> {
    let data = m.data.as_ref().ok_or_else(|| bad("message without data"))?;
    let body = data
        .body
        .as_ref()
        .ok_or_else(|| bad("message without a body"))?;
    Ok((data, body))
}

fn message_type(m: &Message) -> Result<MessageType, Error> {
    let data = m.data.as_ref().ok_or_else(|| bad("message without data"))?;
    enum_value(data.r#type)
}

fn enum_value<E: TryFrom<i32>>(value: i32) -> Result<E, Error> {
    E::try_from(value).map_err(|_| bad(&format!("unknown enum value {value}")))
}

fn from_value<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, Error> {
    serde_json::from_value(value).map_err(|e| bad(&e.to_string()))
}

fn bad(reason: &str) -> Error {
    Error::Malformed(reason.to_owned())
}
