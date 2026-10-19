//! Builds and signs messages with an app key.
//!
//! `MessageData` is encoded once. Those bytes are hashed, the hash is signed,
//! and the same bytes go out as `data_bytes`, so nothing is re-encoded
//! between signing and sending. Hashing and signing follow
//! [`crate::message`]; the node decides whether a message is valid.

use ed25519_dalek::{Signer as _, SigningKey};
use prost::Message as _;

use crate::cast::{Mention, MessageHash, SignerKey, Target};
use crate::message::{VerifyError, hash_data, verify};
use crate::proto::{
    Body, CastAddBody, CastId, CastParent, CastRemoveBody, CastType, Embed, EmbedTarget,
    FarcasterNetwork, HashScheme, LinkBody, LinkTarget, Message, MessageData, MessageType,
    ReactionBody, ReactionTarget, ReactionType, SignatureScheme,
};
use crate::time::Timestamp;

/// An app key's secret half (Ed25519). It can only post messages as its
/// account. Neither printable nor loggable; zeroed on drop.
pub struct AppKey(SigningKey);

impl AppKey {
    /// The caller zeroes its own copy of `secret`.
    pub fn from_secret(secret: &[u8; 32]) -> Self {
        Self(SigningKey::from_bytes(secret))
    }

    pub fn public(&self) -> SignerKey {
        SignerKey::from_bytes(self.0.verifying_key().as_bytes()).expect("Ed25519 keys are 32 bytes")
    }
}

/// Who sends a message, where, and when.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Envelope {
    pub fid: u64,
    pub network: FarcasterNetwork,
    pub timestamp: Timestamp,
}

/// A cast to publish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCast {
    pub text: String,
    pub mentions: Vec<Mention>,
    pub embeds: Vec<Target>,
    pub parent: Option<Target>,
    pub kind: CastType,
}

/// What a message does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    CastAdd(NewCast),
    CastRemove(MessageHash),
    ReactionAdd(ReactionType, Target),
    ReactionRemove(ReactionType, Target),
    /// Follow an account (a `follow` link).
    FollowAdd(u64),
    FollowRemove(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BuildError {
    #[error("timestamp is before the Farcaster epoch or past 2157")]
    TimestampOutOfRange,
    #[error("network must be mainnet, testnet or devnet")]
    NoNetwork,
    #[error("reaction must be a like or a recast")]
    NoReactionType,
}

/// A signed message, ready to send. `data_bytes` are the bytes that were
/// hashed and signed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedMessage {
    data_bytes: Vec<u8>,
    hash: MessageHash,
    signature: [u8; 64],
    signer: SignerKey,
}

impl SignedMessage {
    pub fn data_bytes(&self) -> &[u8] {
        &self.data_bytes
    }

    pub fn hash(&self) -> MessageHash {
        self.hash
    }

    pub fn signature(&self) -> &[u8; 64] {
        &self.signature
    }

    pub fn signer(&self) -> SignerKey {
        self.signer
    }

    /// The envelope, carrying `data_bytes` and no decoded `data`, so the
    /// node hashes exactly what was signed.
    pub fn to_message(&self) -> Message {
        Message {
            data: None,
            hash: self.hash.as_bytes().to_vec(),
            hash_scheme: HashScheme::Blake3.into(),
            signature: self.signature.to_vec(),
            signature_scheme: SignatureScheme::Ed25519.into(),
            signer: self.signer.as_bytes().to_vec(),
            data_bytes: Some(self.data_bytes.clone()),
        }
    }

    /// The protobuf `Message` a node's `submitMessage` takes.
    pub fn encode(&self) -> Vec<u8> {
        self.to_message().encode_to_vec()
    }

    /// Reads back what [`encode`](Self::encode) wrote, such as a message saved
    /// before it was sent. A retry resends these bytes rather than signing
    /// again: signing again would make a second message with a new hash.
    ///
    /// Accepts only bytes whose hash and signature verify and which
    /// `encode` reproduces exactly, so what goes out is what was signed.
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let message = Message::decode(bytes).map_err(|_| DecodeError::NotAMessage)?;
        verify(&message).map_err(DecodeError::Unverified)?;
        let signed = Self {
            data_bytes: message.data_bytes.ok_or(DecodeError::NotAsEncoded)?,
            hash: MessageHash::from_bytes(&message.hash).ok_or(DecodeError::NotAsEncoded)?,
            signature: message.signature[..]
                .try_into()
                .map_err(|_| DecodeError::NotAsEncoded)?,
            signer: SignerKey::from_bytes(&message.signer).ok_or(DecodeError::NotAsEncoded)?,
        };
        (signed.encode() == bytes)
            .then_some(signed)
            .ok_or(DecodeError::NotAsEncoded)
    }
}

