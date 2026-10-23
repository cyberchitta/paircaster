pub mod parse;
pub mod reads;

use std::hash::Hash;

use reqwest::{Client, Url};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::account::Username;
use crate::address::Verification;
use crate::build::SignedMessage;
use crate::cast::{Cast, CastRef, ChannelUrl, MessageHash, Reaction};
use crate::custody::EthAddress;
use crate::error::{Error, NodeErrorBody};
use crate::event::{PageEvent, first_event_of};
use crate::follow::{self, Follow};
use crate::profile::{FollowerTally, Followers, UserDatum};
use crate::proto::ReactionType;
use crate::signer::Signer;
use crate::tally::PageTally;
use crate::thread::{Ancestors, ChainTop};
use crate::time::Timestamp;

/// Per shard, so a page holds up to this many times the shard count.
const FOLLOWER_PAGE_SIZE: u32 = 1000;

/// Per shard, as for followers: for replies and reactions to a cast.
const REPEATING_PAGE_SIZE: u32 = 1000;

/// One page of a paged read. `next_page_token` is `None` on the last page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_page_token: Option<String>,
}

/// Where a paged read starts and how much it returns.
#[derive(Debug, Clone, Default)]
pub struct PageRequest {
    /// The node's default when `None`.
    pub page_size: Option<u32>,
    /// From a previous page's `next_page_token`; the first page when `None`.
    pub page_token: Option<String>,
}

impl PageRequest {
    fn query(&self) -> Vec<(&'static str, String)> {
        let size = self.page_size.map(|s| ("page_size", s.to_string()));
        let token = self.page_token.clone().map(|t| ("page_token", t));
        size.into_iter().chain(token).collect()
    }
}

/// What a node did with a submitted message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Submitted {
    /// Accepted for relay to validators. Not proof it was merged: read it
    /// back to confirm.
    Accepted,
    /// The node already holds it, merged or waiting to be. Sending the same
    /// bytes again is how a retry lands here.
    AlreadyKnown,
}

/// The node's version and how far behind each of its shards is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncState {
    pub version: String,
    pub shards: Vec<ShardSync>,
}

/// One shard: shard 0 holds the block chain, the others hold messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShardSync {
    pub id: u32,
    pub height: u64,
    /// Seconds between the node's clock and the shard's newest block.
    pub seconds_behind: u64,
}

impl SyncState {
    /// The furthest any shard is behind, in seconds; `None` with no shards.
    pub fn seconds_behind(&self) -> Option<u64> {
        self.shards.iter().map(|s| s.seconds_behind).max()
    }

    /// The shard that holds `fid`'s messages; `None` when the node reports
    /// no message shards.
    pub fn shard_of(&self, fid: u64) -> Option<&ShardSync> {
        let count = self.shards.iter().filter(|s| s.id != 0).count();
        let id = route_fid(fid, u32::try_from(count).ok()?)?;
        self.shards.iter().find(|s| s.id == id)
    }
}

/// Which of `count` message shards, numbered from 1, holds `fid`'s messages:
/// the first four bytes of SHA-256 over the FID as a big-endian `u32`, mod
/// `count`, plus one. Mirrors snapchain `src/mempool/routing.rs`,
/// `ShardRouter::route_fid` (commit `f3e857f`). `None` when `count` is 0 or
/// `fid` does not fit the `u32` Snapchain stores.
pub fn route_fid(fid: u64, count: u32) -> Option<u32> {
    let fid = u32::try_from(fid).ok()?;
    let digest = Sha256::digest(fid.to_be_bytes());
    let head = u32::from_be_bytes(
        digest[..4]
            .try_into()
            .expect("a SHA-256 digest is 32 bytes"),
    );
    head.checked_rem(count).map(|r| r + 1)
}

/// A connection to one Snapchain node's HTTP API.
#[derive(Debug, Clone)]
pub struct Node {
    base: Url,
    http: Client,
}

impl Node {
    /// Connects to a node at `url`, e.g. `http://localhost:3381`.
    ///
    /// ```
    /// assert!(paircaster_core::Node::new("http://localhost:3381").is_ok());
    /// assert!(paircaster_core::Node::new("localhost:3381").is_err());
    /// ```
    pub fn new(url: &str) -> Result<Self, Error> {
        let base = parse_base(url)?;
        Ok(Self {
            base,
            http: Client::new(),
        })
    }

    /// `GET /v1/info`: node version and per-shard sync state.
    pub async fn info(&self) -> Result<Value, Error> {
        self.get("v1/info", &[]).await
    }

