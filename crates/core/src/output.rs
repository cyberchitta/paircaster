//! The JSON the reads produce: `paircaster <read> --json`, and the same text
//! from the viewer's WebAssembly. An external format the agent skill, the
//! viewer and pages built on it depend on. Each shape is version 1. A new
//! field keeps the `version`, so readers ignore fields they do not know; a
//! field removed, renamed or changed in meaning bumps it.

use serde::Serialize;

use crate::address::Address;
use crate::cast::{Cast, CastRef, ChannelUrl, Mention, Reaction, Target};
use crate::event::{Change, Content, Event, EventBody};
use crate::feed::FeedItem;
use crate::follow::Follow;
use crate::profile::{self, Names, Profile, Users};
use crate::proto::ReactionType;
use crate::reaction::CastReactions;
use crate::thread::{ChainTop, Thread};

/// `paircaster profile <fid> --json`, version 1:
///
/// ```json
/// {
///   "version": 1,
///   "fid": 1234,
///   "username": "alice",
///   "display_name": "Alice",
///   "bio": "…",
///   "pfp_url": "https://…",
///   "url": "https://…",
///   "location": null,
///   "twitter": "alice",
///   "github": null,
///   "banner_url": "https://…",
///   "following": 10,
///   "followers": 25,
///   "followers_exact": true
/// }
/// ```
///
/// Each text field is `null` when the account has not set it, or set it
/// empty. `following` is how many accounts it follows, `followers` how many
/// follow it, both as the node holds them. `followers_exact` is `false` when
/// counting stopped at 10000 (without `--all-followers`): `followers` is
/// then a lower bound.
pub fn profile_json(p: &Profile) -> String {
    to_json(&ProfileV1 {
        version: 1,
        fid: p.fid,
        username: p.username.as_deref(),
        display_name: p.display_name.as_deref(),
        bio: p.bio.as_deref(),
        pfp_url: p.pfp_url.as_deref(),
        url: p.url.as_deref(),
        location: p.location.as_deref(),
        twitter: p.twitter.as_deref(),
        github: p.github.as_deref(),
        banner_url: p.banner_url.as_deref(),
        following: p.following,
        followers: p.followers.count(),
        followers_exact: p.followers.is_exact(),
    })
}

/// `paircaster feed <fid> --json`, version 1:
///
/// ```json
/// {
///   "version": 1,
///   "fid": 1234,
///   "casts": [
///     {
///       "hash": "0x…",
///       "fid": 1235,
///       "timestamp": "2024-03-03T09:46:40Z",
///       "text": "gm  and hi",
///       "display_text": "gm @alice and hi",
///       "mentions": [{ "fid": 1236, "username": "alice", "position": 3 }],
///       "embeds": [{ "url": "https://…" }, { "cast": { "fid": 1237, "hash": "0x…" } }],
///       "parent": { "cast": { "fid": 1237, "hash": "0x…" } },
///       "signer": "0x…",
///       "app_fid": 9152
///     }
///   ],
///   "accounts": [ { "fid": 1235, "username": "bob", "display_name": "Bob", "pfp_url": "https://…" } ]
/// }
/// ```
///
/// `casts` is newest first, after any `--include-app` or `--exclude-app`
/// filter. `parent` is `null`, `{ "cast": … }` or
/// `{ "url": … }`. `text` is as posted, without the mentioned names;
/// `mentions[].position` is a byte offset into it. `display_text` has each
/// mention written in as `@username`, or `@fid:N` for an account with no
/// username (`mentions[].username` is then `null`); it is `null` when a
/// position falls inside a character, which the network allows.
/// `app_fid` is the account of the app whose key signed the cast, or `null`
/// when that key is no longer listed or its registration names no app.
/// `accounts` is as in `thread`: each author and each mentioned account
/// once, by FID.
pub fn feed_json(fid: u64, items: &[FeedItem], users: &Users) -> String {
    to_json(&FeedV1 {
        version: 1,
        fid,
        casts: casts_v1(items, &profile::names_of(users)),
        accounts: accounts_v1(users),
    })
}

