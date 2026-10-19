mod common;

use common::{fixture, fixture_messages};
use selfcast_core::cast::{Cast, Target};
use selfcast_core::feed;
use selfcast_core::node::parse;
use selfcast_core::profile::Profile;
use selfcast_core::proto::ReactionType;
use selfcast_core::signer::{Signer, app_of};

const FOLLOWED: [u64; 3] = [2, 194, 17];

/// The FIDs snapchain v0.14.3 names in `map_signer_fid_to_name`
/// (`src/connectors/onchain_events/mod.rs:255-260`).
const FARCASTER_APP: u64 = 9152;
const BASE_APP: u64 = 309_857;

fn casts_of(fid: u64) -> Vec<Cast> {
    parse::casts(fixture(&format!("castsByFid-{fid}.json")))
        .expect("casts parse")
        .items
}

fn signers_of(fid: u64) -> Vec<Signer> {
    parse::signers(fixture(&format!("signersByFid-{fid}.json")))
        .expect("signers parse")
        .items
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn profile_takes_user_data_and_counts_follows() {
    let user_data = parse::user_data(fixture("userDataByFid-3.json")).expect("user data parses");
    let follows = parse::follows(fixture("linksByFid-follow-3.json")).expect("follows parse");
    assert_eq!(user_data.next_page_token, None);
    assert!(
        follows.next_page_token.is_some(),
        "fixture is one page of several"
    );

    let profile = Profile::assemble(3, &user_data.items, follows.items.len());

    let raw = fixture("userDataByFid-3.json");
    let expect = |kind: &str| newest_set_value(&raw, kind);
    assert!(
        expect("USER_DATA_TYPE_USERNAME").is_some(),
        "fixture has a username"
    );
    let has_empty = raw["messages"]
        .as_array()
        .expect("messages")
        .iter()
        .any(|m| m["data"]["userDataBody"]["value"] == "");
    assert!(
        has_empty,
        "fixture has an empty value, which must read as unset"
    );
    assert_eq!(profile.username, expect("USER_DATA_TYPE_USERNAME"));
    assert_eq!(profile.display_name, expect("USER_DATA_TYPE_DISPLAY"));
    assert_eq!(profile.bio, expect("USER_DATA_TYPE_BIO"));
    assert_eq!(profile.url, expect("USER_DATA_TYPE_URL"));
    assert_eq!(profile.location, expect("USER_DATA_TYPE_LOCATION"));
    assert_eq!(profile.twitter, expect("USER_DATA_TYPE_TWITTER"));
    assert_eq!(profile.github, expect("USER_DATA_TYPE_GITHUB"));
    assert_eq!(profile.following, 10);
}

/// The newest non-empty value of one user data type in a raw
/// `userDataByFid` response, read straight from the JSON.
fn newest_set_value(raw: &serde_json::Value, kind: &str) -> Option<String> {
    raw["messages"]
        .as_array()
        .expect("messages")
        .iter()
        .filter(|m| m["data"]["userDataBody"]["type"] == kind)
        .max_by_key(|m| m["data"]["timestamp"].as_u64())
        .and_then(|m| m["data"]["userDataBody"]["value"].as_str())
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn feed_is_newest_first_across_followed_accounts() {
    let casts: Vec<Cast> = FOLLOWED.iter().flat_map(|fid| casts_of(*fid)).collect();
    let signers: Vec<Signer> = FOLLOWED.iter().flat_map(|fid| signers_of(*fid)).collect();
    let newest = casts.iter().map(|c| c.timestamp).max().expect("casts");

    let items = feed::assemble(casts.clone(), &signers, 15);

    assert_eq!(items.len(), 15);
    assert_eq!(items[0].cast.timestamp, newest);
    assert!(
        items
            .windows(2)
            .all(|w| w[0].cast.timestamp >= w[1].cast.timestamp),
        "not newest first: {:?}",
        items.iter().map(|i| i.cast.timestamp).collect::<Vec<_>>()
    );
    let authors: std::collections::HashSet<u64> = items.iter().map(|i| i.cast.id.fid).collect();
    assert!(authors.len() > 1, "feed draws on one account only");
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn onchain_signers_name_the_app_from_their_metadata() {
    let apps: std::collections::HashSet<Option<u64>> =
        signers_of(3).iter().map(|s| s.app_fid).collect();
    assert!(apps.contains(&Some(FARCASTER_APP)), "{apps:?}");
    assert!(apps.contains(&Some(BASE_APP)), "{apps:?}");
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn every_fixture_cast_is_attributed_to_an_app() {
    let unattributed: Vec<String> = [3, 2, 194, 17]
        .iter()
        .flat_map(|fid| {
            let signers = signers_of(*fid);
            casts_of(*fid)
                .into_iter()
                .filter(move |c| app_of(c, &signers).is_none())
        })
        .map(|c| c.id.hash.to_string())
        .collect();
    assert!(unattributed.is_empty(), "{unattributed:?}");
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn reactions_parse_with_their_targets() {
    let likes = parse::reactions(fixture("reactionsByFid-like-3.json")).expect("likes parse");
    let recasts = parse::reactions(fixture("reactionsByFid-recast-3.json")).expect("recasts parse");
    assert_eq!(likes.items.len(), 10);
    assert!(likes.items.iter().all(|r| r.kind == ReactionType::Like));
    assert!(recasts.items.iter().all(|r| r.kind == ReactionType::Recast));
    assert!(
        likes
            .items
            .iter()
            .chain(&recasts.items)
            .all(|r| r.fid == 3 && matches!(r.target, Target::Cast(_)))
    );
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn a_cast_remove_in_a_page_of_casts_is_skipped() {
    let messages = fixture_messages("castsByFid-194.json").len();
    assert_eq!(casts_of(194).len(), messages - 1);
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn cast_by_id_matches_the_same_cast_in_casts_by_fid() {
    let cast = parse::cast_by_id(fixture("castById-3.json")).expect("cast parses");
    assert_eq!(cast, casts_of(3)[0]);
}

#[test]
fn a_cast_missing_its_timestamp_is_an_error() {
    let page = serde_json::json!({
        "messages": [{
            "data": {
                "type": "MESSAGE_TYPE_CAST_ADD",
                "fid": 3,
                "network": "FARCASTER_NETWORK_MAINNET",
                "castAddBody": {
                    "embedsDeprecated": [], "mentions": [], "parentCastId": null,
                    "parentUrl": null, "text": "hi", "embeds": [],
                    "mentionsPositions": [], "type": "CAST"
                }
            },
            "hash": "0x1111111111111111111111111111111111111111",
            "hashScheme": "HASH_SCHEME_BLAKE3",
            "signature": "",
            "signatureScheme": "SIGNATURE_SCHEME_ED25519",
            "signer": "0x2222222222222222222222222222222222222222222222222222222222222222"
        }],
        "nextPageToken": ""
    });
    let err = parse::casts(page).unwrap_err().to_string();
    assert!(err.contains("timestamp"), "{err}");
}

#[test]
fn a_page_missing_its_messages_is_an_error() {
    let err = parse::user_data(serde_json::json!({ "nextPageToken": "" })).unwrap_err();
    assert!(err.to_string().contains("messages"), "{err}");
}

/// A gasless signer as `Signer` at snapchain v0.14.3
/// `src/network/http_server.rs:1143` would serialise it; no fixture has one.
fn gasless_signers(request_fid: Option<u64>) -> serde_json::Value {
    let mut signer = serde_json::json!({
        "source": "SIGNER_SOURCE_OFFCHAIN",
        "key": "0x2222222222222222222222222222222222222222222222222222222222222222",
        "keyType": 1,
        "fid": 3,
        "scopes": ["MESSAGE_TYPE_CAST_ADD"],
        "ttl": 7_776_000
    });
    if let Some(fid) = request_fid {
        signer["requestFid"] = fid.into();
    }
    serde_json::json!({
        "signers": [signer],
        "gaslessSignerCount": 1,
        "gaslessSignerLimit": 1000,
        "currentUserNonce": 1
    })
}

#[test]
fn a_gasless_signer_names_its_app_directly() {
    let page = parse::signers(gasless_signers(Some(42))).expect("signers parse");
    assert_eq!(page.items[0].app_fid, Some(42));
    assert_eq!(page.next_page_token, None);
}

#[test]
fn a_gasless_signer_without_request_fid_is_an_error() {
    assert!(parse::signers(gasless_signers(None)).is_err());
}

fn word(value: u64) -> Vec<u8> {
    let mut w = vec![0; 24];
    w.extend(value.to_be_bytes());
    w
}

#[test]
fn request_fid_is_the_first_word_of_the_struct() {
    let metadata = [word(32), word(9152), word(0), word(128), word(0)].concat();
    assert_eq!(parse::request_fid(&metadata).expect("decodes"), 9152);
}

#[test]
fn request_fid_rejects_short_or_oversized_metadata() {
    assert!(parse::request_fid(&word(32)).is_err());
    let mut oversized = word(9152);
    oversized[0] = 1;
    assert!(parse::request_fid(&[word(32), oversized].concat()).is_err());
}
