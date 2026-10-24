use serde::Deserialize;

use crate::account::Username;

/// Errors from talking to a Snapchain node or reading what it returns.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("malformed response from node: {0}")]
    Malformed(String),
    /// A value this version of Paircaster does not know, such as a message
    /// type a newer Snapchain added. Reads of a page skip the message it is
    /// in; a read of that one message fails with it.
    #[error("the node sent {0}, which this version of paircaster does not read")]
    Unknown(String),
    #[error("invalid node URL {url:?}: {reason}")]
    BadNodeUrl { url: String, reason: String },
    #[error("could not reach node: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("node returned HTTP {status}: {error}: {detail}")]
    Node {
        status: u16,
        error: String,
        detail: String,
    },
    #[error("node returned HTTP {status} with an unrecognised body: {body}")]
    UnexpectedResponse { status: u16, body: String },
    #[error("FID {0} is not registered on this node; a node still syncing may not have it yet")]
    NotRegistered(u64),
    #[error(
        "no account holds the username {0} on this node; a node still syncing may not have it yet"
    )]
    UnknownName(Username),
    #[error("the node has no shard {0} with events; `paircaster node status` lists its shards")]
    NoShard(u32),
}

/// The JSON error body Snapchain returns for failed lookups. Query-parsing
/// errors come back as plain text instead and land in `UnexpectedResponse`.
#[derive(Debug, Deserialize)]
pub(crate) struct NodeErrorBody {
    pub error: String,
    pub error_detail: String,
}
