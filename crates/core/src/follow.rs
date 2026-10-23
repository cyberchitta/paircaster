//! Follows as lists: who an account follows, and who follows it.

use std::cmp::Reverse;

use crate::time::Timestamp;

/// One follow: the account at the other end, and when the follow was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Follow {
    pub fid: u64,
    pub timestamp: Timestamp,
}

/// The newest `limit` of `follows`, newest first; follows made in the same
/// second are ordered by FID, so the order is the same on every run.
///
/// ```
/// use paircaster_core::follow::{Follow, newest};
/// use paircaster_core::time::Timestamp;
/// let at = |fid, s| Follow { fid, timestamp: Timestamp::from_unix(s) };
/// let n = newest(vec![at(1, 10), at(2, 30), at(3, 20)], 2);
/// assert_eq!(n, [at(2, 30), at(3, 20)]);
/// ```
pub fn newest(mut follows: Vec<Follow>, limit: usize) -> Vec<Follow> {
    follows.sort_by_key(|f| (Reverse(f.timestamp), f.fid));
    follows.truncate(limit);
    follows
}
