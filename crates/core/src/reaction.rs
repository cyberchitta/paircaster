//! The reactions to one cast: who liked it and who recast it.

use std::collections::BTreeSet;

use crate::cast::Reaction;

/// The likes and recasts of one cast, each oldest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastReactions {
    pub likes: Vec<Reaction>,
    pub recasts: Vec<Reaction>,
}

impl CastReactions {
    /// Puts each kind oldest first.
    pub fn assemble(likes: Vec<Reaction>, recasts: Vec<Reaction>) -> Self {
        Self {
            likes: oldest_first(likes),
            recasts: oldest_first(recasts),
        }
    }

    /// Every account that liked or recast, once.
    pub fn account_fids(&self) -> BTreeSet<u64> {
        self.likes
            .iter()
            .chain(&self.recasts)
            .map(|r| r.fid)
            .collect()
    }
}

fn oldest_first(mut reactions: Vec<Reaction>) -> Vec<Reaction> {
    reactions.sort_by_key(|r| (r.timestamp, r.fid));
    reactions
}
