//! `KEY_ADD` construction, checked the way a node checks it.
//!
//! The signatures are recovered as snapchain v0.14.3 recovers them
//! (`src/core/validations/key.rs`): EIP-712 typed data built from JSON with
//! `alloy-dyn-abi`, independent of the `sol!` types that made them, and the
//! signer recovered from `r || s || v`. Whether a node accepts the message
//! still needs a node and a real account.

use alloy_dyn_abi::TypedData;
use alloy_primitives::{Address, Signature};
use alloy_sol_types::{SolType, sol};
use paircaster_core::build::{Action, AppKey, BuildError, Envelope, sign};
use paircaster_core::custody::{CustodyError, CustodyKey};
use paircaster_core::key_add::{KeyAddError, KeyRequest, MAX_TTL_SECS, authorize};
use paircaster_core::node::parse;
use paircaster_core::proto::{FarcasterNetwork, KeyAddBody, MessageType};
use paircaster_core::time::Timestamp;
use prost::Message as _;

/// Published test phrases and the address each gives at m/44'/60'/0'/0/0:
/// Hardhat's and Anvil's default account, and the BIP-39 all-`abandon`
/// vector as wallets derive it. Their keys are public, yet a Farcaster
/// account exists for the Hardhat address (FID 1607387, seen 2026-10-19):
/// these tests sign offline only, and nothing signed with these phrases may
/// ever be sent to a node.
const HARDHAT: (&str, &str) = (
    "test test test test test test test test test test test junk",
    "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
);
const ABANDON: (&str, &str) = (
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    "0x9858EfFD232B4033E47d90003D41EC34EcaEda94",
);

sol! {
    struct SignedKeyRequestMetadata {
        uint256 requestFid;
        address requestSigner;
        bytes signature;
        uint256 deadline;
    }
}

const SCOPES: [MessageType; 6] = [
    MessageType::CastAdd,
    MessageType::CastRemove,
    MessageType::ReactionAdd,
    MessageType::ReactionRemove,
    MessageType::LinkAdd,
    MessageType::LinkRemove,
];

fn custody() -> CustodyKey {
    CustodyKey::from_phrase(HARDHAT.0).expect("valid phrase")
}

fn app_key() -> AppKey {
    AppKey::from_secret(&[0x11; 32])
}

/// FID 1234, deadline 2026-10-18T01:00:00Z.
fn request() -> KeyRequest {
    KeyRequest {
        fid: 1234,
        key: app_key().public(),
        scopes: SCOPES.to_vec(),
        ttl_secs: MAX_TTL_SECS,
        nonce: 1,
        deadline: Timestamp::from_unix(1_792_285_200),
    }
}

fn domain() -> serde_json::Value {
    serde_json::json!({ "name": "Farcaster KeyAdd", "version": "1", "chainId": 1 })
}

fn domain_type() -> serde_json::Value {
    serde_json::json!([
        { "name": "name", "type": "string" },
        { "name": "version", "type": "string" },
        { "name": "chainId", "type": "uint256" },
    ])
}

/// `key_add_typed_data` in `key.rs`.
fn snapchain_key_add(body: &KeyAddBody, fid: u64) -> TypedData {
    let json = serde_json::json!({
        "types": {
            "EIP712Domain": domain_type(),
            "KeyAdd": [
                { "name": "fid", "type": "uint256" },
                { "name": "key", "type": "bytes" },
                { "name": "keyType", "type": "uint32" },
                { "name": "scopes", "type": "uint32[]" },
                { "name": "ttl", "type": "uint32" },
                { "name": "nonce", "type": "uint32" },
                { "name": "deadline", "type": "uint256" },
            ],
        },
        "primaryType": "KeyAdd",
        "domain": domain(),
        "message": {
            "fid": fid,
            "key": format!("0x{}", hex::encode(&body.key)),
            "keyType": body.key_type,
            "scopes": body.scopes,
            "ttl": body.ttl,
            "nonce": body.nonce,
            "deadline": body.deadline,
        },
    });
    serde_json::from_value(json).expect("typed data")
}

/// `signed_key_request_typed_data` in `key.rs`.
fn snapchain_signed_key_request(request_fid: u64, key: &[u8], deadline: u64) -> TypedData {
    let json = serde_json::json!({
        "types": {
            "EIP712Domain": domain_type(),
            "SignedKeyRequest": [
                { "name": "requestFid", "type": "uint256" },
                { "name": "key", "type": "bytes" },
                { "name": "deadline", "type": "uint256" },
            ],
        },
        "primaryType": "SignedKeyRequest",
        "domain": domain(),
        "message": {
            "requestFid": request_fid,
            "key": format!("0x{}", hex::encode(key)),
            "deadline": deadline,
        },
    });
    serde_json::from_value(json).expect("typed data")
}

/// `recover_from_typed_data` in `key.rs`.
fn recover(typed: &TypedData, signature: &[u8]) -> Address {
    assert_eq!(signature.len(), 65);
    let prehash = typed.eip712_signing_hash().expect("hash");
    let parity = signature[64] != 0x1b && signature[64] != 0x00;
    Signature::from_bytes_and_parity(&signature[..64], parity)
        .recover_address_from_prehash(&prehash)
        .expect("recoverable")
}