/// `paircaster casts <fid>... --json`, version 1:
///
/// ```json
/// { "version": 1, "fids": [2, 3], "casts": [ … ], "accounts": [ … ] }
/// ```
///
/// `fids` are the accounts asked for, in the order given. Each element of
/// `casts` has the same shape as in `feed`, newest first, after any app
/// filter, and `accounts` is as in `feed`.
pub fn casts_json(fids: &[u64], items: &[FeedItem], users: &Users) -> String {
    to_json(&CastsV1 {
        version: 1,
        fids,
        casts: casts_v1(items, &profile::names_of(users)),
        accounts: accounts_v1(users),
    })
}

/// `paircaster channel <URL> --json`, version 1:
///
/// ```json
/// { "version": 1, "channel": "https://warpcast.com/~/channel/fc-updates", "casts": [ … ], "accounts": [ … ] }
/// ```
///
/// `channel` is the URL asked for. Each element of `casts` has the same
/// shape as in `feed`, newest first, after any app filter; replies to them
/// are not included. `accounts` is as in `feed`.
pub fn channel_json(channel: &ChannelUrl, items: &[FeedItem], users: &Users) -> String {
    to_json(&ChannelV1 {
        version: 1,
        channel: channel.as_str(),
        casts: casts_v1(items, &profile::names_of(users)),
        accounts: accounts_v1(users),
    })
}

/// One line of `paircaster events --json`, version 1. A message event:
///
/// ```json
/// { "version": 1, "shard": 1, "id": 804707024897, "block": 49115419,
///   "block_time": "2026-10-19T00:00:03Z", "event": "merge", "type": "cast_add",
///   "fid": 978766, "hash": "0x…", "timestamp": "2026-10-19T00:00:02Z",
///   "signer": "0x…", "text": "gm  and hi",
///   "mentions": [{ "fid": 1236, "position": 3 }], "embeds": [ … ], "parent": null }
/// ```
///
/// `event` is `merge`, `prune` (dropped for storage) or `revoke` (its app
/// key was removed), and `type` the message type: `cast_add`,
/// `reaction_add`, `link_add`, `user_data_add`, …. `timestamp` is the time
/// the message gives itself; `block_time` is when the shard merged it.
/// Only a `cast_add` has `text`, `mentions`, `embeds` and `parent`, as in
/// `feed` but with no usernames: `text` is without the mentioned names.
/// A `reaction_add` or `reaction_remove` ends with what it reacts to, a
/// `link_add` or `link_remove` with the account it links:
///
/// ```json
/// { …, "type": "reaction_add", …, "reaction": "like", "target": { "cast": { "fid": 37, "hash": "0x…" } } }
/// { …, "type": "link_add", …, "link": "follow", "target_fid": 3 }
/// ```
///
/// The other events, after the same first five fields:
///
/// ```json
/// { "event": "username_proof", "type": "fname", "fid": 3356219, "name": "creatorfuel" }
/// { "event": "onchain", "type": "id_register", "fid": 3356213 }
/// { "event": "other", "type": "HUB_EVENT_TYPE_…" }
/// ```
pub fn event_json(e: &Event) -> String {
    to_json(&EventV1 {
        version: 1,
        shard: e.shard,
        id: e.id,
        block: e.block,
        block_time: e.block_time.to_string(),
        body: event_body_v1(&e.body),
    })
}

