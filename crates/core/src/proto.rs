//! Hand-written protobuf structs for the Farcaster messages Selfcast reads and
//! writes.
//!
//! Each type mirrors `proto/definitions/message.proto` in snapchain v0.14.3
//! (GPL-3.0, so neither vendored nor generated from); the line cited on each
//! item is in that file. Only the messages and bodies Selfcast uses are here.
//! `prost` encodes fields in tag order, the order snapchain's own
//! `encode_to_vec` uses when it hashes a message without `data_bytes`.
//!
//! Every enum keeps its proto zero value as its first variant: `prost` takes
//! the first variant as a field's default and does not encode the default,
//! so leaving it out drops a real value from the hashed bytes. The zero
//! values cannot be read from the node's JSON.
//!
//! Timestamps here are the raw protocol value, seconds since 2021-01-01 UTC:
//! these are the wire format, not the types the rest of Selfcast works with.

mod cast_add;
pub mod json;

use serde::Deserialize;

/// The signed envelope around [`MessageData`]. `message.proto:10`.
#[derive(Clone, PartialEq, prost::Message)]
pub struct Message {
    #[prost(message, optional, tag = "1")]
    pub data: Option<MessageData>,
    #[prost(bytes = "vec", tag = "2")]
    pub hash: Vec<u8>,
    #[prost(enumeration = "HashScheme", tag = "3")]
    pub hash_scheme: i32,
    #[prost(bytes = "vec", tag = "4")]
    pub signature: Vec<u8>,
    #[prost(enumeration = "SignatureScheme", tag = "5")]
    pub signature_scheme: i32,
    #[prost(bytes = "vec", tag = "6")]
    pub signer: Vec<u8>,
    /// `MessageData` exactly as hashed and signed, when the sender kept it.
    #[prost(bytes = "vec", optional, tag = "7")]
    pub data_bytes: Option<Vec<u8>>,
}

/// What is hashed and signed. `message.proto:24`.
#[derive(Clone, PartialEq, prost::Message)]
pub struct MessageData {
    #[prost(enumeration = "MessageType", tag = "1")]
    pub r#type: i32,
    #[prost(uint64, tag = "2")]
    pub fid: u64,
    /// Seconds since 2021-01-01 UTC.
    #[prost(uint32, tag = "3")]
    pub timestamp: u32,
    #[prost(enumeration = "FarcasterNetwork", tag = "4")]
    pub network: i32,
    #[prost(oneof = "Body", tags = "5, 6, 7, 12, 14, 19")]
    pub body: Option<Body>,
}

/// The `body` oneof of `MessageData`, limited to the bodies Selfcast uses.
/// `message.proto:29`.
#[derive(Clone, PartialEq, prost::Oneof)]
pub enum Body {
    #[prost(message, tag = "5")]
    CastAdd(CastAddBody),
    #[prost(message, tag = "6")]
    CastRemove(CastRemoveBody),
    #[prost(message, tag = "7")]
    Reaction(ReactionBody),
    #[prost(message, tag = "12")]
    UserData(UserDataBody),
    #[prost(message, tag = "14")]
    Link(LinkBody),
    #[prost(message, tag = "19")]
    KeyAdd(KeyAddBody),
}

/// `message.proto:56`.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration, Deserialize,
)]
#[repr(i32)]
pub enum HashScheme {
    #[serde(skip)]
    None = 0,
    #[serde(rename = "HASH_SCHEME_BLAKE3")]
    Blake3 = 1,
}

/// `message.proto:62`.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration, Deserialize,
)]
#[repr(i32)]
pub enum SignatureScheme {
    #[serde(skip)]
    None = 0,
    #[serde(rename = "SIGNATURE_SCHEME_ED25519")]
    Ed25519 = 1,
}