fn custody_address() -> Address {
    HARDHAT.1.parse().expect("address")
}

#[test]
fn phrases_give_the_published_addresses() {
    for (phrase, address) in [HARDHAT, ABANDON] {
        let key = CustodyKey::from_phrase(phrase).expect("valid phrase");
        assert_eq!(key.address().to_string(), address);
    }
}

#[test]
fn phrase_case_and_spacing_do_not_matter() {
    let messy = format!("  {}\n", HARDHAT.0.to_uppercase().replace(' ', "   "));
    let key = CustodyKey::from_phrase(&messy).expect("valid phrase");
    assert_eq!(key.address().to_string(), HARDHAT.1);
}

#[test]
fn a_bad_checksum_or_word_is_refused() {
    let bad_checksum = "abandon ".repeat(12);
    let bad_word = HARDHAT.0.replace("junk", "jnuk");
    for phrase in [bad_checksum.as_str(), bad_word.as_str(), ""] {
        assert_eq!(
            CustodyKey::from_phrase(phrase).err(),
            Some(CustodyError::BadPhrase)
        );
    }
}

#[test]
fn the_custody_signature_recovers_to_the_custody_address() {
    let body = authorize(&custody(), &request()).expect("authorizes");
    let typed = snapchain_key_add(&body, 1234);
    assert_eq!(recover(&typed, &body.custody_signature), custody_address());
}

#[test]
fn the_metadata_is_the_accounts_own_request_signed_by_its_custody_key() {
    let body = authorize(&custody(), &request()).expect("authorizes");
    let meta = SignedKeyRequestMetadata::abi_decode(&body.metadata).expect("decodes");
    let request_fid: u64 = meta.requestFid.try_into().expect("fits");
    let deadline: u64 = meta.deadline.try_into().expect("fits");
    assert_eq!(request_fid, 1234);
    assert_eq!(deadline, u64::from(body.deadline));
    assert_eq!(meta.requestSigner, custody_address());
    let typed = snapchain_signed_key_request(request_fid, &body.key, deadline);
    assert_eq!(recover(&typed, &meta.signature), custody_address());
    assert_eq!(
        parse::request_fid(&body.metadata).expect("on-chain decoder"),
        1234
    );
}

#[test]
fn the_body_carries_the_requested_terms() {
    let body = authorize(&custody(), &request()).expect("authorizes");
    assert_eq!(body.key, app_key().public().as_bytes());
    assert_eq!((body.key_type, body.metadata_type), (1, 1));
    assert_eq!(body.scopes, [1, 2, 3, 4, 5, 6]);
    assert_eq!((body.ttl, body.nonce), (MAX_TTL_SECS, 1));
    assert_eq!(
        Some(body.deadline),
        Timestamp::from_unix(1_792_285_200).to_farcaster()
    );
    assert!(body.registration_tx_hash.is_empty());
}

#[test]
fn bad_terms_are_refused() {
    let no_scopes = KeyRequest {
        scopes: Vec::new(),
        ..request()
    };
    let no_ttl = KeyRequest {
        ttl_secs: 0,
        ..request()
    };
    let long_ttl = KeyRequest {
        ttl_secs: MAX_TTL_SECS + 1,
        ..request()
    };
    let early = KeyRequest {
        deadline: Timestamp::from_unix(0),
        ..request()
    };
    assert_eq!(
        authorize(&custody(), &no_scopes),
        Err(KeyAddError::NoScopes)
    );
    assert_eq!(authorize(&custody(), &no_ttl), Err(KeyAddError::BadTtl));
    assert_eq!(authorize(&custody(), &long_ttl), Err(KeyAddError::BadTtl));
    assert_eq!(
        authorize(&custody(), &early),
        Err(KeyAddError::DeadlineOutOfRange)
    );
}

fn envelope() -> Envelope {
    Envelope {
        fid: 1234,
        network: FarcasterNetwork::Mainnet,
        timestamp: Timestamp::from_unix(1_792_281_600),
    }
}

#[test]
fn a_key_add_must_be_signed_by_the_key_it_adds() {
    let body = authorize(&custody(), &request()).expect("authorizes");
    let other = AppKey::from_secret(&[0x22; 32]);
    assert_eq!(
        sign(&other, &envelope(), &Action::KeyAdd(body.clone())),
        Err(BuildError::KeyAddNotSelfSigned)
    );
    let signed = sign(&app_key(), &envelope(), &Action::KeyAdd(body)).expect("signs");
    assert_eq!(signed.signer(), app_key().public());
}

/// Our own output when fixed, so it guards against regressions only; the
/// recovery tests above are the check that the signatures are right.
/// Signing is deterministic (RFC 6979 for secp256k1, Ed25519 by design).
const GOLDEN_MESSAGE_HASH: &str = "801ba1e1a213035f26ebc6ef1c51c7db233d0d01";

#[test]
fn golden_key_add_message() {
    let body = authorize(&custody(), &request()).expect("authorizes");
    let encoded_body = body.encode_to_vec();
    let signed = sign(&app_key(), &envelope(), &Action::KeyAdd(body)).expect("signs");
    let hash = hex::encode(signed.hash().as_bytes());
    assert_eq!(
        hash,
        GOLDEN_MESSAGE_HASH,
        "body {}",
        hex::encode(encoded_body)
    );
}
