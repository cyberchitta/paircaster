mod common;

use std::collections::{BTreeSet, HashMap};

use common::{fixture, fixture_messages};
use selfcast_core::Error;
use selfcast_core::account::Username;
use selfcast_core::cast::{Cast, CastRef, ChannelUrl, Reaction, Target};
use selfcast_core::event::{self, Change, EventBody, EventWalk, PageEvent};
use selfcast_core::feed::{self, AppFilter, FeedItem};
use selfcast_core::follow::{self, Follow};
use selfcast_core::node::{self, ShardSync, SyncState, parse};
use selfcast_core::profile::{FollowerTally, Followers, Profile};
use selfcast_core::proto::ReactionType;
use selfcast_core::reaction::CastReactions;
use selfcast_core::signer::{Signer, app_of};
use selfcast_core::tally::PageTally;
use selfcast_core::time::Timestamp;

const FOLLOWED: [u64; 3] = [2, 194, 17];

/// When the fixtures were saved, 2026-10-19.
const NOW: Timestamp = Timestamp::from_unix(1_792_368_000);

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

    let profile = Profile::assemble(
        3,
        &user_data.items,
        follows.items.len(),
        Followers::Exact(25),
    );

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
    assert_eq!(profile.followers, Followers::Exact(25));
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn followers_are_the_accounts_whose_follow_targets_the_account() {
    let raw = fixture("linksByTargetFid-follow-17.json");
    let page = parse::followers(raw.clone(), 17).expect("followers parse");
    let expected: Vec<u64> = raw["messages"]
        .as_array()
        .expect("messages")
        .iter()
        .map(|m| m["data"]["fid"].as_u64().expect("fid"))
        .collect();
    assert_eq!(page.items, expected);
    assert!(
        page.next_page_token.is_some(),
        "fixture is one page of several"
    );
    assert!(
        parse::followers(raw, 18).is_err(),
        "followers of 17 are not of 18"
    );
}

