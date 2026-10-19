//! Authorising an app key with the custody key: the body of a gasless
//! `KEY_ADD`.
//!
//! The custody key makes two EIP-712 signatures, both in the domain
//! `Farcaster KeyAdd`, version 1, chain id 1, with no verifying contract:
//! `KeyAdd` over the key and its terms, and `SignedKeyRequest` naming the
//! app that asked for the key. Selfcast is not a registered app, so the
//! account requests its own key: `requestFid` is the account's FID and the
//! request is signed by the same custody key. Types and field order follow
//! snapchain v0.14.3 `src/core/validations/key.rs` (`key_add_types`,
//! `signed_key_request_types`, `SignedKeyRequestMetadata`).
//!
//! The message carrying this body is signed by the new app key itself
//! ([`crate::build::sign`] with [`crate::build::Action::KeyAdd`]).

use alloy_primitives::{Bytes, U256};
use alloy_sol_types::{Eip712Domain, SolStruct, SolValue, eip712_domain, sol};

use crate::cast::SignerKey;
use crate::custody::CustodyKey;
use crate::proto::{KeyAddBody, MessageType};
use crate::time::Timestamp;

/// The longest sliding expiry a node accepts: 90 days
/// (`MAX_KEY_TTL_SECONDS`, `key.rs`).
pub const MAX_TTL_SECS: u32 = 90 * 24 * 60 * 60;

const KEY_TYPE_ED25519: u32 = 1;
const METADATA_TYPE_SIGNED_KEY_REQUEST: u32 = 1;

sol! {
    struct KeyAdd {
        uint256 fid;
        bytes key;
        uint32 keyType;
        uint32[] scopes;
        uint32 ttl;
        uint32 nonce;
        uint256 deadline;
    }

    struct SignedKeyRequest {
        uint256 requestFid;
        bytes key;
        uint256 deadline;
    }

    struct SignedKeyRequestMetadata {
        uint256 requestFid;
        address requestSigner;
        bytes signature;
        uint256 deadline;
    }
}

/// The terms an app key is added on. These are exactly the fields the
/// custody key signs, so a confirmation shown from them shows what is signed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyRequest {
    pub fid: u64,
    pub key: SignerKey,
    /// The message types the key may sign.
    pub scopes: Vec<MessageType>,
    /// Sliding expiry in seconds, refreshed each time the key is used.
    pub ttl_secs: u32,
    /// Must exceed the account's current user nonce.
    pub nonce: u32,
    /// After this the custody signatures lapse.
    pub deadline: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeyAddError {
    #[error("deadline is before the Farcaster epoch or past 2157")]
    DeadlineOutOfRange,
    #[error("a key needs at least one scope")]
    NoScopes,
    #[error("a key's expiry must be between 1 second and 90 days")]
    BadTtl,
}

fn domain() -> Eip712Domain {
    eip712_domain! {
        name: "Farcaster KeyAdd",
        version: "1",
        chain_id: 1,
    }
}

/// The `KEY_ADD` body for `request`, signed by `custody`.
pub fn authorize(custody: &CustodyKey, request: &KeyRequest) -> Result<KeyAddBody, KeyAddError> {
    let deadline = request
        .deadline
        .to_farcaster()
        .ok_or(KeyAddError::DeadlineOutOfRange)?;
    if request.scopes.is_empty() {
        return Err(KeyAddError::NoScopes);
    }
    if request.ttl_secs == 0 || request.ttl_secs > MAX_TTL_SECS {
        return Err(KeyAddError::BadTtl);
    }
    let custody_signature =
        custody.sign_hash(&key_add(request, deadline).eip712_signing_hash(&domain()));
    Ok(KeyAddBody {
        key: request.key.as_bytes().to_vec(),
        key_type: KEY_TYPE_ED25519,
        custody_signature: custody_signature.to_vec(),
        deadline,
        nonce: request.nonce,
        metadata: metadata(custody, request.fid, &request.key, deadline),
        metadata_type: METADATA_TYPE_SIGNED_KEY_REQUEST,
        registration_tx_hash: Vec::new(),
        scopes: request.scopes.iter().map(|s| *s as i32).collect(),
        ttl: request.ttl_secs,
    })
}

fn key_add(request: &KeyRequest, deadline: u32) -> KeyAdd {
    KeyAdd {
        fid: U256::from(request.fid),
        key: Bytes::copy_from_slice(request.key.as_bytes()),
        keyType: KEY_TYPE_ED25519,
        scopes: request.scopes.iter().map(|s| *s as u32).collect(),
        ttl: request.ttl_secs,
        nonce: request.nonce,
        deadline: U256::from(deadline),
    }
}

/// The account requests its own key: `requestFid` is `fid` and the request
/// is signed by `custody`.
fn metadata(custody: &CustodyKey, fid: u64, key: &SignerKey, deadline: u32) -> Vec<u8> {
    let request = SignedKeyRequest {
        requestFid: U256::from(fid),
        key: Bytes::copy_from_slice(key.as_bytes()),
        deadline: U256::from(deadline),
    };
    let signature = custody.sign_hash(&request.eip712_signing_hash(&domain()));
    SignedKeyRequestMetadata {
        requestFid: U256::from(fid),
        requestSigner: custody.address().inner(),
        signature: Bytes::copy_from_slice(&signature),
        deadline: U256::from(deadline),
    }
    .abi_encode()
}
