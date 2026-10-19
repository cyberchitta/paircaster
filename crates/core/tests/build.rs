mod common;

use common::fixture_messages;
use prost::Message as _;
use selfcast_core::build::{Action, AppKey, BuildError, Envelope, NewCast, sign};
use selfcast_core::cast::{CastRef, Mention, MessageHash, Target};
use selfcast_core::message::{VerifyError, hash_data, verify};
use selfcast_core::node::parse;
use selfcast_core::proto::{CastType, FarcasterNetwork, MessageData, ReactionType};
use selfcast_core::time::Timestamp;

/// Not any account's key: 32 bytes of 0x11.
const SECRET: [u8; 32] = [0x11; 32];

/// An FID far beyond those registered, and a fixed time:
/// 2026-10-18T00:00:00Z.
fn envelope() -> Envelope {
    Envelope {
        fid: 4_000_000_000,
        network: FarcasterNetwork::Testnet,
        timestamp: Timestamp::from_unix(1_792_281_600),
    }
}

fn some_cast() -> CastRef {
    CastRef {
        fid: 2,
        hash: MessageHash::from_bytes(&[0xab; 20]).expect("20 bytes"),
    }
}

fn actions() -> Vec<(&'static str, Action)> {
    vec![
        (
            "cast_add",
            Action::CastAdd(NewCast {
                text: "gm @x".to_owned(),
                mentions: vec![Mention {
                    fid: 3,
                    position: 3,
                }],
                embeds: vec![Target::Url("https://example.com".to_owned())],
                parent: Some(Target::Cast(some_cast())),
                kind: CastType::Cast,
            }),
        ),
        ("cast_remove", Action::CastRemove(some_cast().hash)),
        (
            "reaction_add",
            Action::ReactionAdd(ReactionType::Like, Target::Cast(some_cast())),
        ),
        (
            "reaction_remove",
            Action::ReactionRemove(ReactionType::Recast, Target::Cast(some_cast())),
        ),
        ("follow_add", Action::FollowAdd(3)),
        ("follow_remove", Action::FollowRemove(3)),
    ]
}

/// Our own output when the vectors were fixed, so they guard against
/// regressions only; a node's `validateMessage` is the check that they are
/// right. Each line: name, `data_bytes`, hash, signature, all hex.
const GOLDEN: &[(&str, &str, &str, &str)] = &[
    (
        "cast_add",
        "08011080d0acf30e1880cc965720022a3e1201031a1808021214abababababababababababababababababababab2205676d2040782a010332150a1368747470733a2f2f6578616d706c652e636f6d",
        "10edad09d66ccc55ebd2bec8bbd8801dabd308e3",
        "1e46fbb30c10fe889d1754ac942555545c31da1220d8479a28feb6284958e322324baefb9370d5ae3f07d917d127d4d31db96fc547c606655a8bbea80c83ab01",
    ),
    (
        "cast_remove",
        "08021080d0acf30e1880cc9657200232160a14abababababababababababababababababababab",
        "b3ca64db71c79628523ed548d5fb9ef1860d98fe",
        "25216588baf636d8a1da873432f419f679476b624234702af2e1f31e54fe3f36273d5e7c7a28fe0b251954586c782a6e7fc06fd2b70082aea58f623ed51a170a",
    ),
    (
        "reaction_add",
        "08031080d0acf30e1880cc965720023a1c0801121808021214abababababababababababababababababababab",
        "07a8205a40eb1f3ea6191b4f2a2689be7a84c08e",
        "03b54ecdbab311afda6298677c9c058c498acd8439ee1c1197eda0636294e4207b9d696008f15cc5d1c7ab7fe20bf5610de5437190d2c04c0f1f141c98f1d80e",
    ),
    (
        "reaction_remove",
        "08041080d0acf30e1880cc965720023a1c0802121808021214abababababababababababababababababababab",
        "ea59a4ab077e71a3a01fb994673b75dac92bc89e",
        "c23cec9d35c1321f234a03e86a50f13d332b50aa13daa09733f65fbe29f8f2ffd05e70184fcd4f904e0ea6516565ab76172055c412baf0d33cd907a571a7f60b",
    ),
    (
        "follow_add",
        "08051080d0acf30e1880cc96572002720a0a06666f6c6c6f771803",
        "e5d9a747e7c9945aa7cfde09f8cb393e913db271",
        "8d61efcca50b2b164c54a7d5fe23a266c4d565200010f4756f733c9472dcbbc0b74204e1da29b82f30d1688315359f33b9e9d8383aa589e023269c8df3b7a30f",
    ),
    (
        "follow_remove",
        "08061080d0acf30e1880cc96572002720a0a06666f6c6c6f771803",
        "07fa8031a21836d65a900731497ed3c71527bc46",
        "9dffc73eb83a4f7afb7f294e01f7d46bca95defc0a5ae2b795c4cfee1dea0689e5c034bb51c6aab580aa0197446c89c6c3e79e5c961ff3f4a2fa6fab63b5b10a",
    ),
];

