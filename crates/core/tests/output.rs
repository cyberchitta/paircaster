//! The `--json` shapes, locked byte for byte: the skill, the viewer and
//! pages built on it read them.

use paircaster_core::address::{Address, Protocol, Verification};
use paircaster_core::cast::{Cast, CastRef, Mention, MessageHash, Reaction, SignerKey, Target};
use paircaster_core::event::{Change, Content, Event, EventBody};
use paircaster_core::feed::FeedItem;
use paircaster_core::follow::Follow;
use paircaster_core::output;
use paircaster_core::profile::{Followers, Profile, UserDatum, Users};
use paircaster_core::proto::{CastType, ReactionType, UserDataType};
use paircaster_core::reaction::CastReactions;
use paircaster_core::thread::{ChainTop, Thread};
use paircaster_core::time::Timestamp;

fn item() -> FeedItem {
    let hash = MessageHash::from_bytes(&[0xab; 20]).expect("20 bytes");
    FeedItem {
        cast: Cast {
            id: CastRef { fid: 2, hash },
            timestamp: Timestamp::from_farcaster(0),
            kind: CastType::Cast,
            text: "hi ".to_owned(),
            mentions: vec![Mention {
                fid: 9,
                position: 3,
            }],
            embeds: vec![Target::Url("https://e".to_owned())],
            parent: Some(Target::Cast(CastRef { fid: 17, hash })),
            signer: SignerKey::from_bytes(&[1; 32]).expect("32 bytes"),
        },
        app_fid: Some(9152),
    }
}

/// The author, 2, has no username; the mentioned account, 9, is alice.
fn users() -> Users {
    [(2, Vec::new()), username(9, "alice")].into()
}

const ACCOUNTS: &str = r#""accounts":[{"fid":2,"username":null,"display_name":null,"pfp_url":null},{"fid":9,"username":"alice","display_name":null,"pfp_url":null}]"#;

fn cast_json() -> String {
    let h = format!("0x{}", "ab".repeat(20));
    let signer = format!("0x{}", "01".repeat(32));
    format!(
        r#"{{"hash":"{h}","fid":2,"timestamp":"2021-01-01T00:00:00Z","text":"hi ","display_text":"hi @alice","mentions":[{{"fid":9,"username":"alice","position":3}}],"embeds":[{{"url":"https://e"}}],"parent":{{"cast":{{"fid":17,"hash":"{h}"}}}},"signer":"{signer}","app_fid":9152}}"#
    )
}

fn username(fid: u64, name: &str) -> (u64, Vec<UserDatum>) {
    let datum = UserDatum {
        kind: UserDataType::Username,
        value: name.to_owned(),
        timestamp: Timestamp::from_farcaster(0),
    };
    (fid, vec![datum])
}

#[test]
fn feed_json_is_version_1() {
    let expected = format!(
        r#"{{"version":1,"fid":3,"casts":[{}],{ACCOUNTS}}}"#,
        cast_json()
    );
    assert_eq!(output::feed_json(3, &[item()], &users()), expected);
}

#[test]
fn casts_json_is_version_1() {
    let expected = format!(
        r#"{{"version":1,"fids":[2,5],"casts":[{}],{ACCOUNTS}}}"#,
        cast_json()
    );
    assert_eq!(output::casts_json(&[2, 5], &[item()], &users()), expected);
}

#[test]
fn replies_json_is_version_1() {
    let parent = CastRef {
        fid: 37,
        hash: MessageHash::from_bytes(&[0xcd; 20]).expect("20 bytes"),
    };
    let expected = format!(
        r#"{{"version":1,"parent":{{"fid":37,"hash":"0x{}"}},"casts":[{}],{ACCOUNTS}}}"#,
        "cd".repeat(20),
        cast_json()
    );
    assert_eq!(output::replies_json(&parent, &[item()], &users()), expected);
}