/// Why saved bytes cannot be resent as they are.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    #[error("not a protobuf Message")]
    NotAMessage,
    #[error("message does not verify: {0}")]
    Unverified(VerifyError),
    #[error("message is not in the form SignedMessage::encode writes")]
    NotAsEncoded,
}

/// Builds `action` as `envelope.fid` would send it, and signs it with `key`.
pub fn sign(
    key: &AppKey,
    envelope: &Envelope,
    action: &Action,
) -> Result<SignedMessage, BuildError> {
    let data_bytes = message_data(envelope, action)?.encode_to_vec();
    let hash = MessageHash::from_bytes(&hash_data(&data_bytes)).expect("hash_data gives 20 bytes");
    let signature = key.0.sign(hash.as_bytes()).to_bytes();
    Ok(SignedMessage {
        data_bytes,
        hash,
        signature,
        signer: key.public(),
    })
}

fn message_data(envelope: &Envelope, action: &Action) -> Result<MessageData, BuildError> {
    if envelope.network == FarcasterNetwork::None {
        return Err(BuildError::NoNetwork);
    }
    let timestamp = envelope
        .timestamp
        .to_farcaster()
        .ok_or(BuildError::TimestampOutOfRange)?;
    let (message_type, body) = body(action)?;
    Ok(MessageData {
        r#type: message_type.into(),
        fid: envelope.fid,
        timestamp,
        network: envelope.network.into(),
        body: Some(body),
    })
}

fn body(action: &Action) -> Result<(MessageType, Body), BuildError> {
    Ok(match action {
        Action::CastAdd(cast) => (MessageType::CastAdd, Body::CastAdd(cast_add(cast))),
        Action::CastRemove(hash) => (
            MessageType::CastRemove,
            Body::CastRemove(CastRemoveBody {
                target_hash: hash.as_bytes().to_vec(),
            }),
        ),
        Action::ReactionAdd(kind, target) => (MessageType::ReactionAdd, reaction(*kind, target)?),
        Action::ReactionRemove(kind, target) => {
            (MessageType::ReactionRemove, reaction(*kind, target)?)
        }
        Action::FollowAdd(fid) => (MessageType::LinkAdd, follow(*fid)),
        Action::FollowRemove(fid) => (MessageType::LinkRemove, follow(*fid)),
    })
}

fn cast_add(cast: &NewCast) -> CastAddBody {
    CastAddBody {
        embeds_deprecated: Vec::new(),
        mentions: cast.mentions.iter().map(|m| m.fid).collect(),
        parent: cast.parent.as_ref().map(|p| match p {
            Target::Cast(r) => CastParent::CastId(cast_id(r.fid, r.hash)),
            Target::Url(url) => CastParent::Url(url.clone()),
        }),
        text: cast.text.clone(),
        mentions_positions: cast.mentions.iter().map(|m| m.position).collect(),
        embeds: cast.embeds.iter().map(embed).collect(),
        r#type: cast.kind.into(),
    }
}

fn embed(target: &Target) -> Embed {
    let embed = match target {
        Target::Cast(r) => EmbedTarget::CastId(cast_id(r.fid, r.hash)),
        Target::Url(url) => EmbedTarget::Url(url.clone()),
    };
    Embed { embed: Some(embed) }
}

fn reaction(kind: ReactionType, target: &Target) -> Result<Body, BuildError> {
    if kind == ReactionType::None {
        return Err(BuildError::NoReactionType);
    }
    let target = match target {
        Target::Cast(r) => ReactionTarget::CastId(cast_id(r.fid, r.hash)),
        Target::Url(url) => ReactionTarget::Url(url.clone()),
    };
    Ok(Body::Reaction(ReactionBody {
        r#type: kind.into(),
        target: Some(target),
    }))
}

fn follow(target_fid: u64) -> Body {
    Body::Link(LinkBody {
        r#type: "follow".to_owned(),
        display_timestamp: None,
        target: Some(LinkTarget::Fid(target_fid)),
    })
}

fn cast_id(fid: u64, hash: MessageHash) -> CastId {
    CastId {
        fid,
        hash: hash.as_bytes().to_vec(),
    }
}
