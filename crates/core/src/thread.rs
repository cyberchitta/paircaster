//! A thread: the replies to one cast, gathered across the node's pages.

use std::collections::HashSet;

use crate::cast::{Cast, MessageHash};

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