#[test]
fn golden_vectors_hold_for_every_message_type() {
    let key = AppKey::from_secret(&SECRET);
    let actual: Vec<(String, String, String, String)> = actions()
        .iter()
        .map(|(name, action)| {
            let m = sign(&key, &envelope(), action).expect("builds");
            (
                (*name).to_owned(),
                hex::encode(m.data_bytes()),
                hex::encode(m.hash().as_bytes()),
                hex::encode(m.signature()),
            )
        })
        .collect();
    let expected: Vec<(String, String, String, String)> = GOLDEN
        .iter()
        .map(|(n, d, h, s)| ((*n).into(), (*d).into(), (*h).into(), (*s).into()))
        .collect();
    assert_eq!(actual, expected, "\n{}", as_rust(&actual));
}

fn as_rust(rows: &[(String, String, String, String)]) -> String {
    rows.iter()
        .map(|(n, d, h, s)| format!("    (\n        \"{n}\",\n        \"{d}\",\n        \"{h}\",\n        \"{s}\",\n    ),\n"))
        .collect()
}

#[test]
fn every_built_message_verifies_and_decodes_back() {
    let key = AppKey::from_secret(&SECRET);
    for (name, action) in actions() {
        let signed = sign(&key, &envelope(), &action).expect("builds");
        let message = signed.to_message();
        assert_eq!(verify(&message), Ok(()), "{name}");
        assert_eq!(
            message.data, None,
            "{name}: data is sent only as data_bytes"
        );
        let data = MessageData::decode(signed.data_bytes()).expect("decodes");
        assert_eq!(data.fid, 4_000_000_000, "{name}");
        assert_eq!(data.encode_to_vec(), signed.data_bytes(), "{name}");
    }
}

#[test]
fn the_submitted_message_carries_the_signed_bytes() {
    let key = AppKey::from_secret(&SECRET);
    let signed = sign(&key, &envelope(), &Action::FollowAdd(3)).expect("builds");
    let sent = selfcast_core::proto::Message::decode(signed.encode().as_slice()).expect("decodes");
    assert_eq!(sent.data_bytes.as_deref(), Some(signed.data_bytes()));
    assert_eq!(sent.hash, hash_data(signed.data_bytes()));
}

#[test]
fn a_byte_changed_after_signing_fails_verification() {
    let key = AppKey::from_secret(&SECRET);
    let signed = sign(&key, &envelope(), &Action::FollowAdd(3)).expect("builds");
    let mut message = signed.to_message();
    let bytes = message.data_bytes.as_mut().expect("data_bytes");
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    assert!(matches!(
        verify(&message),
        Err(VerifyError::HashMismatch { .. })
    ));

    message.hash = hash_data(message.data_bytes.as_deref().expect("data_bytes")).to_vec();
    assert_eq!(verify(&message), Err(VerifyError::BadSignature));
}

#[test]
fn a_message_needs_a_network_and_a_time_after_the_epoch() {
    let key = AppKey::from_secret(&SECRET);
    let no_network = Envelope {
        network: FarcasterNetwork::None,
        ..envelope()
    };
    let too_early = Envelope {
        timestamp: Timestamp::from_unix(0),
        ..envelope()
    };
    assert_eq!(
        sign(&key, &no_network, &Action::FollowAdd(3)),
        Err(BuildError::NoNetwork)
    );
    assert_eq!(
        sign(&key, &too_early, &Action::FollowAdd(3)),
        Err(BuildError::TimestampOutOfRange)
    );
}

const CAST_FIXTURES: &[&str] = &[
    "castsByFid-3.json",
    "castsByFid-2.json",
    "castsByFid-194.json",
    "castsByFid-17.json",
];

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn the_encoder_rebuilds_every_fixture_cast_exactly() {
    let key = AppKey::from_secret(&SECRET);
    let mut rebuilt = 0;
    let mismatches: Vec<String> = CAST_FIXTURES
        .iter()
        .flat_map(|name| {
            let page =
                serde_json::json!({ "messages": fixture_messages(name), "nextPageToken": "" });
            parse::casts(page).expect("casts parse").items
        })
        .filter_map(|cast| {
            rebuilt += 1;
            let envelope = Envelope {
                fid: cast.id.fid,
                network: FarcasterNetwork::Mainnet,
                timestamp: cast.timestamp,
            };
            let action = Action::CastAdd(NewCast {
                text: cast.text.clone(),
                mentions: cast.mentions.clone(),
                embeds: cast.embeds.clone(),
                parent: cast.parent.clone(),
                kind: cast.kind,
            });
            let signed = sign(&key, &envelope, &action).expect("builds");
            (signed.hash() != cast.id.hash).then(|| cast.id.hash.to_string())
        })
        .collect();
    assert_eq!(rebuilt, 49);
    assert!(mismatches.is_empty(), "not rebuilt exactly: {mismatches:?}");
}
