//! A feed: casts from the accounts one follows, newest first.

use std::cmp::Reverse;

use crate::cast::Cast;
use crate::signer::{Signer, app_of};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedItem {
    pub cast: Cast,
    /// The app whose key signed the cast, per [`app_of`].
    pub app_fid: Option<u64>,
}

/// The newest `limit` of `casts`, newest first; casts at the same second are
/// ordered by hash, so the order is the same on every run.
pub fn assemble(
    casts: impl IntoIterator<Item = Cast>,
    signers: &[Signer],
    limit: usize,
) -> Vec<FeedItem> {
    let mut casts: Vec<Cast> = casts.into_iter().collect();
    casts.sort_by_key(|c| Reverse((c.timestamp, c.id.hash)));
    casts
        .into_iter()
        .take(limit)
        .map(|cast| FeedItem {
            app_fid: app_of(&cast, signers),
            cast,
        })
        .collect()
}
