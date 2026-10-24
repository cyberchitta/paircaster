//! A shard's events: what it merged, pruned and revoked, block by block, as
//! the node records them. Pure; the node module fetches the pages.

use crate::Error;
use crate::cast::{Cast, MessageHash, SignerKey, Target};
use crate::proto::ReactionType;
use crate::time::Timestamp;

/// Bits of an event id below the block height (snapchain v0.14.3
/// `src/storage/store/account/event.rs` line 13, `SEQUENCE_BITS`).
const SEQUENCE_BITS: u32 = 14;

/// The id of block `block`'s first event: the height shifted left
/// [`SEQUENCE_BITS`], plus the event's place in the block. Place 0 is the
/// block's confirmation (observed on our node).
pub fn first_event_of(block: u64) -> u64 {
    block << SEQUENCE_BITS
}

/// What happened to a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// The shard stored it.
    Merge,
    /// It was dropped for storage: the account was over its limit.
    Prune,
    /// Its app key was removed.
    Revoke,
}

/// One event other than a block's confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventBody {
    Message {
        change: Change,
        /// The node's message type without its prefix, lowercase:
        /// `cast_add`, `reaction_add`, `link_add`, …
        kind: String,
        fid: u64,
        hash: MessageHash,
        /// The time the message gives itself, not when it was merged.
        timestamp: Timestamp,
        signer: SignerKey,
        /// What the message says, for the kinds read; `None` for the rest,
        /// and for one holding a value this version does not know.
        content: Option<Content>,
    },
    UsernameProof {
        fid: u64,
        name: String,
        /// `fname`, `ens_l1`, `basename`, …
        kind: String,
    },
    OnChain {
        fid: u64,
        /// `id_register`, `signer`, `storage_rent`, …
        kind: String,
    },
    /// An event of a type Paircaster does not read, with the node's name for it.
    Other { node_type: String },
}

/// What a message in an event says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    /// A `cast_add`: the cast.
    Cast(Box<Cast>),
    /// A `reaction_add` or `reaction_remove`: a like or a recast, and what of.
    Reaction { kind: ReactionType, target: Target },
    /// A `link_add` or `link_remove`: the link's type (`follow`) and the
    /// account it points at.
    Link { kind: String, target_fid: u64 },
}

/// An event as a page lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageEvent {
    /// A block's confirmation, which carries the block's time.
    Block { block: u64, time: Timestamp },
    Event {
        id: u64,
        block: u64,
        body: EventBody,
    },
}

/// An event with its shard and the time of the block that holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub shard: u32,
    pub id: u64,
    pub block: u64,
    pub block_time: Timestamp,
    pub body: EventBody,
}

/// A walk over one shard's pages of events, in id order: it gives each
/// event its block's time and keeps those whose block is at or after
/// `since` and before `until`.
#[derive(Debug)]
pub struct EventWalk {
    shard: u32,
    since: Option<Timestamp>,
    until: Option<Timestamp>,
    next_id: Option<u64>,
    block: Option<(u64, Timestamp)>,
}

impl EventWalk {
    pub fn new(
        shard: u32,
        start_id: u64,
        since: Option<Timestamp>,
        until: Option<Timestamp>,
    ) -> Self {
        Self {
            shard,
            since,
            until,
            next_id: Some(start_id),
            block: None,
        }
    }

    pub fn shard(&self) -> u32 {
        self.shard
    }

    /// Where the next page starts; `None` once the walk is over.
    pub fn next_id(&self) -> Option<u64> {
        self.next_id
    }

    /// The events of `page` in the window. An empty page, or a block at or
    /// past `until`, ends the walk. An event whose block's confirmation was
    /// not seen first is an error.
    pub fn add_page(&mut self, page: Vec<PageEvent>) -> Result<Vec<Event>, Error> {
        let mut kept = Vec::new();
        let mut last = None;
        for item in page {
            match item {
                PageEvent::Block { block, time } => {
                    last = Some(first_event_of(block));
                    if self.until.is_some_and(|u| time >= u) {
                        self.next_id = None;
                        return Ok(kept);
                    }
                    self.block = Some((block, time));
                }
                PageEvent::Event { id, block, body } => {
                    last = Some(id);
                    let time = self.time_of(block)?;
                    if self.since.is_none_or(|s| time >= s) {
                        kept.push(self.event(id, block, time, body));
                    }
                }
            }
        }
        self.next_id = last.map(|id| id + 1);
        Ok(kept)
    }

    fn time_of(&self, block: u64) -> Result<Timestamp, Error> {
        match self.block {
            Some((b, time)) if b == block => Ok(time),
            _ => Err(Error::Malformed(format!(
                "event in block {block} before that block's confirmation"
            ))),
        }
    }

    fn event(&self, id: u64, block: u64, block_time: Timestamp, body: EventBody) -> Event {
        Event {
            shard: self.shard,
            id,
            block,
            block_time,
            body,
        }
    }
}
