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
    pub followers: usize,
}

impl Profile {
    /// Where an account has set one kind of user data more than once, the
    /// newest wins. An empty value counts as unset.
    pub fn assemble(fid: u64, user_data: &[UserDatum], following: usize, followers: usize) -> Self {
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
}

fn latest(user_data: &[UserDatum], kind: UserDataType) -> Option<String> {
    user_data
        .iter()
        .filter(|d| d.kind == kind)
        .max_by_key(|d| d.timestamp)
        .map(|d| d.value.clone())
        .filter(|v| !v.is_empty())
}