/// The message types Selfcast reads and writes. `message.proto:69`.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration, Deserialize,
)]
#[repr(i32)]
pub enum MessageType {
    #[serde(skip)]
    None = 0,
    #[serde(rename = "MESSAGE_TYPE_CAST_ADD")]
    CastAdd = 1,
    #[serde(rename = "MESSAGE_TYPE_CAST_REMOVE")]
    CastRemove = 2,
    #[serde(rename = "MESSAGE_TYPE_REACTION_ADD")]
    ReactionAdd = 3,
    #[serde(rename = "MESSAGE_TYPE_REACTION_REMOVE")]
    ReactionRemove = 4,
    #[serde(rename = "MESSAGE_TYPE_LINK_ADD")]
    LinkAdd = 5,
    #[serde(rename = "MESSAGE_TYPE_LINK_REMOVE")]
    LinkRemove = 6,
    #[serde(rename = "MESSAGE_TYPE_USER_DATA_ADD")]
    UserDataAdd = 11,
    #[serde(rename = "MESSAGE_TYPE_KEY_ADD")]
    KeyAdd = 16,
}

/// `message.proto:96`.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration, Deserialize,
)]
#[repr(i32)]
pub enum FarcasterNetwork {
    #[serde(skip)]
    None = 0,
    #[serde(rename = "FARCASTER_NETWORK_MAINNET")]
    Mainnet = 1,
    #[serde(rename = "FARCASTER_NETWORK_TESTNET")]
    Testnet = 2,
    #[serde(rename = "FARCASTER_NETWORK_DEVNET")]
    Devnet = 3,
}

/// `message.proto:104`.
#[derive(Clone, PartialEq, prost::Message)]
pub struct UserDataBody {
    #[prost(enumeration = "UserDataType", tag = "1")]
    pub r#type: i32,
    #[prost(string, tag = "2")]
    pub value: String,
}

/// `message.proto:110`.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration, Deserialize,
)]
#[repr(i32)]
pub enum UserDataType {
    #[serde(skip)]
    None = 0,
    #[serde(rename = "USER_DATA_TYPE_PFP")]
    Pfp = 1,
    #[serde(rename = "USER_DATA_TYPE_DISPLAY")]
    Display = 2,
    #[serde(rename = "USER_DATA_TYPE_BIO")]
    Bio = 3,
    #[serde(rename = "USER_DATA_TYPE_URL")]
    Url = 5,
    #[serde(rename = "USER_DATA_TYPE_USERNAME")]
    Username = 6,
    #[serde(rename = "USER_DATA_TYPE_LOCATION")]
    Location = 7,
    #[serde(rename = "USER_DATA_TYPE_TWITTER")]
    Twitter = 8,
    #[serde(rename = "USER_DATA_TYPE_GITHUB")]
    Github = 9,
    #[serde(rename = "USER_DATA_TYPE_BANNER")]
    Banner = 10,
    #[serde(rename = "USER_DATA_PRIMARY_ADDRESS_ETHEREUM")]
    PrimaryAddressEthereum = 11,
    #[serde(rename = "USER_DATA_PRIMARY_ADDRESS_SOLANA")]
    PrimaryAddressSolana = 12,
    #[serde(rename = "USER_DATA_TYPE_PROFILE_TOKEN")]
    ProfileToken = 13,
    #[serde(rename = "USER_DATA_TYPE_LIVE_AT")]
    LiveAt = 14,
}

/// `message.proto:127`.
#[derive(Clone, PartialEq, prost::Message)]
pub struct Embed {
    #[prost(oneof = "EmbedTarget", tags = "1, 2")]
    pub embed: Option<EmbedTarget>,
}

/// The `embed` oneof of `Embed`. `message.proto:128`.
#[derive(Clone, PartialEq, prost::Oneof)]
pub enum EmbedTarget {
    #[prost(string, tag = "1")]
    Url(String),
    #[prost(message, tag = "2")]
    CastId(CastId),
}

/// `message.proto:135`.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration, Deserialize,
)]
#[repr(i32)]
pub enum CastType {
    #[serde(rename = "CAST")]
    Cast = 0,
    #[serde(rename = "LONG_CAST")]
    LongCast = 1,
    #[serde(rename = "TEN_K_CAST")]
    TenKCast = 2,
}

/// `message.proto:143`.
///
/// Its encoding is written by hand (in `cast_add`) rather than derived:
/// clients built on `@farcaster/hub-nodejs` (ts-proto) write `mentions` and
/// `mentions_positions` even when empty, as a zero-length packed field, and
/// sign those bytes. Canonical proto3, as `prost` derives it, leaves them out
/// and so cannot reproduce the hash of most casts on the network.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct CastAddBody {
    pub embeds_deprecated: Vec<String>,
    pub mentions: Vec<u64>,
    pub parent: Option<CastParent>,
    pub text: String,
    pub mentions_positions: Vec<u32>,
    pub embeds: Vec<Embed>,
    /// A [`CastType`].
    pub r#type: i32,
}