#[test]
fn a_page_that_brings_no_new_follower_ends_the_count() {
    let mut tally = FollowerTally::default();
    let pages: [&[u64]; 4] = [&[1, 2, 3], &[4, 1], &[], &[2, 3]];
    let fresh: Vec<bool> = pages.iter().map(|p| tally.add_page(p)).collect();
    assert_eq!(fresh, [true, true, false, false]);
    assert_eq!(tally.count(), 4);
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn repeating_reply_pages_give_each_reply_once() {
    let page = |n: u32| {
        parse::casts(fixture(&format!("castsByParent-37-0x50184b62-p{n}.json")))
            .expect("replies parse")
    };
    let mut tally = PageTally::default();
    let fresh: Vec<bool> = (1..=3)
        .map(|n| tally.add_page(page(n).items, |c: &Cast| c.id.hash))
        .collect();
    assert_eq!(fresh, [true, true, false], "page 3 is page 1 again");
    assert!(
        page(3).next_page_token.is_some(),
        "the node never ends the paging"
    );
    let all = parse::casts(fixture("castsByParent-37-0x50184b62-all.json")).expect("parse");
    let hashes = |casts: &[Cast]| casts.iter().map(|c| c.id.hash).collect::<BTreeSet<_>>();
    let replies = tally.into_items();
    assert_eq!(replies.len(), 48);
    assert_eq!(hashes(&replies), hashes(&all.items));
}

fn bounty() -> CastRef {
    "37:0x50184b62b4c52fd76628713562e1eda3dccaf63d"
        .parse()
        .expect("FID:HASH")
}

fn likes_page(name: &str) -> node::Page<Reaction> {
    let file = format!("reactionsByCast-like-37-0x50184b62-{name}.json");
    parse::reactions_to_cast(fixture(&file), &bounty(), ReactionType::Like).expect("likes parse")
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn repeating_reaction_pages_give_each_account_once() {
    let mut tally = PageTally::default();
    let fresh: Vec<bool> = (1..=5)
        .map(|n| tally.add_page(likes_page(&format!("p{n}")).items, |r: &Reaction| r.fid))
        .collect();
    assert_eq!(fresh, [true, true, true, true, false], "page 5 repeats");
    let fids = |rs: &[Reaction]| rs.iter().map(|r| r.fid).collect::<BTreeSet<_>>();
    let likes = tally.into_items();
    assert_eq!(likes.len(), 290);
    assert_eq!(fids(&likes), fids(&likes_page("all").items));
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn a_page_of_another_kind_of_reaction_is_refused() {
    let file = fixture("reactionsByCast-like-37-0x50184b62-all.json");
    assert!(parse::reactions_to_cast(file, &bounty(), ReactionType::Recast).is_err());
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn a_page_of_reactions_to_another_cast_is_refused() {
    let file = fixture("reactionsByCast-like-37-0x50184b62-all.json");
    let other = CastRef { fid: 3, ..bounty() };
    assert!(parse::reactions_to_cast(file, &other, ReactionType::Like).is_err());
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn reactions_are_oldest_first_and_each_account_listed_once() {
    let recasts = parse::reactions_to_cast(
        fixture("reactionsByCast-recast-37-0x50184b62-all.json"),
        &bounty(),
        ReactionType::Recast,
    )
    .expect("recasts parse");
    let mut likes = likes_page("all").items;
    likes.reverse();
    let r = CastReactions::assemble(likes, recasts.items);
    assert!(r.likes.is_sorted_by_key(|x| x.timestamp));
    assert!(r.recasts.is_sorted_by_key(|x| x.timestamp));
    let likers = r.likes.iter().map(|x| x.fid);
    let recasters = r.recasts.iter().map(|x| x.fid);
    assert_eq!(r.account_fids(), likers.chain(recasters).collect());
    assert!(r.account_fids().len() < r.likes.len() + r.recasts.len());
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn mentions_are_written_in_where_they_were_made() {
    let replies = parse::casts(fixture("castsByParent-37-0x50184b62-all.json")).expect("parse");
    let mentioned: Vec<&Cast> = replies
        .items
        .iter()
        .filter(|c| !c.mentions.is_empty())
        .collect();
    assert_eq!(mentioned.len(), 12);
    let names: HashMap<u64, String> = mentioned
        .iter()
        .flat_map(|c| &c.mentions)
        .map(|m| (m.fid, format!("n{}", m.fid)))
        .collect();
    for cast in mentioned {
        let shown = cast
            .display_text(&names)
            .expect("positions fall between characters");
        let added: usize = cast.mentions.iter().map(|m| names[&m.fid].len() + 1).sum();
        assert_eq!(shown.len(), cast.text.len() + added, "{}", cast.id);
        let first = &cast.mentions[0];
        let at = first.position as usize;
        assert_eq!(&shown[..at], &cast.text[..at]);
        assert!(shown[at..].starts_with(&format!("@{}", names[&first.fid])));
    }
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn mention_positions_the_network_refuses_are_refused() {
    let raw = fixture("castsByFid-3.json");
    let with = |edit: &dyn Fn(&mut serde_json::Value)| {
        let mut page = raw.clone();
        let cast = page["messages"]
            .as_array_mut()
            .expect("messages")
            .iter_mut()
            .find(|m| {
                m["hash"]
                    .as_str()
                    .is_some_and(|h| h.starts_with("0x9579611f"))
            })
            .expect("the cast with a mention at its end");
        edit(&mut cast["data"]["castAddBody"]);
        parse::casts(page)
    };
    assert!(with(&|_| {}).is_ok());
    assert!(
        with(&|b| b["mentionsPositions"][0] = 35.into()).is_err(),
        "past the end"
    );
    let unsorted = |b: &mut serde_json::Value| {
        b["mentions"] = serde_json::json!([3, 2]);
        b["mentionsPositions"] = serde_json::json!([5, 4]);
    };
    assert!(with(&unsorted).is_err(), "not ascending");
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

    let items = feed::assemble(casts.clone(), &signers, &AppFilter::Any, NOW, 15);

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

/// The whole fixture feed, unfiltered and unlimited.
fn whole_feed() -> (Vec<Cast>, Vec<Signer>, Vec<FeedItem>) {
    let casts: Vec<Cast> = FOLLOWED.iter().flat_map(|fid| casts_of(*fid)).collect();
    let signers: Vec<Signer> = FOLLOWED.iter().flat_map(|fid| signers_of(*fid)).collect();
    let all = feed::assemble(casts.clone(), &signers, &AppFilter::Any, NOW, usize::MAX);
    (casts, signers, all)
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn including_an_app_keeps_only_its_casts_newest_first() {
    let (casts, signers, all) = whole_feed();
    let ours = |i: &&FeedItem| i.app_fid == Some(FARCASTER_APP);
    assert!(
        all.iter().any(|i| !ours(&i)),
        "fixtures need casts from another app"
    );
    let expected: Vec<FeedItem> = all.iter().filter(ours).take(5).cloned().collect();
    assert_eq!(expected.len(), 5, "fixtures need 5 casts from the app");

    let only = AppFilter::Only([FARCASTER_APP].into());
    assert_eq!(feed::assemble(casts, &signers, &only, NOW, 5), expected);
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn excluding_an_app_keeps_every_other_cast() {
    let (casts, signers, all) = whole_feed();
    let expected: Vec<FeedItem> = all
        .iter()
        .filter(|i| i.app_fid != Some(FARCASTER_APP))
        .cloned()
        .collect();
    assert!(!expected.is_empty(), "fixtures need casts from another app");

    let not = AppFilter::Not([FARCASTER_APP].into());
    assert_eq!(
        feed::assemble(casts, &signers, &not, NOW, usize::MAX),
        expected
    );
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn a_cast_dated_over_ten_minutes_ahead_is_left_out() {
    let signers = signers_of(2);
    let dated = |secs: u64| Cast {
        timestamp: Timestamp::from_unix(NOW.unix_seconds() + secs),
        ..casts_of(2)[0].clone()
    };
    let (edge, beyond) = (
        dated(feed::MAX_FUTURE_SECS),
        dated(feed::MAX_FUTURE_SECS + 1),
    );

    let items = feed::assemble([edge.clone(), beyond], &signers, &AppFilter::Any, NOW, 10);

    let kept: Vec<Cast> = items.into_iter().map(|i| i.cast).collect();
    assert_eq!(kept, [edge]);
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

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn sync_state_takes_each_shards_height_and_lag() {
    let state = parse::sync_state(fixture("info.json")).expect("info parses");
    assert_eq!(state.version, "0.14.2");
    let ids: Vec<u32> = state.shards.iter().map(|s| s.id).collect();
    assert_eq!(ids, [0, 1, 2]);
    assert_eq!(state.shards[1].height, 49_015_571);
    assert_eq!(state.seconds_behind(), Some(1));
}

#[test]
fn sync_state_without_block_delay_is_an_error() {
    let info = serde_json::json!({
        "version": "0.14.2",
        "shardInfos": [{ "shardId": 1, "maxHeight": 7 }],
    });
    assert!(parse::sync_state(info).is_err());
}

#[test]
fn an_empty_id_register_page_means_the_node_has_not_seen_the_account() {
    let seen = serde_json::json!({ "events": [{ "type": "EVENT_TYPE_ID_REGISTER", "fid": 3 }] });
    let unseen = serde_json::json!({ "events": [] });
    assert!(parse::has_id_register(seen).expect("parses"));
    assert!(!parse::has_id_register(unseen).expect("parses"));
    assert!(parse::has_id_register(serde_json::json!({})).is_err());
}

#[test]
fn follower_counting_stops_at_the_cap_with_a_lower_bound() {
    let mut tally = FollowerTally::default();
    assert_eq!(tally.step(&[1, 2], true, Some(4)), None);
    assert_eq!(
        tally.step(&[3, 4, 5], true, Some(4)),
        Some(Followers::AtLeast(5))
    );
    let mut all = FollowerTally::default();
    assert_eq!(all.step(&[1, 2], true, None), None);
    assert_eq!(all.step(&[3, 1], true, None), None);
    assert_eq!(all.step(&[1, 2], true, None), Some(Followers::Exact(3)));
    let mut last = FollowerTally::default();
    assert_eq!(
        last.step(&[1, 2, 3, 4], false, Some(4)),
        Some(Followers::Exact(4))
    );
}

/// From snapchain's `route_fid` (`src/mempool/routing.rs`, `f3e857f`),
/// worked by hand with mainnet's two message shards.
#[test]
fn route_fid_matches_snapchain_for_known_fids() {
    let routed: Vec<_> = [1, 2, 3, 194, 3_356_360]
        .into_iter()
        .map(|fid| node::route_fid(fid, 2))
        .collect();
    assert_eq!(routed, [Some(1), Some(2), Some(1), Some(1), Some(2)]);
}

#[test]
fn route_fid_has_no_shard_without_shards_or_for_an_oversized_fid() {
    assert_eq!(node::route_fid(3, 0), None);
    assert_eq!(node::route_fid(u64::from(u32::MAX) + 1, 2), None);
}

#[test]
fn shard_of_skips_the_block_shard_when_counting() {
    let shard = |id| ShardSync {
        id,
        height: 1,
        seconds_behind: u64::from(id) * 100,
    };
    let state = SyncState {
        version: "0.14.3".to_owned(),
        shards: vec![shard(0), shard(1), shard(2)],
    };
    assert_eq!(state.shard_of(3_356_360).map(|s| s.id), Some(2));
    assert_eq!(state.shard_of(3).map(|s| s.id), Some(1));
    let blocks_only = SyncState {
        shards: vec![shard(0)],
        ..state
    };
    assert_eq!(blocks_only.shard_of(3), None);
}

fn name(s: &str) -> Username {
    s.parse().expect("a username")
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn each_kind_of_name_proof_gives_its_fid() {
    [("v", 2), ("vitalik.eth", 5650), ("jesse.base.eth", 99)]
        .iter()
        .for_each(|(n, fid)| {
            let proof = fixture(&format!("userNameProofByName-{n}.json"));
            assert_eq!(parse::name_proof(proof, &name(n)).expect("parses"), *fid);
        });
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn a_proof_for_another_name_or_fid_0_is_refused() {
    let proof = || fixture("userNameProofByName-v.json");
    assert!(parse::name_proof(proof(), &name("w")).is_err());
    let mut zero = proof();
    zero["fid"] = 0.into();
    assert!(parse::name_proof(zero, &name("v")).is_err());
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn a_name_the_node_lacks_is_its_not_found_reply() {
    [
        "error-userNameProofByName.json",
        "error-userNameProofByName-ens.json",
    ]
    .iter()
    .for_each(|f| {
        let body = fixture(f);
        let detail = body["error_detail"].as_str().expect("error_detail");
        let e = Error::Node {
            status: 400,
            error: String::new(),
            detail: detail.to_owned(),
        };
        assert!(parse::is_not_found(&e), "{f}");
    });
}

fn popup_ama() -> ChannelUrl {
    "https://warpcast.com/~/channel/popup-ama"
        .parse()
        .expect("a channel URL")
}

fn channel_page(name: &str) -> Vec<Cast> {
    let file = fixture(&format!("castsByParent-url-popup-ama-{name}.json"));
    parse::casts_in_channel(file, &popup_ama())
        .expect("channel parses")
        .items
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn a_small_page_holds_the_channels_newest_casts() {
    let newest = |casts: Vec<Cast>| {
        feed::assemble(
            casts,
            &[],
            &AppFilter::Any,
            Timestamp::from_unix(1_800_000_000),
            10,
        )
        .into_iter()
        .map(|i| i.cast.id.hash)
        .collect::<Vec<_>>()
    };
    let small = channel_page("p10");
    assert_eq!(small.len(), 20, "10 from each of two shards");
    assert_eq!(newest(small), newest(channel_page("all")));
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn a_page_for_another_channel_is_refused() {
    let file = fixture("castsByParent-url-popup-ama-p10.json");
    let other: ChannelUrl = "https://warpcast.com/~/channel/popup".parse().expect("URL");
    assert!(parse::casts_in_channel(file, &other).is_err());
}

#[test]
fn a_channel_url_is_1_to_256_bytes() {
    assert!("x".parse::<ChannelUrl>().is_ok());
    assert!("x".repeat(256).parse::<ChannelUrl>().is_ok());
    assert!("x".repeat(257).parse::<ChannelUrl>().is_err());
    assert!("".parse::<ChannelUrl>().is_err());
}

fn events_page() -> Vec<PageEvent> {
    parse::events(fixture("events-1-49115416.json")).expect("events parse")
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn an_events_page_reads_every_kind_of_event() {
    let page = events_page();
    let count = |f: &dyn Fn(&PageEvent) -> bool| page.iter().filter(|e| f(e)).count();
    let change = |c: Change| move |e: &PageEvent| matches!(e, PageEvent::Event { body: EventBody::Message { change, .. }, .. } if *change == c);
    assert_eq!(count(&|e| matches!(e, PageEvent::Block { .. })), 15_600);
    assert_eq!(count(&change(Change::Merge)), 2_850);
    assert_eq!(count(&change(Change::Prune)), 1_458);
    assert_eq!(count(&change(Change::Revoke)), 2);
    let body = |f: fn(&EventBody) -> bool| {
        count(&|e| matches!(e, PageEvent::Event { body, .. } if f(body)))
    };
    assert_eq!(body(|b| matches!(b, EventBody::OnChain { .. })), 69);
    assert_eq!(body(|b| matches!(b, EventBody::UsernameProof { .. })), 21);
    assert_eq!(body(|b| matches!(b, EventBody::Other { .. })), 0);
    let casts = body(
        |b| matches!(b, EventBody::Message { change: Change::Merge, kind, .. } if kind == "cast_add"),
    );
    assert_eq!(casts, 799);
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn a_walk_keeps_the_blocks_in_its_window_and_stops_at_until() {
    let since = Timestamp::from_farcaster(183_000_000);
    let until = Timestamp::from_farcaster(183_005_000);
    let mut walk = EventWalk::new(
        1,
        event::first_event_of(49_115_416),
        Some(since),
        Some(until),
    );
    let kept = walk.add_page(events_page()).expect("walks");
    assert!(!kept.is_empty());
    assert!(
        kept.iter()
            .all(|e| e.block_time >= since && e.block_time < until)
    );
    assert!(kept.iter().all(|e| e.shard == 1 && e.id >> 14 == e.block));
    assert_eq!(walk.next_id(), None, "until falls inside the page");
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn a_walk_goes_on_after_the_last_event_of_a_page() {
    let page = events_page();
    let last = match page.last().expect("not empty") {
        PageEvent::Block { block, .. } => event::first_event_of(*block),
        PageEvent::Event { id, .. } => *id,
    };
    let mut walk = EventWalk::new(1, 0, None, None);
    let kept = walk.add_page(page).expect("walks");
    assert_eq!(kept.len(), 20_000 - 15_600);
    assert_eq!(walk.next_id(), Some(last + 1));
    walk.add_page(Vec::new()).expect("an empty page");
    assert_eq!(walk.next_id(), None, "an empty page ends it");
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn an_event_before_its_blocks_confirmation_is_refused() {
    let page = events_page();
    let first = page
        .iter()
        .find_map(|e| match e {
            PageEvent::Event { block, .. } => Some(*block),
            PageEvent::Block { .. } => None,
        })
        .expect("an event");
    let without: Vec<PageEvent> = page
        .into_iter()
        .filter(|e| !matches!(e, PageEvent::Block { block, .. } if *block == first))
        .collect();
    assert!(
        matches!(without[0], PageEvent::Block { .. }),
        "an earlier block stays"
    );
    assert!(EventWalk::new(1, 0, None, None).add_page(without).is_err());
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn an_event_outside_its_block_is_refused() {
    let mut page = fixture("events-1-49115416.json");
    let merge = page["events"]
        .as_array_mut()
        .expect("events")
        .iter_mut()
        .find(|e| e["type"] == "HUB_EVENT_TYPE_MERGE_MESSAGE")
        .expect("a merge");
    merge["blockNumber"] = 1.into();
    assert!(parse::events(page).is_err());
}

fn newest_followers_page(n: u32) -> Vec<Follow> {
    let file = fixture(&format!("linksByTargetFid-follow-3-newest-{n}.json"));
    parse::follower_links(file, 3)
        .expect("followers parse")
        .items
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn a_small_page_holds_the_newest_followers() {
    let small = newest_followers_page(50);
    assert_eq!(small.len(), 100, "50 from each of two shards");
    let newest = follow::newest(small, 50);
    assert!(newest.is_sorted_by_key(|f| std::cmp::Reverse(f.timestamp)));
    assert_eq!(newest, follow::newest(newest_followers_page(1000), 50));
}

#[test]
#[ignore = "needs SELFCAST_FIXTURES"]
fn follows_keep_when_they_were_made() {
    let raw = fixture("linksByFid-follow-3.json");
    let times: Vec<u32> = raw["messages"]
        .as_array()
        .expect("messages")
        .iter()
        .map(|m| m["data"]["timestamp"].as_u64().expect("timestamp") as u32)
        .collect();
    let follows = parse::follow_links(raw).expect("follows parse").items;
    let parsed: Vec<Timestamp> = follows.iter().map(|f| f.timestamp).collect();
    assert_eq!(
        parsed,
        times
            .into_iter()
            .map(Timestamp::from_farcaster)
            .collect::<Vec<_>>()
    );
}