    /// `GET /v1/info`, as how far behind each shard is.
    pub async fn sync_state(&self) -> Result<SyncState, Error> {
        parse::sync_state(self.info().await?)
    }

    /// `GET /v1/castsByFid`, newest first.
    pub async fn casts_by_fid(&self, fid: u64, page: &PageRequest) -> Result<Page<Cast>, Error> {
        let query = with_page(&[fid_param(fid), ("reverse", "true".to_owned())], page);
        parse::casts(self.get("v1/castsByFid", &query).await?)
    }

    /// `GET /v1/castById`.
    pub async fn cast_by_id(&self, fid: u64, hash: &MessageHash) -> Result<Cast, Error> {
        let query = [fid_param(fid), ("hash", hash.to_string())];
        parse::cast_by_id(self.get("v1/castById", &query).await?)
    }

    /// `GET /v1/castsByParent`: replies to a cast. Pages repeat; see
    /// [`PageTally`].
    pub async fn casts_by_parent(
        &self,
        parent: &CastRef,
        page: &PageRequest,
    ) -> Result<Page<Cast>, Error> {
        let query = with_page(
            &[fid_param(parent.fid), ("hash", parent.hash.to_string())],
            page,
        );
        parse::casts(self.get("v1/castsByParent", &query).await?)
    }

    /// `GET /v1/castsByParent` by URL, newest first: the casts posted in
    /// `channel`, not the replies to them (a reply's parent is a cast).
    /// Each shard answers with its own newest, so a page holds up to
    /// `page_size` times the shard count, and the newest `page_size` of it
    /// are the channel's newest.
    pub async fn casts_in_channel(
        &self,
        channel: &ChannelUrl,
        page: &PageRequest,
    ) -> Result<Page<Cast>, Error> {
        let query = with_page(
            &[("url", channel.to_string()), ("reverse", "true".to_owned())],
            page,
        );
        parse::casts_in_channel(self.get("v1/castsByParent", &query).await?, channel)
    }

    /// Every reply to `parent`, each once: pages are read until one brings
    /// no new reply or there is no next page.
    pub async fn replies(&self, parent: &CastRef) -> Result<Vec<Cast>, Error> {
        self.until_repeat(
            |page| async move { self.casts_by_parent(parent, &page).await },
            |c: &Cast| c.id.hash,
        )
        .await
    }

    /// `GET /v1/reactionsByCast` for one kind of reaction. Pages repeat; see
    /// [`PageTally`].
    pub async fn reactions_by_cast(
        &self,
        cast: &CastRef,
        kind: ReactionType,
        page: &PageRequest,
    ) -> Result<Page<Reaction>, Error> {
        let query = with_page(
            &[
                ("target_fid", cast.fid.to_string()),
                ("target_hash", cast.hash.to_string()),
                ("reaction_type", reaction_type_name(kind)?.to_owned()),
            ],
            page,
        );
        parse::reactions_to_cast(self.get("v1/reactionsByCast", &query).await?, cast, kind)
    }

    /// Every `kind` reaction to `cast`, one per account.
    pub async fn reactions_to_cast(
        &self,
        cast: &CastRef,
        kind: ReactionType,
    ) -> Result<Vec<Reaction>, Error> {
        self.until_repeat(
            |page| async move { self.reactions_by_cast(cast, kind, &page).await },
            |r: &Reaction| r.fid,
        )
        .await
    }

    /// The casts `cast` replies to, root first: its parent, that cast's
    /// parent, and so on, reading at most `max` of them. A parent the node
    /// does not hold ends the chain.
    pub async fn ancestors(&self, cast: &Cast, max: usize) -> Result<Ancestors, Error> {
        let mut casts: Vec<Cast> = Vec::new();
        let mut next = cast.parent_cast();
        let top = loop {
            let Some(id) = next else {
                break ChainTop::Root;
            };
            if casts.len() == max {
                break ChainTop::Cut(id);
            }
            match self.cast_by_id(id.fid, &id.hash).await {
                Ok(parent) => {
                    next = parent.parent_cast();
                    casts.push(parent);
                }
                Err(e) if parse::is_not_found(&e) => break ChainTop::Missing(id),
                Err(e) => return Err(e),
            }
        };
        casts.reverse();
        Ok(Ancestors { casts, top })
    }

    /// `GET /v1/userDataByFid`, every page.
    pub async fn user_data_by_fid(&self, fid: u64) -> Result<Vec<UserDatum>, Error> {
        self.all_pages(|page| async move {
            let query = with_page(&[fid_param(fid)], &page);
            parse::user_data(self.get("v1/userDataByFid", &query).await?)
        })
        .await
    }

