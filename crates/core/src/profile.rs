//! A profile, assembled from an account's user data and follows.

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
}

impl Profile {
    /// Where an account has set one kind of user data more than once, the
    /// newest wins. An empty value counts as unset.
    pub fn assemble(fid: u64, user_data: &[UserDatum], following: usize) -> Self {
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
