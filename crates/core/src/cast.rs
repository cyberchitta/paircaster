//! Casts and reactions as Selfcast works with them, past the `node` boundary.

use std::fmt;
use std::str::FromStr;

use crate::proto::{CastType, ReactionType};
use crate::time::Timestamp;

/// A message hash: BLAKE3 of the message data, truncated to 20 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MessageHash([u8; 20]);

impl MessageHash {
    /// `None` unless `bytes` is exactly 20 bytes long.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        bytes.try_into().ok().map(Self)
    }

    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }
}

/// `0x`-prefixed lowercase hex, as the node writes it.
impl fmt::Display for MessageHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

/// Why a hash or cast reference typed by a person did not parse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseRefError {
    #[error("a hash is 0x and 40 hex digits")]
    BadHash,
    #[error("a cast is FID:HASH, for example 3:0x0123…")]
    BadCastRef,
}

/// `0x` and 40 hex digits, as [`Display`](fmt::Display) writes it.
///
/// ```
/// use selfcast_core::cast::MessageHash;
/// let h: MessageHash = "0x68f602dd2b91063e7263366c98f38b0357fe1def".parse().unwrap();
/// assert_eq!(h.to_string(), "0x68f602dd2b91063e7263366c98f38b0357fe1def");
/// assert!("68f602dd2b91063e7263366c98f38b0357fe1def".parse::<MessageHash>().is_err());
/// ```
impl FromStr for MessageHash {
    type Err = ParseRefError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.strip_prefix("0x")
            .and_then(|h| hex::decode(h).ok())
            .and_then(|b| Self::from_bytes(&b))
            .ok_or(ParseRefError::BadHash)
    }
}

/// The public half of an app key (Ed25519), which signs an account's
/// messages. Not a secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SignerKey([u8; 32]);

impl SignerKey {
    /// `None` unless `bytes` is exactly 32 bytes long.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        bytes.try_into().ok().map(Self)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// `0x`-prefixed lowercase hex, as the node writes it.
impl fmt::Display for SignerKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

/// Identifies a cast: its author and its hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CastRef {
    pub fid: u64,
    pub hash: MessageHash,
}

/// `FID:HASH`, the author's FID and the cast's hash.
///
/// ```
/// use selfcast_core::cast::CastRef;
/// let r: CastRef = "576:0x68f602dd2b91063e7263366c98f38b0357fe1def".parse().unwrap();
/// assert_eq!(r.fid, 576);
/// assert_eq!(r.to_string(), "576:0x68f602dd2b91063e7263366c98f38b0357fe1def");
/// assert!("0x68f602dd2b91063e7263366c98f38b0357fe1def".parse::<CastRef>().is_err());
/// ```
impl FromStr for CastRef {
    type Err = ParseRefError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (fid, hash) = s.split_once(':').ok_or(ParseRefError::BadCastRef)?;
        Ok(Self {
            fid: fid.parse().map_err(|_| ParseRefError::BadCastRef)?,
            hash: hash.parse()?,
        })
    }
}

impl fmt::Display for CastRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.fid, self.hash)
    }
}

/// What a cast embeds or replies to, or what a reaction is to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Cast(CastRef),
    Url(String),
}

/// An account mentioned in a cast, at a byte offset into its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mention {
    pub fid: u64,
    pub position: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cast {
    pub id: CastRef,
    pub timestamp: Timestamp,
    pub kind: CastType,
    pub text: String,
    pub mentions: Vec<Mention>,
    pub embeds: Vec<Target>,
    pub parent: Option<Target>,
    pub signer: SignerKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reaction {
    pub fid: u64,
    pub kind: ReactionType,
    pub target: Target,
    pub timestamp: Timestamp,
}
