//! A thread: the casts one cast replies to, and the replies to it, gathered
//! across the node's pages.

use std::collections::{BTreeSet, HashSet};

use crate::cast::{Cast, CastRef, MessageHash};
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

/// Replies gathered from `castsByParent` pages, each kept once.
///
/// The node's pages repeat. A page holds replies from every message shard,
/// and a shard that has run out starts again from its first reply on the
/// next page; once all have run out, the next page is the first one again
/// (snapchain v0.14.3). So a reply can arrive on several pages, and the
/// thread is complete at the first page that brings no new reply.
///
/// ```
/// use selfcast_core::thread::ReplyTally;
/// let mut tally = ReplyTally::default();
/// assert!(!tally.add_page(vec![]));
/// assert!(tally.into_casts().is_empty());
/// ```
#[derive(Debug, Clone, Default)]
pub struct ReplyTally {
    seen: HashSet<MessageHash>,
    casts: Vec<Cast>,
}

impl ReplyTally {
    /// Adds one page's replies, skipping any already seen; `false` when none
    /// of them is new.
    pub fn add_page(&mut self, page: Vec<Cast>) -> bool {
        let before = self.casts.len();
        let fresh = page.into_iter().filter(|c| self.seen.insert(c.id.hash));
        self.casts.extend(fresh.collect::<Vec<_>>());
        self.casts.len() > before
    }

    /// Every reply seen, in the order the pages gave them.
    pub fn into_casts(self) -> Vec<Cast> {
        self.casts
    }
}