/// The `parent` oneof of `CastAddBody`. `message.proto:146`.
#[derive(Clone, PartialEq, prost::Oneof)]
pub enum CastParent {
    #[prost(message, tag = "3")]
    CastId(CastId),
    #[prost(string, tag = "7")]
    Url(String),
}

/// `message.proto:157`.
#[derive(Clone, PartialEq, prost::Message)]
pub struct CastRemoveBody {
    #[prost(bytes = "vec", tag = "1")]
    pub target_hash: Vec<u8>,
}

/// `message.proto:162`.
#[derive(Clone, PartialEq, prost::Message)]
pub struct CastId {
    #[prost(uint64, tag = "1")]
    pub fid: u64,
    #[prost(bytes = "vec", tag = "2")]
    pub hash: Vec<u8>,
}

/// `message.proto:168`.
#[derive(Clone, PartialEq, prost::Message)]
pub struct ReactionBody {
    #[prost(enumeration = "ReactionType", tag = "1")]
    pub r#type: i32,
    #[prost(oneof = "ReactionTarget", tags = "2, 3")]
    pub target: Option<ReactionTarget>,
}

/// The `target` oneof of `ReactionBody`. `message.proto:170`.
#[derive(Clone, PartialEq, prost::Oneof)]
pub enum ReactionTarget {
    #[prost(message, tag = "2")]
    CastId(CastId),
    #[prost(string, tag = "3")]
    Url(String),
}

/// `message.proto:177`.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration, Deserialize,
)]
#[repr(i32)]
pub enum ReactionType {
    #[serde(skip)]
    None = 0,
    #[serde(rename = "REACTION_TYPE_LIKE")]
    Like = 1,
    #[serde(rename = "REACTION_TYPE_RECAST")]
    Recast = 2,
}

/// `message.proto:206`.
#[derive(Clone, PartialEq, prost::Message)]
pub struct LinkBody {
    /// At most 8 characters, e.g. `follow`.
    #[prost(string, tag = "1")]
    pub r#type: String,
    #[prost(uint32, optional, tag = "2")]
    pub display_timestamp: Option<u32>,
    #[prost(oneof = "LinkTarget", tags = "3")]
    pub target: Option<LinkTarget>,
}

/// The `target` oneof of `LinkBody`. `message.proto:209`.
#[derive(Clone, PartialEq, prost::Oneof)]
pub enum LinkTarget {
    #[prost(uint64, tag = "3")]
    Fid(u64),
}

/// Registers an app key, authorised by the account's custody key: the
/// gasless `KEY_ADD`. `message.proto:244`.
#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct KeyAddBody {
    /// The Ed25519 public key being added.
    #[prost(bytes = "vec", tag = "1")]
    pub key: Vec<u8>,
    /// 1 for Ed25519, the only type defined.
    #[prost(uint32, tag = "2")]
    pub key_type: u32,
    /// EIP-712 `KeyAdd` signature by the custody key, 65 bytes.
    #[prost(bytes = "vec", tag = "3")]
    pub custody_signature: Vec<u8>,
    /// Seconds since 2021-01-01 UTC after which the custody signature lapses.
    #[prost(uint32, tag = "4")]
    pub deadline: u32,
    /// Must exceed the account's `currentUserNonce`.
    #[prost(uint32, tag = "5")]
    pub nonce: u32,
    /// ABI-encoded `SignedKeyRequestMetadata`.
    #[prost(bytes = "vec", tag = "6")]
    pub metadata: Vec<u8>,
    /// 1 for `SignedKeyRequest`, the only type defined.
    #[prost(uint32, tag = "7")]
    pub metadata_type: u32,
    #[prost(bytes = "vec", tag = "8")]
    pub registration_tx_hash: Vec<u8>,
    /// The [`MessageType`]s the key may sign.
    #[prost(int32, repeated, tag = "9")]
    pub scopes: Vec<i32>,
    /// Sliding expiry in seconds, refreshed by use; at most 90 days.
    #[prost(uint32, tag = "10")]
    pub ttl: u32,
}
