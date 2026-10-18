use reqwest::{Client, Url};
use serde_json::Value;

use crate::error::{Error, NodeErrorBody};

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

    async fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value, Error> {
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