fn event_body_v1(body: &EventBody) -> EventBodyV1<'_> {
    match body {
        EventBody::Message {
            change,
            kind,
            fid,
            hash,
            timestamp,
            signer,
            content,
        } => {
            let m = EventMessageV1 {
                kind,
                fid: *fid,
                hash: hash.to_string(),
                timestamp: timestamp.to_string(),
                signer: signer.to_string(),
                content: content.as_ref().map(event_content_v1),
            };
            changed(*change, m)
        }
        EventBody::UsernameProof { fid, name, kind } => EventBodyV1::UsernameProof {
            kind,
            fid: *fid,
            name,
        },
        EventBody::OnChain { fid, kind } => EventBodyV1::Onchain { kind, fid: *fid },
        EventBody::Other { node_type } => EventBodyV1::Other { kind: node_type },
    }
}

fn changed(change: Change, m: EventMessageV1<'_>) -> EventBodyV1<'_> {
    match change {
        Change::Merge => EventBodyV1::Merge(m),
        Change::Prune => EventBodyV1::Prune(m),
        Change::Revoke => EventBodyV1::Revoke(m),
    }
}

fn event_content_v1(c: &Content) -> EventContentV1<'_> {
    match c {
        Content::Cast(cast) => EventContentV1::Cast(event_cast_v1(cast)),
        Content::Reaction { kind, target } => EventContentV1::Reaction {
            reaction: reaction_name(*kind),
            target: target_v1(target),
        },
        Content::Link { kind, target_fid } => EventContentV1::Link {
            link: kind,
            target_fid: *target_fid,
        },
    }
}

/// A reaction's kind as the JSON names it: `like` or `recast`.
pub fn reaction_name(kind: ReactionType) -> &'static str {
    match kind {
        ReactionType::Like => "like",
        ReactionType::Recast => "recast",
        ReactionType::None => "none",
    }
}

fn event_cast_v1(c: &Cast) -> EventCastV1<'_> {
    EventCastV1 {
        text: &c.text,
        mentions: c.mentions.iter().map(event_mention_v1).collect(),
        embeds: c.embeds.iter().map(target_v1).collect(),
        parent: c.parent.as_ref().map(target_v1),
    }
}

fn event_mention_v1(m: &Mention) -> EventMentionV1 {
    EventMentionV1 {
        fid: m.fid,
        position: m.position,
    }
}

/// `paircaster replies <FID:HASH> --json`, version 1:
///
/// ```json
/// { "version": 1, "parent": { "fid": 37, "hash": "0x…" }, "casts": [ … ], "accounts": [ … ] }
/// ```
///
/// `parent` is the cast asked about. Each element of `casts` has the same
/// shape as in `feed`, but oldest first: the newest `--limit` replies after
/// any app filter, in the order they were posted. `accounts` is as in
/// `feed`.
pub fn replies_json(parent: &CastRef, items: &[FeedItem], users: &Users) -> String {
    to_json(&RepliesV1 {
        version: 1,
        parent: parent_v1(parent),
        casts: casts_v1(items, &profile::names_of(users)),
        accounts: accounts_v1(users),
    })
}

/// `paircaster thread <FID:HASH> --json`, version 1:
///
/// ```json
/// {
///   "version": 1,
///   "top": "root",
///   "ancestors": [ … ],
///   "cast": { … },
///   "replies": [ … ],
///   "accounts": [ { "fid": 2, "username": "v", "display_name": "Varun", "pfp_url": "https://…" } ]
/// }
/// ```
///
/// `cast` is the cast asked about. `ancestors` are the casts it replies to,
/// root first. `top` says where they stop: `"root"` at a cast that replies
/// to no cast; `{"missing": {"fid", "hash"}}` at a parent this node does not
/// hold; `{"cut": {"fid", "hash"}}` after 50 casts, naming the next one up.
/// `replies` are the direct replies, as `replies` gives them. Every cast has
/// the shape it has in `feed`. `accounts` has each author and each mentioned
/// account once, by FID, with `null` for a name or picture not set.
pub fn thread_json(t: &Thread, users: &Users) -> String {
    let names = profile::names_of(users);
    to_json(&ThreadV1 {
        version: 1,
        top: top_v1(t.top),
        ancestors: casts_v1(&t.ancestors, &names),
        cast: cast_v1(&t.cast.cast, t.cast.app_fid, &names),
        replies: casts_v1(&t.replies, &names),
        accounts: accounts_v1(users),
    })
}

