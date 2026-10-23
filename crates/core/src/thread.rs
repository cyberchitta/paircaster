//! A thread: the casts one cast replies to, and the replies to it, gathered
//! across the node's pages.

use std::collections::BTreeSet;

use crate::cast::{Cast, CastRef};
use crate::feed::{self, AppFilter, FeedItem};
use crate::signer::{Signer, app_of};
use crate::time::Timestamp;

/// A cast with the casts it replies to and its direct replies, each with the
/// app it was posted from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thread {
    pub top: ChainTop,
    /// Root first.
    pub ancestors: Vec<FeedItem>,
    pub cast: FeedItem,
    /// Oldest first.
    pub replies: Vec<FeedItem>,
}

impl Thread {
    /// Attributes each cast to its app, and keeps the newest `limit` replies,
    /// oldest first, leaving out any dated too far past `now`.
    pub fn assemble(
        above: Ancestors,
        cast: Cast,
        replies: Vec<Cast>,
        signers: &[Signer],
        now: Timestamp,
        limit: usize,
    ) -> Self {
        let item = |cast: Cast| FeedItem {
            app_fid: app_of(&cast, signers),
            cast,
        };
        let mut replies = feed::assemble(replies, signers, &AppFilter::Any, now, limit);
        replies.reverse();
        Thread {
            top: above.top,
            ancestors: above.casts.into_iter().map(item).collect(),
            cast: item(cast),
            replies,
        }
    }

    /// Every cast in the thread, top to bottom.
    pub fn items(&self) -> impl Iterator<Item = &FeedItem> {
        self.ancestors
            .iter()
            .chain([&self.cast])
            .chain(&self.replies)
    }

    /// Each author in the thread and each account its casts mention.
    pub fn account_fids(&self) -> BTreeSet<u64> {
        self.items()
            .flat_map(|i| {
                let mentioned = i.cast.mentions.iter().map(|m| m.fid);
                std::iter::once(i.cast.id.fid).chain(mentioned)
            })
            .collect()
    }
}

/// The casts a cast replies to, root first, and where the chain stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ancestors {
    pub casts: Vec<Cast>,
    pub top: ChainTop,
}

/// Where a chain of parents stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainTop {
    /// At a cast that replies to no cast: the start of the thread.
    Root,
    /// At a parent the node does not hold.
    Missing(CastRef),
    /// At the walk's limit; this parent and those above it were not read.
    Cut(CastRef),
}