#[test]
fn thread_json_is_version_1() {
    let thread = Thread {
        top: ChainTop::Missing(CastRef {
            fid: 17,
            hash: MessageHash::from_bytes(&[0xab; 20]).expect("20 bytes"),
        }),
        ancestors: vec![item()],
        cast: item(),
        replies: vec![],
    };
    let (fid, mut alice) = username(9, "alice");
    alice.push(UserDatum {
        kind: UserDataType::Pfp,
        value: "https://i/a.png".to_owned(),
        timestamp: Timestamp::from_farcaster(0),
    });
    let users = [username(2, "v"), (fid, alice)].into();
    let expected = format!(
        r#"{{"version":1,"top":{{"missing":{{"fid":17,"hash":"0x{}"}}}},"ancestors":[{c}],"cast":{c},"replies":[],"accounts":[{{"fid":2,"username":"v","display_name":null,"pfp_url":null}},{{"fid":9,"username":"alice","display_name":null,"pfp_url":"https://i/a.png"}}]}}"#,
        "ab".repeat(20),
        c = cast_json()
    );
    assert_eq!(output::thread_json(&thread, &users), expected);
}

#[test]
fn reactions_json_is_version_1() {
    let cast = CastRef {
        fid: 37,
        hash: MessageHash::from_bytes(&[0xcd; 20]).expect("20 bytes"),
    };
    let reaction = |fid, kind| Reaction {
        fid,
        kind,
        target: Target::Cast(cast),
        timestamp: Timestamp::from_farcaster(0),
    };
    let r = CastReactions::assemble(
        vec![reaction(2, ReactionType::Like)],
        vec![reaction(9, ReactionType::Recast)],
    );
    let users = [username(2, "v"), username(9, "alice")].into();
    let expected = format!(
        r#"{{"version":1,"cast":{{"fid":37,"hash":"0x{}"}},"likes":[{{"fid":2,"timestamp":"2021-01-01T00:00:00Z"}}],"recasts":[{{"fid":9,"timestamp":"2021-01-01T00:00:00Z"}}],"accounts":[{{"fid":2,"username":"v","display_name":null,"pfp_url":null}},{{"fid":9,"username":"alice","display_name":null,"pfp_url":null}}]}}"#,
        "cd".repeat(20)
    );
    assert_eq!(output::reactions_json(&cast, &r, &users), expected);
}

#[test]
fn profile_json_is_version_1() {
    let profile = Profile {
        fid: 3,
        username: Some("alice".to_owned()),
        display_name: None,
        bio: None,
        pfp_url: None,
        url: None,
        location: None,
        twitter: None,
        github: None,
        banner_url: None,
        following: 10,
        followers: Followers::AtLeast(10_002),
    };
    assert_eq!(
        output::profile_json(&profile),
        r#"{"version":1,"fid":3,"username":"alice","display_name":null,"bio":null,"pfp_url":null,"url":null,"location":null,"twitter":null,"github":null,"banner_url":null,"following":10,"followers":10002,"followers_exact":false}"#
    );
}

fn event(body: EventBody) -> Event {
    Event {
        shard: 1,
        id: 5 << 14 | 2,
        block: 5,
        block_time: Timestamp::from_farcaster(10),
        body,
    }
}