/// `paircaster show <FID:HASH> --json`, version 1: one cast by its ID.
///
/// ```json
/// { "version": 1, "cast": { … }, "accounts": [ … ] }
/// ```
///
/// `cast` has the shape it has in `feed`. `accounts` is as in `thread`, for
/// its author and each account it mentions. The casts it replies to and its
/// replies are not read: `thread` reads those.
pub fn cast_json(item: &FeedItem, users: &Users) -> String {
    let names = profile::names_of(users);
    to_json(&OneCastV1 {
        version: 1,
        cast: cast_v1(&item.cast, item.app_fid, &names),
        accounts: accounts_v1(users),
    })
}

/// `paircaster followers <account> --json` and `paircaster following <account>
/// --json`, version 1:
///
/// ```json
/// {
///   "version": 1,
///   "fid": 3,
///   "followers": [ { "fid": 2, "timestamp": "2026-10-22T06:51:30Z" } ],
///   "accounts": [ { "fid": 2, "username": "v", "display_name": "Varun", "pfp_url": "https://…" } ]
/// }
/// ```
///
/// The list is `followers` or `following`: the newest follows first, each
/// with when it was made. `accounts` is as in `thread`, for everyone listed.
pub fn follows_json(fid: u64, followers: bool, follows: &[Follow], users: &Users) -> String {
    let list = follows.iter().map(follow_v1).collect();
    let (followers, following) = match followers {
        true => (Some(list), None),
        false => (None, Some(list)),
    };
    to_json(&FollowsV1 {
        version: 1,
        fid,
        followers,
        following,
        accounts: accounts_v1(users),
    })
}

fn follow_v1(f: &Follow) -> AccountAtV1 {
    AccountAtV1 {
        fid: f.fid,
        timestamp: f.timestamp.to_string(),
    }
}

/// `paircaster addresses <account> --json`, version 1:
///
/// ```json
/// {
///   "version": 1,
///   "fid": 3,
///   "addresses": [
///     { "address": "0x6ce0…fea5", "protocol": "ethereum", "chain_id": 0,
///       "timestamp": "2025-09-23T…Z", "primary": true }
///   ]
/// }
/// ```
///
/// Oldest first. `protocol` is `ethereum` or `solana`; `chain_id` is as the
/// verification gives it; `primary` marks the address the account names as
/// its primary one for that protocol.
pub fn addresses_json(fid: u64, addresses: &[Address]) -> String {
    to_json(&AddressesV1 {
        version: 1,
        fid,
        addresses: addresses
            .iter()
            .map(|a| AddressV1 {
                address: &a.verification.address,
                protocol: a.verification.protocol.name(),
                chain_id: a.verification.chain_id,
                timestamp: a.verification.timestamp.to_string(),
                primary: a.primary,
            })
            .collect(),
    })
}

/// `paircaster reactions <FID:HASH> --json`, version 1:
///
/// ```json
/// {
///   "version": 1,
///   "cast": { "fid": 37, "hash": "0x…" },
///   "likes": [ { "fid": 2, "timestamp": "2026-10-22T06:51:30Z" } ],
///   "recasts": [ … ],
///   "accounts": [ { "fid": 2, "username": "v", "display_name": "Varun", "pfp_url": "https://…" } ]
/// }
/// ```
///
/// `cast` is the cast asked about. `likes` and `recasts` hold one entry per
/// account, oldest first, as the node holds them now: an undone like is not
/// there. `accounts` is as in `thread`, for everyone who reacted.
pub fn reactions_json(cast: &CastRef, r: &CastReactions, users: &Users) -> String {
    to_json(&ReactionsV1 {
        version: 1,
        cast: parent_v1(cast),
        likes: r.likes.iter().map(account_at_v1).collect(),
        recasts: r.recasts.iter().map(account_at_v1).collect(),
        accounts: accounts_v1(users),
    })
}

