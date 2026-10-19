//! Hashing and verification of Farcaster messages.
//!
//! Follows snapchain v0.14.3 `src/core/validations/message.rs`: the hash is
//! the first 20 bytes of BLAKE3 over `data_bytes` when the message carries
//! them, and over `MessageData` encoded otherwise (lines 105-125, 382-399);
//! the Ed25519 signature is over that 20-byte hash, checked with
//! `verify_strict` (lines 162-168, 356-378).

use ed25519_dalek::{Signature, VerifyingKey};
use prost::Message as _;

use crate::proto::Message;

/// Length of a Farcaster message hash, in bytes.
pub const HASH_LEN: usize = 20;

/// Why a message failed verification.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VerifyError {
    #[error("message has no data")]
    MissingData,
    #[error("hash is {stated}, but the data hashes to {computed}")]
    HashMismatch { stated: String, computed: String },
    #[error("signer is not an Ed25519 public key")]
    BadSigner,
    #[error("signature does not verify against the signer")]
    BadSignature,
}

/// The message hash of encoded `MessageData`: BLAKE3, truncated to 20 bytes.
pub fn hash_data(data_bytes: &[u8]) -> [u8; HASH_LEN] {
    let mut hash = [0; HASH_LEN];
    hash.copy_from_slice(&blake3::hash(data_bytes).as_bytes()[..HASH_LEN]);
    hash
}

/// The bytes a message's hash covers: `data_bytes` if present, else `data`
/// encoded.
pub fn signed_bytes(message: &Message) -> Result<Vec<u8>, VerifyError> {
    match (&message.data_bytes, &message.data) {
        (Some(bytes), _) => Ok(bytes.clone()),
        (None, Some(data)) => Ok(data.encode_to_vec()),
        (None, None) => Err(VerifyError::MissingData),
    }
}

/// Checks that the message's hash matches its data and that its signature
/// verifies against its signer.
pub fn verify(message: &Message) -> Result<(), VerifyError> {
    let computed = hash_data(&signed_bytes(message)?);
    if computed[..] != message.hash[..] {
        return Err(VerifyError::HashMismatch {
            stated: hex::encode(&message.hash),
            computed: hex::encode(computed),
        });
    }
    verify_signature(message)
}

fn verify_signature(message: &Message) -> Result<(), VerifyError> {
    let signer: [u8; 32] = message
        .signer
        .as_slice()
        .try_into()
        .map_err(|_| VerifyError::BadSigner)?;
    let key = VerifyingKey::from_bytes(&signer).map_err(|_| VerifyError::BadSigner)?;
    let signature =
        Signature::from_slice(&message.signature).map_err(|_| VerifyError::BadSignature)?;
    key.verify_strict(&message.hash, &signature)
        .map_err(|_| VerifyError::BadSignature)
}
