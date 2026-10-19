//! Casts and reactions as Selfcast works with them, past the `node` boundary.

use std::fmt;

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
