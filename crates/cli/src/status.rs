//! `paircaster node status`: whether the node answers, and if so how far
//! behind each shard is. It reads only the node's HTTP API, so it reports a
//! node anywhere, through an SSH tunnel too. What comes before a node
//! answers, the snapshot download and unpack, is seen only beside the node,
//! by `paircaster-node status`. A connection this process is not permitted
//! to open, as under an agent's sandbox, is an error: nothing is inferred
//! from it.

use paircaster_core::Node;
use paircaster_core::status::Synced;
use serde::Serialize;

use crate::CliError;

/// `paircaster node status --json`, version 1: the answering phases of
/// `paircaster-node status`, with the same fields. `disk` is always `null`
/// here; the disk is the node machine's.
#[derive(Serialize)]
struct StatusV1 {
    version: u32,
    #[serde(flatten)]
    phase: Synced,
    disk: (),
}

/// `node status` could not learn the node's state.
#[derive(Debug, thiserror::Error)]
pub enum StatusError {
    #[error(
        "not permitted to connect to {0}, so the node's state is unknown; an agent's sandbox may be blocking network access for paircaster"
    )]
    NetworkDenied(String),
    #[error(
        "no node answers at {0}. If it runs on this machine and is still being set up, `paircaster-node status` there shows how far along it is"
    )]
    NoAnswer(String),
}

pub async fn run(node: &Node, url: &str, json: bool) -> Result<(), CliError> {
    let state = node.sync_state().await.map_err(|e| unanswered(e, url))?;
    let phase = Synced::from_state(state);
    let out = match json {
        true => serde_json::to_string(&StatusV1 {
            version: 1,
            phase,
            disk: (),
        })
        .expect("a status always serialises"),
        false => phase.describe(),
    };
    crate::out::line(out);
    Ok(())
}

fn unanswered(e: paircaster_core::Error, url: &str) -> CliError {
    match e {
        e if e.is_denied() => StatusError::NetworkDenied(url.to_owned()).into(),
        paircaster_core::Error::Transport(_) => StatusError::NoAnswer(url.to_owned()).into(),
        other => other.into(),
    }
}
