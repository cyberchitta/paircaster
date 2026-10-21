//! A profile, assembled from an account's user data, follows and followers.

use std::collections::HashSet;

use crate::proto::UserDataType;
use crate::time::Timestamp;

/// One piece of an account's user data, e.g. its username.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserDatum {
    pub kind: UserDataType,
    pub value: String,
    pub timestamp: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub fid: u64,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub pfp_url: Option<String>,
    pub url: Option<String>,
    pub location: Option<String>,
    pub twitter: Option<String>,
    pub github: Option<String>,
    pub banner_url: Option<String>,
    /// How many accounts this one follows.
    pub following: usize,
    /// How many accounts follow this one.
    pub followers: Followers,
}

/// Followers counted before `profile` stops and reports a lower bound.
/// Counting is one request per 1000 per shard, in sequence, and a large
/// account has hundreds of thousands (2026-10-21: FID 37 took over 120 s).
pub const FOLLOWER_COUNT_CAP: usize = 10_000;

/// How many accounts follow one: exact, or at least this many when counting
/// stopped at a cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Followers {
    Exact(usize),
    AtLeast(usize),
}

impl Followers {
    pub fn count(self) -> usize {
        match self {
            Followers::Exact(n) | Followers::AtLeast(n) => n,
        }
    }

    pub fn is_exact(self) -> bool {
        matches!(self, Followers::Exact(_))
    }
}

impl Profile {
    /// Where an account has set one kind of user data more than once, the
    /// newest wins. An empty value counts as unset.
    pub fn assemble(
        fid: u64,
        user_data: &[UserDatum],
        following: usize,
        followers: Followers,
    ) -> Self {
        let get = |kind| latest(user_data, kind);
        Profile {
            fid,
            username: get(UserDataType::Username),
            display_name: get(UserDataType::Display),
            bio: get(UserDataType::Bio),
            pfp_url: get(UserDataType::Pfp),
            url: get(UserDataType::Url),
            location: get(UserDataType::Location),
            twitter: get(UserDataType::Twitter),
            github: get(UserDataType::Github),
            banner_url: get(UserDataType::Banner),
            following,
            followers,
        }
    }
}

/// The distinct accounts seen across pages of `linksByTargetFid`.
///
/// That endpoint pages each shard separately and cannot mark one shard as
/// finished: a shard that runs out restarts from its first follower on the
/// next page (snapchain v0.14.3 `src/network/server.rs` `get_links_by_target`,
/// lines 3396-3440). So pages repeat, and its page token never ends. Each
/// follower lives on one shard and follows once, so a shard still reading
/// always brings someone new; the first page that brings no one means every
/// shard has been read.
///
/// ```
/// use selfcast_core::profile::FollowerTally;
/// let mut tally = FollowerTally::default();
/// assert!(tally.add_page(&[5, 6]));
/// assert!(tally.add_page(&[7, 5]));
/// assert!(!tally.add_page(&[5, 6]));
/// assert_eq!(tally.count(), 3);
/// ```
#[derive(Debug, Clone, Default)]
pub struct FollowerTally {
    fids: HashSet<u64>,
}

impl FollowerTally {
    /// Adds one page's follower FIDs; `false` when none of them is new.
    pub fn add_page(&mut self, fids: &[u64]) -> bool {
        let before = self.fids.len();
        self.fids.extend(fids);
        self.fids.len() > before
    }

    pub fn count(&self) -> usize {
        self.fids.len()
    }

    /// Adds one page and says whether counting is done: exact once a page
    /// brings no one new or there is no next page, at least the count once
    /// `cap` followers have been seen.
    ///
    /// ```
    /// use selfcast_core::profile::{FollowerTally, Followers};
    /// let mut tally = FollowerTally::default();
    /// assert_eq!(tally.step(&[1, 2], true, Some(3)), None);
    /// assert_eq!(tally.step(&[3, 4], true, Some(3)), Some(Followers::AtLeast(4)));
    /// ```
    pub fn step(&mut self, fids: &[u64], more: bool, cap: Option<usize>) -> Option<Followers> {
        let fresh = self.add_page(fids);
        let count = self.count();
        match (fresh && more, cap) {
            (false, _) => Some(Followers::Exact(count)),
            (true, Some(c)) if count >= c => Some(Followers::AtLeast(count)),
            (true, _) => None,
        }
    }
}

fn latest(user_data: &[UserDatum], kind: UserDataType) -> Option<String> {
    user_data
        .iter()
        .filter(|d| d.kind == kind)
        .max_by_key(|d| d.timestamp)
        .map(|d| d.value.clone())
        .filter(|v| !v.is_empty())
}
