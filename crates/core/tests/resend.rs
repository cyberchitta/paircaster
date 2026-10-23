//! Retrying a write resends the saved bytes instead of signing again, and a
//! node that already holds them counts as sent.

use paircaster_core::Error;
use paircaster_core::build::{Action, AppKey, DecodeError, Envelope, SignedMessage, sign};
use paircaster_core::cast::MessageHash;
use paircaster_core::message::VerifyError;
use paircaster_core::node::Submitted;
use paircaster_core::node::parse::submitted;
use paircaster_core::proto::{FarcasterNetwork, Message, MessageData};
use paircaster_core::time::Timestamp;
use prost::Message as _;

/// Not any account's key: 32 bytes of 0x22.
const SECRET: [u8; 32] = [0x22; 32];

fn signed() -> SignedMessage {
    let envelope = Envelope {
        fid: 4_000_000_000,
        network: FarcasterNetwork::Testnet,
        timestamp: Timestamp::from_unix(1_792_281_600),
    };
    sign(
        &AppKey::from_secret(&SECRET),
        &envelope,
        &Action::FollowAdd(3),
    )
    .expect("builds")
}

#[test]
fn saved_bytes_decode_to_the_message_that_was_signed() {
    let original = signed();
    let saved = original.encode();
    let reloaded = SignedMessage::decode(&saved).expect("decodes");
    assert_eq!(reloaded, original);
    assert_eq!(reloaded.encode(), saved);
}

#[test]
fn saved_bytes_changed_on_disk_are_refused() {
    let mut message = Message::decode(&signed().encode()[..]).expect("decodes");
    let data = message.data_bytes.as_mut().expect("has data_bytes");
    let last = data.len() - 1;
    data[last] ^= 1;
    assert!(matches!(
        SignedMessage::decode(&message.encode_to_vec()),
        Err(DecodeError::Unverified(VerifyError::HashMismatch { .. }))
    ));
}

#[test]
fn a_message_carrying_decoded_data_is_not_resent() {
    let mut message = Message::decode(&signed().encode()[..]).expect("decodes");
    let bytes = message.data_bytes.take().expect("has data_bytes");
    message.data = Some(MessageData::decode(&bytes[..]).expect("decodes"));
    assert_eq!(
        SignedMessage::decode(&message.encode_to_vec()),
        Err(DecodeError::NotAsEncoded)
    );
}

#[test]
fn a_message_that_would_not_encode_back_the_same_is_not_resent() {
    let mut message = Message::decode(&signed().encode()[..]).expect("decodes");
    message.hash_scheme = 0;
    assert_eq!(
        SignedMessage::decode(&message.encode_to_vec()),
        Err(DecodeError::NotAsEncoded)
    );
}

#[test]
fn bytes_that_are_not_a_message_are_refused() {
    assert_eq!(
        SignedMessage::decode(&[0xff, 0xff, 0xff]),
        Err(DecodeError::NotAMessage)
    );
}

fn hash() -> MessageHash {
    signed().hash()
}

#[test]
fn a_success_echoing_the_hash_is_accepted() {
    let body = format!(r#"{{"data":{{}},"hash":"{}","signer":"0x00"}}"#, hash());
    assert_eq!(
        submitted(200, &body, hash()).ok(),
        Some(Submitted::Accepted)
    );
}

#[test]
fn a_success_echoing_another_hash_is_an_error() {
    let other = MessageHash::from_bytes(&[0; 20]).expect("20 bytes");
    let body = format!(r#"{{"hash":"{other}"}}"#);
    assert!(matches!(
        submitted(200, &body, hash()),
        Err(Error::Malformed(_))
    ));
}

/// Shaped from snapchain v0.14.3 `src/network/http_server.rs`
/// (`ErrorResponse`, `submit_message`) and the message in
/// `server_tests.rs` line 5447, in the form of a live lookup error saved
/// from `crackle`. No live submit rejection seen yet.
fn rejection(code_and_message: &str) -> String {
    format!(
        r#"{{"error":"Failed to submit message","error_detail":"status: InvalidArgument, message: \"{code_and_message}\", details: [], metadata: MetadataMap {{ headers: {{}} }}"}}"#
    )
}

#[test]
fn a_duplicate_counts_as_already_sent() {
    let body = rejection("bad_request.duplicate/message has already been merged");
    assert_eq!(
        submitted(400, &body, hash()).ok(),
        Some(Submitted::AlreadyKnown)
    );
}

#[test]
fn any_other_rejection_is_an_error() {
    let body = rejection("bad_request.validation_failure/unknown fid");
    assert!(matches!(
        submitted(400, &body, hash()),
        Err(Error::Node { status: 400, .. })
    ));
}
