pub mod parse;

use reqwest::{Client, Url};
use serde_json::Value;

use crate::cast::{Cast, MessageHash, Reaction};
use crate::error::{Error, NodeErrorBody};
use crate::profile::UserDatum;
use crate::proto::ReactionType;
use crate::signer::Signer;

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

    /// `GET /v1/reactionsByFid` for one kind of reaction.
    pub async fn reactions_by_fid(
        &self,
        fid: u64,
        kind: ReactionType,
        page: &PageRequest,
    ) -> Result<Page<Reaction>, Error> {
        let kind = match kind {
            ReactionType::Like => "Like",
            ReactionType::Recast => "Recast",
            ReactionType::None => return Err(Error::Malformed("reaction type NONE".to_owned())),
        };
        let query = with_page(&[fid_param(fid), ("reaction_type", kind.to_owned())], page);
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
