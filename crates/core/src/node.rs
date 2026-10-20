pub mod parse;

use reqwest::{Client, Url};
use serde_json::Value;

use crate::build::SignedMessage;
use crate::cast::{Cast, CastRef, MessageHash, Reaction};
use crate::custody::EthAddress;
use crate::error::{Error, NodeErrorBody};
use crate::profile::{FollowerTally, UserDatum};
use crate::proto::ReactionType;
use crate::signer::Signer;

/// Per shard, so a page holds up to this many times the shard count.
const FOLLOWER_PAGE_SIZE: u32 = 1000;

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
    /// assert!(selfcast_core::Node::new("http://localhost:3381").is_ok());
    /// assert!(selfcast_core::Node::new("localhost:3381").is_err());
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

    /// How many accounts follow `fid`: one request per 1000 followers per
    /// shard, plus one.
    pub async fn follower_count(&self, fid: u64) -> Result<usize, Error> {
        let mut tally = FollowerTally::default();
        let mut request = PageRequest {
            page_size: Some(FOLLOWER_PAGE_SIZE),
            page_token: None,
        };
        loop {
            let page = self.followers_by_fid(fid, &request).await?;
            let fresh = tally.add_page(&page.items);
            match page.next_page_token {
                Some(token) if fresh => request.page_token = Some(token),
                _ => return Ok(tally.count()),
            }
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
