//! Accounts as a person names them: by FID or by username.

use std::fmt;
use std::str::FromStr;

/// A Farcaster username: an fname (`dwr`), an ENS name (`vitalik.eth`) or
/// a basename (`jesse.base.eth`). Each dot-separated part matches
/// snapchain's fname pattern `^[a-z0-9][a-z0-9-]{0,15}$` (v0.14.3
/// `src/core/validations/message.rs` line 26), and a name of several parts
/// ends in `.eth`. Whether anyone holds the name is the node's to say.
///
/// ```
/// use selfcast_core::account::Username;
/// assert!("vitalik.eth".parse::<Username>().is_ok());
/// assert!("Vitalik.eth".parse::<Username>().is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Username(String);

impl Username {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Username {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Username {
    type Err = ParseAccountError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('.').collect();
        let ends_well = parts.len() == 1 || parts.last() == Some(&"eth");
        match ends_well && parts.iter().all(|p| is_name_part(p)) {
            true => Ok(Self(s.to_owned())),
            false => Err(ParseAccountError::BadName(s.to_owned())),
        }
    }
}

fn is_name_part(part: &str) -> bool {
    let allowed = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit();
    part.len() <= 16 && part.starts_with(allowed) && part.chars().all(|c| allowed(c) || c == '-')
}

/// An account as typed: a FID, or a username to look up on the node.
/// Digits alone are a FID; `@` before a name is optional, and is how a
/// name made only of digits is written.
///
/// ```
/// use selfcast_core::account::Account;
/// assert_eq!("3".parse::<Account>().unwrap(), Account::Fid(3));
/// assert!(matches!("@3".parse::<Account>().unwrap(), Account::Name(_)));
/// assert!(matches!("dwr".parse::<Account>().unwrap(), Account::Name(_)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Account {
    Fid(u64),
    Name(Username),
}

impl FromStr for Account {
    type Err = ParseAccountError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.strip_prefix('@') {
            Some(name) => name.parse().map(Self::Name),
            None if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) => s
                .parse()
                .map(Self::Fid)
                .map_err(|_| ParseAccountError::BadFid(s.to_owned())),
            None => s.parse().map(Self::Name),
        }
    }
}

/// Why an account typed by a person did not parse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseAccountError {
    #[error(
        "{0:?} is not a FID or a username: usernames are lowercase letters, digits and hyphens, as in dwr, vitalik.eth or jesse.base.eth"
    )]
    BadName(String),
    #[error("{0} is too large for a FID")]
    BadFid(String),
}