#[test]
fn event_lines_keep_their_shape() {
    let h = format!("0x{}", "ab".repeat(20));
    let signer = format!("0x{}", "01".repeat(32));
    let head = r#"{"version":1,"shard":1,"id":81922,"block":5,"block_time":"2021-01-01T00:00:10Z""#;
    let message = |change, kind: &str, content| {
        event(EventBody::Message {
            change,
            kind: kind.to_owned(),
            fid: 2,
            hash: MessageHash::from_bytes(&[0xab; 20]).expect("20 bytes"),
            timestamp: Timestamp::from_farcaster(9),
            signer: SignerKey::from_bytes(&[1; 32]).expect("32 bytes"),
            content,
        })
    };
    let fields =
        format!(r#""fid":2,"hash":"{h}","timestamp":"2021-01-01T00:00:09Z","signer":"{signer}""#);
    let cast = format!(
        r#","text":"hi ","mentions":[{{"fid":9,"position":3}}],"embeds":[{{"url":"https://e"}}],"parent":{{"cast":{{"fid":17,"hash":"{h}"}}}}"#
    );
    [
        (Change::Merge, "merge"),
        (Change::Prune, "prune"),
        (Change::Revoke, "revoke"),
    ]
    .iter()
    .for_each(|(c, name)| {
        assert_eq!(
            output::event_json(&message(
                *c,
                "cast_add",
                Some(Content::Cast(Box::new(item().cast)))
            )),
            format!(r#"{head},"event":"{name}","type":"cast_add",{fields}{cast}}}"#)
        );
    });
    let like = Content::Reaction {
        kind: ReactionType::Like,
        target: Target::Cast(CastRef {
            fid: 37,
            hash: MessageHash::from_bytes(&[0xab; 20]).expect("20 bytes"),
        }),
    };
    let follow = Content::Link {
        kind: "follow".to_owned(),
        target_fid: 3,
    };
    [
        (
            "reaction_add",
            Some(like),
            format!(r#","reaction":"like","target":{{"cast":{{"fid":37,"hash":"{h}"}}}}"#),
        ),
        (
            "link_add",
            Some(follow),
            r#","link":"follow","target_fid":3"#.to_owned(),
        ),
        ("user_data_add", None, String::new()),
    ]
    .into_iter()
    .for_each(|(kind, content, rest)| {
        assert_eq!(
            output::event_json(&message(Change::Merge, kind, content)),
            format!(r#"{head},"event":"merge","type":"{kind}",{fields}{rest}}}"#)
        );
    });
    let others = [
        (
            EventBody::UsernameProof {
                fid: 3,
                name: "dwr".to_owned(),
                kind: "fname".to_owned(),
            },
            r#""event":"username_proof","type":"fname","fid":3,"name":"dwr"}"#,
        ),
        (
            EventBody::OnChain {
                fid: 4,
                kind: "id_register".to_owned(),
            },
            r#""event":"onchain","type":"id_register","fid":4}"#,
        ),
        (
            EventBody::Other {
                node_type: "HUB_EVENT_TYPE_MERGE_FAILURE".to_owned(),
            },
            r#""event":"other","type":"HUB_EVENT_TYPE_MERGE_FAILURE"}"#,
        ),
    ];
    others.into_iter().for_each(|(body, rest)| {
        assert_eq!(output::event_json(&event(body)), format!("{head},{rest}"));
    });
}

#[test]
fn follows_json_is_version_1() {
    let follows = [Follow {
        fid: 2,
        timestamp: Timestamp::from_farcaster(0),
    }];
    let users = [username(2, "v")].into();
    let tail = r#"[{"fid":2,"timestamp":"2021-01-01T00:00:00Z"}],"accounts":[{"fid":2,"username":"v","display_name":null,"pfp_url":null}]}"#;
    assert_eq!(
        output::follows_json(3, true, &follows, &users),
        format!(r#"{{"version":1,"fid":3,"followers":{tail}"#)
    );
    assert_eq!(
        output::follows_json(3, false, &follows, &users),
        format!(r#"{{"version":1,"fid":3,"following":{tail}"#)
    );
}

#[test]
fn addresses_json_is_version_1() {
    let a = Address {
        verification: Verification {
            address: "0xab".to_owned(),
            protocol: Protocol::Solana,
            chain_id: 10,
            timestamp: Timestamp::from_farcaster(0),
        },
        primary: true,
    };
    assert_eq!(
        output::addresses_json(3, &[a]),
        r#"{"version":1,"fid":3,"addresses":[{"address":"0xab","protocol":"solana","chain_id":10,"timestamp":"2021-01-01T00:00:00Z","primary":true}]}"#
    );
}
