//! The JSON the reads produce: `selfcast <read> --json`, and the same text
//! from the viewer's WebAssembly. An external format the agent skill, the
//! viewer and pages built on it depend on. Each shape is version 1; a change
//! to a shape bumps its `version`.

use serde::Serialize;

use crate::cast::{Cast, CastRef, Reaction, Target};
use crate::feed::FeedItem;
use crate::profile::{self, Names, Profile, Users};
use crate::reaction::CastReactions;
use crate::thread::{ChainTop, Thread};

/// `selfcast profile <fid> --json`, version 1:
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

/// `selfcast feed <fid> --json`, version 1:
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
///   ]
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
pub fn feed_json(fid: u64, items: &[FeedItem], names: &Names) -> String {
    to_json(&FeedV1 {
        version: 1,
        fid,
        casts: casts_v1(items, names),
    })
}

/// `selfcast casts <fid>... --json`, version 1:
///
/// ```json
/// { "version": 1, "fids": [2, 3], "casts": [ … ] }
/// ```
///
/// `fids` are the accounts asked for, in the order given. Each element of
/// `casts` has the same shape as in `feed`, newest first, after any app
/// filter.
pub fn casts_json(fids: &[u64], items: &[FeedItem], names: &Names) -> String {
    to_json(&CastsV1 {
        version: 1,
        fids,
        casts: casts_v1(items, names),
    })
}

/// `selfcast replies <FID:HASH> --json`, version 1:
///
/// ```json
/// { "version": 1, "parent": { "fid": 37, "hash": "0x…" }, "casts": [ … ] }
/// ```
///
/// `parent` is the cast asked about. Each element of `casts` has the same
/// shape as in `feed`, but oldest first: the newest `--limit` replies after
/// any app filter, in the order they were posted.
pub fn replies_json(parent: &CastRef, items: &[FeedItem], names: &Names) -> String {
    to_json(&RepliesV1 {
        version: 1,
        parent: parent_v1(parent),
        casts: casts_v1(items, names),
    })
}

/// `selfcast thread <FID:HASH> --json`, version 1:
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

/// `selfcast reactions <FID:HASH> --json`, version 1:
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
        likes: r.likes.iter().map(reaction_v1).collect(),
        recasts: r.recasts.iter().map(reaction_v1).collect(),
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
}

#[derive(Serialize)]
struct CastsV1<'a> {
    version: u32,
    fids: &'a [u64],
    casts: Vec<CastV1<'a>>,
}

#[derive(Serialize)]
struct RepliesV1<'a> {
    version: u32,
    parent: ParentV1,
    casts: Vec<CastV1<'a>>,
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
struct ReactionsV1 {
    version: u32,
    cast: ParentV1,
    likes: Vec<ReactionV1>,
    recasts: Vec<ReactionV1>,
    accounts: Vec<AccountV1>,
}

#[derive(Serialize)]
struct ReactionV1 {
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

fn reaction_v1(r: &Reaction) -> ReactionV1 {
    ReactionV1 {
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