    /// `GET /v1/linksByFid?link_type=follow`: the FIDs an account follows.
    pub async fn follows_by_fid(&self, fid: u64, page: &PageRequest) -> Result<Page<u64>, Error> {
        let query = with_page(&[fid_param(fid), ("link_type", "follow".to_owned())], page);
        parse::follows(self.get("v1/linksByFid", &query).await?)
    }

    /// Every FID an account follows, across all pages.
    pub async fn all_follows_by_fid(&self, fid: u64) -> Result<Vec<u64>, Error> {
        self.all_pages(|page| async move { self.follows_by_fid(fid, &page).await })
            .await
    }

    /// `GET /v1/linksByTargetFid?link_type=follow`: FIDs following an
    /// account. Pages repeat; see [`FollowerTally`].
    pub async fn followers_by_fid(&self, fid: u64, page: &PageRequest) -> Result<Page<u64>, Error> {
        let params = [
            ("target_fid", fid.to_string()),
            ("link_type", "follow".to_owned()),
        ];
        parse::followers(
            self.get("v1/linksByTargetFid", &with_page(&params, page))
                .await?,
            fid,
        )
    }

    /// The newest `limit` accounts `fid` follows, newest first, with when
    /// it followed each. An account's follows are on its own shard, read
    /// newest first.
    pub async fn newest_follows(&self, fid: u64, limit: u32) -> Result<Vec<Follow>, Error> {
        let query = [
            fid_param(fid),
            ("link_type", "follow".to_owned()),
            ("page_size", limit.to_string()),
            ("reverse", "true".to_owned()),
        ];
        let page = parse::follow_links(self.get("v1/linksByFid", &query).await?)?;
        Ok(follow::newest(page.items, limit as usize))
    }

    /// The newest `limit` accounts that follow `fid`, newest first, with
    /// when each followed. Each shard answers with its own newest `limit`,
    /// in time order (observed on our node), so one request holds the
    /// newest `limit` of all.
    pub async fn newest_followers(&self, fid: u64, limit: u32) -> Result<Vec<Follow>, Error> {
        let query = [
            ("target_fid", fid.to_string()),
            ("link_type", "follow".to_owned()),
            ("page_size", limit.to_string()),
            ("reverse", "true".to_owned()),
        ];
        let page = parse::follower_links(self.get("v1/linksByTargetFid", &query).await?, fid)?;
        Ok(follow::newest(page.items, limit as usize))
    }

    /// How many accounts follow `fid`, counted up to `cap` (all of them
    /// when `None`): one request per 1000 followers per shard, plus one.
    pub async fn follower_count(&self, fid: u64, cap: Option<usize>) -> Result<Followers, Error> {
        let mut tally = FollowerTally::default();
        let mut request = PageRequest {
            page_size: Some(FOLLOWER_PAGE_SIZE),
            page_token: None,
        };
        loop {
            let page = self.followers_by_fid(fid, &request).await?;
            let more = page.next_page_token.is_some();
            if let Some(done) = tally.step(&page.items, more, cap) {
                return Ok(done);
            }
            request.page_token = page.next_page_token;
        }
    }

    /// `GET /v1/reactionsByFid` for one kind of reaction.
    pub async fn reactions_by_fid(
        &self,
        fid: u64,
        kind: ReactionType,
        page: &PageRequest,
    ) -> Result<Page<Reaction>, Error> {
        let query = with_page(
            &[
                fid_param(fid),
                ("reaction_type", reaction_type_name(kind)?.to_owned()),
            ],
            page,
        );
        parse::reactions(self.get("v1/reactionsByFid", &query).await?)
    }

    /// `GET /v1/signersByFid`, every page: an account's app keys.
    pub async fn signers_by_fid(&self, fid: u64) -> Result<Vec<Signer>, Error> {
        self.all_pages(|page| async move {
            let query = with_page(&[fid_param(fid)], &page);
            parse::signers(self.get("v1/signersByFid", &query).await?)
        })
        .await
    }

    /// `GET /v1/signersByFid`'s `currentUserNonce`: a `KEY_ADD` must carry
    /// a higher nonce than this.
    pub async fn user_nonce(&self, fid: u64) -> Result<u32, Error> {
        parse::user_nonce(self.get("v1/signersByFid", &[fid_param(fid)]).await?)
    }

    /// `GET /v1/verificationsByFid`: every address `fid` has verified.
    pub async fn verifications_by_fid(&self, fid: u64) -> Result<Vec<Verification>, Error> {
        self.all_pages(|page| async move {
            let query = with_page(&[fid_param(fid)], &page);
            parse::verifications(self.get("v1/verificationsByFid", &query).await?)
        })
        .await
    }

