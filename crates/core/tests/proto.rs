mod common;

use common::fixture_messages;
use paircaster_core::message::verify;
use paircaster_core::proto::json::message_from_json;
use paircaster_core::proto::{CastAddBody, CastType, MessageData};
use prost::Message as _;

const SIGNED_FIXTURES: &[&str] = &[
    "castsByFid-3.json",
    "castsByFid-2.json",
    "castsByFid-194.json",
    "castsByFid-17.json",
    "castById-3.json",
    "reactionsByFid-like-3.json",
    "reactionsByFid-recast-3.json",
    "linksByFid-follow-3.json",
    "linksByTargetFid-follow-17.json",
];

#[test]
#[ignore = "needs PAIRCASTER_FIXTURES"]
fn every_fixture_cast_reaction_and_link_reencodes_to_its_hash_and_signature() {
    let messages: Vec<_> = SIGNED_FIXTURES
        .iter()
        .flat_map(|name| fixture_messages(name).into_iter().map(move |m| (*name, m)))
        .collect();
    let failures: Vec<String> = messages
        .iter()
        .filter_map(|(name, json)| {
            let hash = json["hash"].as_str().unwrap_or("<no hash>").to_owned();
            let outcome = message_from_json(json.clone())
                .map_err(|e| e.to_string())
                .and_then(|m| verify(&m).map_err(|e| e.to_string()));
            outcome.err().map(|e| format!("{name} {hash}: {e}"))
        })
        .collect();
    assert!(
        messages.len() >= 80,
        "only {} messages read",
        messages.len()
    );
    assert!(
        failures.is_empty(),
        "{} of {} failed:\n{}",
        failures.len(),
        messages.len(),
        failures.join("\n")
    );
}

#[test]
#[ignore = "needs PAIRCASTER_FIXTURES"]
fn every_fixture_message_data_decodes_back_from_its_encoding() {
    let mismatches: Vec<String> = SIGNED_FIXTURES
        .iter()
        .flat_map(|name| fixture_messages(name))
        .map(|json| message_from_json(json).expect("fixture message parses"))
        .filter_map(|m| {
            let data = m.data.expect("converted messages carry data");
            let decoded = MessageData::decode(data.encode_to_vec().as_slice());
            (decoded.as_ref() != Ok(&data)).then(|| hex::encode(&m.hash))
        })
        .collect();
    assert!(mismatches.is_empty(), "do not round-trip: {mismatches:?}");
}

#[test]
fn an_empty_cast_writes_its_packed_fields_as_ts_proto_does() {
    let body = CastAddBody {
        text: "hi".to_owned(),
        ..CastAddBody::default()
    };
    assert_eq!(hex::encode(body.encode_to_vec()), "1200220268692a00");
}

#[test]
fn a_cast_with_mentions_writes_them_packed_before_its_text() {
    let body = CastAddBody {
        mentions: vec![3, 300],
        text: "hi".to_owned(),
        mentions_positions: vec![0],
        r#type: CastType::LongCast.into(),
        ..CastAddBody::default()
    };
    assert_eq!(
        hex::encode(body.encode_to_vec()),
        "120303ac02220268692a01004001"
    );
}

#[test]
fn a_message_missing_its_fid_is_an_error() {
    let json = serde_json::json!({
        "data": {
            "type": "MESSAGE_TYPE_LINK_ADD",
            "timestamp": 1,
            "network": "FARCASTER_NETWORK_MAINNET",
            "linkBody": { "type": "follow", "displayTimestamp": null, "targetFid": 2 }
        },
        "hash": "0x00",
        "hashScheme": "HASH_SCHEME_BLAKE3",
        "signature": "",
        "signatureScheme": "SIGNATURE_SCHEME_ED25519",
        "signer": "0x00"
    });
    let err = message_from_json(json).unwrap_err().to_string();
    assert!(err.contains("fid"), "{err}");
}

#[test]
fn a_message_whose_body_does_not_match_its_type_is_an_error() {
    let json = serde_json::json!({
        "data": {
            "type": "MESSAGE_TYPE_CAST_REMOVE",
            "fid": 3,
            "timestamp": 1,
            "network": "FARCASTER_NETWORK_MAINNET",
            "linkBody": { "type": "follow", "displayTimestamp": null, "targetFid": 2 }
        },
        "hash": "0x00",
        "hashScheme": "HASH_SCHEME_BLAKE3",
        "signature": "",
        "signatureScheme": "SIGNATURE_SCHEME_ED25519",
        "signer": "0x00"
    });
    assert!(message_from_json(json).is_err());
}

#[test]
fn hex_without_0x_is_an_error() {
    let json = serde_json::json!({
        "data": {
            "type": "MESSAGE_TYPE_LINK_ADD",
            "fid": 3,
            "timestamp": 1,
            "network": "FARCASTER_NETWORK_MAINNET",
            "linkBody": { "type": "follow", "displayTimestamp": null, "targetFid": 2 }
        },
        "hash": "00",
        "hashScheme": "HASH_SCHEME_BLAKE3",
        "signature": "",
        "signatureScheme": "SIGNATURE_SCHEME_ED25519",
        "signer": "0x00"
    });
    let err = message_from_json(json).unwrap_err().to_string();
    assert!(err.contains("0x"), "{err}");
}