#[derive(Serialize)]
struct ProfileV1<'a> {
    version: u32,
    fid: u64,
    username: Option<&'a str>,
    display_name: Option<&'a str>,
    bio: Option<&'a str>,
    pfp_url: Option<&'a str>,
    url: Option<&'a str>,
    location: Option<&'a str>,
    twitter: Option<&'a str>,
    github: Option<&'a str>,
    banner_url: Option<&'a str>,
    following: usize,
    followers: usize,
    followers_exact: bool,
}

#[derive(Serialize)]
struct FeedV1<'a> {
    version: u32,
    fid: u64,
    casts: Vec<CastV1<'a>>,
    accounts: Vec<AccountV1>,
}

#[derive(Serialize)]
struct CastsV1<'a> {
    version: u32,
    fids: &'a [u64],
    casts: Vec<CastV1<'a>>,
    accounts: Vec<AccountV1>,
}

#[derive(Serialize)]
struct EventV1<'a> {
    version: u32,
    shard: u32,
    id: u64,
    block: u64,
    block_time: String,
    #[serde(flatten)]
    body: EventBodyV1<'a>,
}

#[derive(Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum EventBodyV1<'a> {
    Merge(EventMessageV1<'a>),
    Prune(EventMessageV1<'a>),
    Revoke(EventMessageV1<'a>),
    UsernameProof {
        #[serde(rename = "type")]
        kind: &'a str,
        fid: u64,
        name: &'a str,
    },
    Onchain {
        #[serde(rename = "type")]
        kind: &'a str,
        fid: u64,
    },
    Other {
        #[serde(rename = "type")]
        kind: &'a str,
    },
}

#[derive(Serialize)]
struct EventMessageV1<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    fid: u64,
    hash: String,
    timestamp: String,
    signer: String,
    #[serde(flatten)]
    content: Option<EventContentV1<'a>>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum EventContentV1<'a> {
    Cast(EventCastV1<'a>),
    Reaction {
        reaction: &'static str,
        target: TargetV1<'a>,
    },
    Link {
        link: &'a str,
        target_fid: u64,
    },
}

#[derive(Serialize)]
struct EventCastV1<'a> {
    text: &'a str,
    mentions: Vec<EventMentionV1>,
    embeds: Vec<TargetV1<'a>>,
    parent: Option<TargetV1<'a>>,
}

#[derive(Serialize)]
struct EventMentionV1 {
    fid: u64,
    position: u32,
}

#[derive(Serialize)]
struct FollowsV1 {
    version: u32,
    fid: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    followers: Option<Vec<AccountAtV1>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    following: Option<Vec<AccountAtV1>>,
    accounts: Vec<AccountV1>,
}

#[derive(Serialize)]
struct AddressesV1<'a> {
    version: u32,
    fid: u64,
    addresses: Vec<AddressV1<'a>>,
}

#[derive(Serialize)]
struct AddressV1<'a> {
    address: &'a str,
    protocol: &'static str,
    chain_id: u32,
    timestamp: String,
    primary: bool,
}

#[derive(Serialize)]
struct ChannelV1<'a> {
    version: u32,
    channel: &'a str,
    casts: Vec<CastV1<'a>>,
    accounts: Vec<AccountV1>,
}

#[derive(Serialize)]
struct RepliesV1<'a> {
    version: u32,
    parent: ParentV1,
    casts: Vec<CastV1<'a>>,
    accounts: Vec<AccountV1>,
}

#[derive(Serialize)]
struct ParentV1 {
    fid: u64,
    hash: String,
}

