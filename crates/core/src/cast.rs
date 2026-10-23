//! Casts and reactions as Paircaster works with them, past the `node` boundary.

use std::collections::HashMap;
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
    #[error("a channel is the URL its casts reply to, 1 to 256 bytes")]
    BadChannelUrl,
}

/// A channel, as the URL its casts name as their parent, for example
/// `https://warpcast.com/~/channel/fc-updates`. Snapchain checks only the length
/// of a parent URL, 1 to 256 bytes (v0.14.3 `src/core/validations/mod.rs`,
/// `validate_url`), so any such text is one; whether anyone posted there
/// is the node's to say.
///
/// ```
/// use paircaster_core::cast::ChannelUrl;
/// assert!("https://warpcast.com/~/channel/fc-updates".parse::<ChannelUrl>().is_ok());
/// assert!("".parse::<ChannelUrl>().is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelUrl(String);

impl ChannelUrl {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for ChannelUrl {
    type Err = ParseRefError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match (1..=256).contains(&s.len()) {
            true => Ok(Self(s.to_owned())),
            false => Err(ParseRefError::BadChannelUrl),
        }
    }
}

impl fmt::Display for ChannelUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// `0x` and 40 hex digits, as [`Display`](fmt::Display) writes it.
///
/// ```
/// use paircaster_core::cast::MessageHash;
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
/// use paircaster_core::cast::CastRef;
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

impl Cast {
    /// The cast this one replies to; `None` for a cast that starts a thread,
    /// in a channel (a URL parent) or not.
    pub fn parent_cast(&self) -> Option<CastRef> {
        match self.parent {
            Some(Target::Cast(r)) => Some(r),
            Some(Target::Url(_)) | None => None,
        }
    }

    /// The text with each mention written in where it was made: `@username`,
    /// or `@fid:N` for an account `names` has no username for. `None` when a
    /// position falls inside a character, which the network allows.
    /// Positions are in bounds and ascending, as parsing checked.
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use paircaster_core::cast::{Cast, CastRef, Mention, MessageHash, SignerKey};
    /// use paircaster_core::proto::CastType;
    /// use paircaster_core::time::Timestamp;
    /// let cast = |text: &str, positions: &[u32]| Cast {
    ///     id: CastRef { fid: 1, hash: MessageHash::from_bytes(&[0; 20]).unwrap() },
    ///     timestamp: Timestamp::from_farcaster(0),
    ///     kind: CastType::Cast,
    ///     text: text.to_owned(),
    ///     mentions: positions.iter().zip([3, 4]).map(|(&position, fid)| Mention { fid, position }).collect(),
    ///     embeds: vec![],
    ///     parent: None,
    ///     signer: SignerKey::from_bytes(&[1; 32]).unwrap(),
    /// };
    /// let names = HashMap::from([(3, "dwr".to_owned())]);
    /// assert_eq!(cast("hi  and ", &[3, 8]).display_text(&names).unwrap(), "hi @dwr and @fid:4");
    /// assert_eq!(cast("é", &[1]).display_text(&names), None);
    /// ```
    pub fn display_text(&self, names: &HashMap<u64, String>) -> Option<String> {
        let mut out = String::with_capacity(self.text.len());
        let mut from = 0;
        for m in &self.mentions {
            let at = m.position as usize;
            out.push_str(self.text.get(from..at)?);
            match names.get(&m.fid) {
                Some(name) => out.push_str(&format!("@{name}")),
                None => out.push_str(&format!("@fid:{}", m.fid)),
            }
            from = at;
        }
        out.push_str(self.text.get(from..)?);
        Some(out)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reaction {
    pub fid: u64,
    pub kind: ReactionType,
    pub target: Target,
    pub timestamp: Timestamp,
}