    /// `GET /v1/onChainEventsByFid` for `EVENT_TYPE_ID_REGISTER`: whether
    /// the node has seen the account registered. A node still syncing may
    /// not have.
    pub async fn is_registered(&self, fid: u64) -> Result<bool, Error> {
        let query = [
            fid_param(fid),
            ("event_type", "EVENT_TYPE_ID_REGISTER".to_string()),
        ];
        parse::has_id_register(self.get("v1/onChainEventsByFid", &query).await?)
    }

    /// `GET /v1/onChainIdRegistryEventByAddress`: the account whose custody
    /// address is `address`, or `None` if it has none.
    pub async fn fid_of_custody(&self, address: &EthAddress) -> Result<Option<u64>, Error> {
        let query = [("address", format!("0x{}", hex::encode(address.as_bytes())))];
        match self.get("v1/onChainIdRegistryEventByAddress", &query).await {
            Ok(event) => parse::id_register(event, address).map(Some),
            Err(e) if parse::is_not_found(&e) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// `GET /v1/userNameProofByName`: the account that the node's proof for
    /// `name` names. A name with no proof is [`Error::UnknownName`].
    pub async fn fid_of_name(&self, name: &Username) -> Result<u64, Error> {
        let query = [("name", name.to_string())];
        match self.get("v1/userNameProofByName", &query).await {
            Ok(proof) => parse::name_proof(proof, name),
            Err(e) if parse::is_not_found(&e) => Err(Error::UnknownName(name.clone())),
            Err(e) => Err(e),
        }
    }

    /// `GET /v1/events`: up to `page_size` of `shard`'s events, from event
    /// id `from` on, in id order. Shard 0 orders blocks and keeps no events:
    /// it answers "Shard not found" (observed on our node).
    pub async fn events(
        &self,
        shard: u32,
        from: u64,
        page_size: u32,
    ) -> Result<Vec<PageEvent>, Error> {
        let query = [
            ("shard_index", shard.to_string()),
            ("from_event_id", from.to_string()),
            ("page_size", page_size.to_string()),
        ];
        parse::events(self.get("v1/events", &query).await?)
    }

    /// The id of the first event in the earliest block on `shard` dated at
    /// or after `since`, found by halving the heights up to `newest`. Past
    /// every event the node holds when no block is that late.
    pub async fn first_event_since(
        &self,
        shard: u32,
        since: Timestamp,
        newest: u64,
    ) -> Result<u64, Error> {
        let (mut low, mut high) = (0, newest + 1);
        while low < high {
            let mid = low + (high - low) / 2;
            match self.block_from(shard, mid).await? {
                Some((block, time)) if time < since => low = block + 1,
                _ => high = mid,
            }
        }
        Ok(first_event_of(low))
    }

    /// The first block at or after `height` that the node holds events
    /// for, with its time; `None` past the newest.
    async fn block_from(&self, shard: u32, height: u64) -> Result<Option<(u64, Timestamp)>, Error> {
        match self.events(shard, first_event_of(height), 1).await?.first() {
            None => Ok(None),
            Some(PageEvent::Block { block, time }) => Ok(Some((*block, *time))),
            Some(PageEvent::Event { id, .. }) => Err(Error::Malformed(format!(
                "event {id} came before its block's confirmation"
            ))),
        }
    }

    /// `POST /v1/submitMessage` with `message`'s bytes exactly as signed.
    pub async fn submit_message(&self, message: &SignedMessage) -> Result<Submitted, Error> {
        let (status, body) = self.post_message("v1/submitMessage", message).await?;
        parse::submitted(status, &body, message.hash())
    }

    /// `POST /v1/validateMessage`: whether the node would take `message` as
    /// built. Nothing is sent on. It does not check that the signing key is
    /// registered to the account.
    pub async fn validate_message(&self, message: &SignedMessage) -> Result<bool, Error> {
        let (status, body) = self.post_message("v1/validateMessage", message).await?;
        parse::validated(status, &body)
    }

    /// `GET /v1/castById`: the hash of the cast at `cast`, or `None` if the
    /// node holds none there.
    pub async fn cast_hash(&self, cast: &CastRef) -> Result<Option<MessageHash>, Error> {
        let query = [fid_param(cast.fid), ("hash", cast.hash.to_string())];
        self.lookup_hash("v1/castById", &query).await
    }

    /// `GET /v1/reactionById`: the hash of `fid`'s current `kind` reaction
    /// to `cast`, or `None` if it has none.
    pub async fn reaction_hash(
        &self,
        fid: u64,
        kind: ReactionType,
        cast: &CastRef,
    ) -> Result<Option<MessageHash>, Error> {
        let query = [
            fid_param(fid),
            ("reaction_type", reaction_type_name(kind)?.to_owned()),
            ("target_fid", cast.fid.to_string()),
            ("target_hash", cast.hash.to_string()),
        ];
        self.lookup_hash("v1/reactionById", &query).await
    }

    /// `GET /v1/linkById`: the hash of `fid`'s current follow of `target`,
    /// or `None` if it does not follow it.
    pub async fn follow_hash(&self, fid: u64, target: u64) -> Result<Option<MessageHash>, Error> {
        let query = [
            fid_param(fid),
            ("target_fid", target.to_string()),
            ("link_type", "follow".to_owned()),
        ];
        self.lookup_hash("v1/linkById", &query).await
    }

    async fn lookup_hash(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Option<MessageHash>, Error> {
        match self.get(path, query).await {
            Ok(value) => parse::message_hash(value).map(Some),
            Err(e) if parse::is_not_found(&e) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn post_message(
        &self,
        path: &str,
        message: &SignedMessage,
    ) -> Result<(u16, String), Error> {
        let url = self.base.join(path).expect("static relative URL");
        let resp = self
            .http
            .post(url)
            .header("content-type", "application/octet-stream")
            .body(message.encode())
            .send()
            .await?;
        let status = resp.status().as_u16();
        Ok((status, resp.text().await?))
    }

    async fn all_pages<T, F, Fut>(&self, read: F) -> Result<Vec<T>, Error>
    where
        F: Fn(PageRequest) -> Fut,
        Fut: Future<Output = Result<Page<T>, Error>>,
    {
        let mut items = Vec::new();
        let mut request = PageRequest::default();
        loop {
            let page = read(request).await?;
            items.extend(page.items);
            match page.next_page_token {
                Some(token) => {
                    request = PageRequest {
                        page_size: None,
                        page_token: Some(token),
                    }
                }
                None => return Ok(items),
            }
        }
    }

    /// Every item of a read whose pages repeat, each once: pages are read
    /// until one brings nothing new or there is no next page.
    async fn until_repeat<K, T, F, Fut>(
        &self,
        read: F,
        key: impl Fn(&T) -> K,
    ) -> Result<Vec<T>, Error>
    where
        K: Eq + Hash,
        F: Fn(PageRequest) -> Fut,
        Fut: Future<Output = Result<Page<T>, Error>>,
    {
        let mut tally = PageTally::default();
        let mut request = PageRequest {
            page_size: Some(REPEATING_PAGE_SIZE),
            page_token: None,
        };
        loop {
            let page = read(request.clone()).await?;
            let fresh = tally.add_page(page.items, &key);
            match page.next_page_token {
                Some(token) if fresh => request.page_token = Some(token),
                _ => return Ok(tally.into_items()),
            }
        }
    }

    async fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value, Error> {
        let url = self.base.join(path).expect("path is a static relative URL");
        let resp = self.http.get(url).query(query).send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        if status.is_success() {
            serde_json::from_str(&body).map_err(|_| unexpected(status.as_u16(), body))
        } else {
            Err(node_error(status.as_u16(), body))
        }
    }
}

/// How the node's query strings name a reaction type.
fn reaction_type_name(kind: ReactionType) -> Result<&'static str, Error> {
    match kind {
        ReactionType::Like => Ok("Like"),
        ReactionType::Recast => Ok("Recast"),
        ReactionType::None => Err(Error::Malformed("reaction type NONE".to_owned())),
    }
}

fn fid_param(fid: u64) -> (&'static str, String) {
    ("fid", fid.to_string())
}

fn with_page(params: &[(&'static str, String)], page: &PageRequest) -> Vec<(&'static str, String)> {
    params.iter().cloned().chain(page.query()).collect()
}

fn parse_base(url: &str) -> Result<Url, Error> {
    let bad = |reason: &str| Error::BadNodeUrl {
        url: url.to_owned(),
        reason: reason.to_owned(),
    };
    let parsed = Url::parse(url).map_err(|e| bad(&e.to_string()))?;
    match parsed.scheme() {
        "http" | "https" if parsed.has_host() => Ok(parsed),
        "http" | "https" => Err(bad("missing host")),
        _ => Err(bad("scheme must be http or https")),
    }
}

fn node_error(status: u16, body: String) -> Error {
    match serde_json::from_str::<NodeErrorBody>(&body) {
        Ok(e) => Error::Node {
            status,
            error: e.error,
            detail: e.error_detail,
        },
        Err(_) => unexpected(status, body),
    }
}

fn unexpected(status: u16, body: String) -> Error {
    Error::UnexpectedResponse { status, body }
}