#[derive(Serialize)]
struct ThreadV1<'a> {
    version: u32,
    top: TopV1,
    ancestors: Vec<CastV1<'a>>,
    cast: CastV1<'a>,
    replies: Vec<CastV1<'a>>,
    accounts: Vec<AccountV1>,
}

#[derive(Serialize)]
struct OneCastV1<'a> {
    version: u32,
    cast: CastV1<'a>,
    accounts: Vec<AccountV1>,
}

#[derive(Serialize)]
struct ReactionsV1 {
    version: u32,
    cast: ParentV1,
    likes: Vec<AccountAtV1>,
    recasts: Vec<AccountAtV1>,
    accounts: Vec<AccountV1>,
}

#[derive(Serialize)]
struct AccountAtV1 {
    fid: u64,
    timestamp: String,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum TopV1 {
    Root,
    Missing(ParentV1),
    Cut(ParentV1),
}

#[derive(Serialize)]
struct AccountV1 {
    fid: u64,
    username: Option<String>,
    display_name: Option<String>,
    pfp_url: Option<String>,
}

#[derive(Serialize)]
struct CastV1<'a> {
    hash: String,
    fid: u64,
    timestamp: String,
    text: &'a str,
    display_text: Option<String>,
    mentions: Vec<MentionV1<'a>>,
    embeds: Vec<TargetV1<'a>>,
    parent: Option<TargetV1<'a>>,
    signer: String,
    app_fid: Option<u64>,
}

#[derive(Serialize)]
struct MentionV1<'a> {
    fid: u64,
    username: Option<&'a str>,
    position: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum TargetV1<'a> {
    Cast { fid: u64, hash: String },
    Url(&'a str),
}

fn top_v1(top: ChainTop) -> TopV1 {
    match top {
        ChainTop::Root => TopV1::Root,
        ChainTop::Missing(r) => TopV1::Missing(parent_v1(&r)),
        ChainTop::Cut(r) => TopV1::Cut(parent_v1(&r)),
    }
}

fn parent_v1(r: &CastRef) -> ParentV1 {
    ParentV1 {
        fid: r.fid,
        hash: r.hash.to_string(),
    }
}

fn accounts_v1(users: &Users) -> Vec<AccountV1> {
    users
        .iter()
        .map(|(fid, data)| AccountV1 {
            fid: *fid,
            username: profile::username(data),
            display_name: profile::display_name(data),
            pfp_url: profile::pfp_url(data),
        })
        .collect()
}

fn account_at_v1(r: &Reaction) -> AccountAtV1 {
    AccountAtV1 {
        fid: r.fid,
        timestamp: r.timestamp.to_string(),
    }
}

fn casts_v1<'a>(items: &'a [FeedItem], names: &'a Names) -> Vec<CastV1<'a>> {
    items
        .iter()
        .map(|i| cast_v1(&i.cast, i.app_fid, names))
        .collect()
}

fn cast_v1<'a>(c: &'a Cast, app_fid: Option<u64>, names: &'a Names) -> CastV1<'a> {
    CastV1 {
        hash: c.id.hash.to_string(),
        fid: c.id.fid,
        timestamp: c.timestamp.to_string(),
        text: &c.text,
        display_text: c.display_text(names),
        mentions: c
            .mentions
            .iter()
            .map(|m| MentionV1 {
                fid: m.fid,
                username: names.get(&m.fid).map(String::as_str),
                position: m.position,
            })
            .collect(),
        embeds: c.embeds.iter().map(target_v1).collect(),
        parent: c.parent.as_ref().map(target_v1),
        signer: c.signer.to_string(),
        app_fid,
    }
}

fn target_v1(t: &Target) -> TargetV1<'_> {
    match t {
        Target::Cast(r) => TargetV1::Cast {
            fid: r.fid,
            hash: r.hash.to_string(),
        },
        Target::Url(url) => TargetV1::Url(url),
    }
}

fn to_json(value: &impl Serialize) -> String {
    serde_json::to_string(value).expect("output structs always serialise")
}
