//! `selfcast profile`, `selfcast feed`, `selfcast casts` and
//! `selfcast replies`, and their
//! `--json` output.
//!
//! The JSON shapes are an external format the agent skill depends on. Each is
//! version 1; a change to a shape bumps its `version`.

use selfcast_core::cast::{Cast, CastRef, Target};
use selfcast_core::feed::{self, AppFilter, FeedItem};
use selfcast_core::node::PageRequest;
use selfcast_core::profile::{self, FOLLOWER_COUNT_CAP, Followers, Profile};
use selfcast_core::signer::Signer;
use selfcast_core::{Error, Node};
use std::collections::{BTreeSet, HashMap};

use serde::Serialize;
use tokio::task::JoinSet;

use crate::clock;

/// How many followed accounts the feed fetches at once.
const FETCH_AT_ONCE: usize = 8;

#[derive(Debug, clap::Args)]
pub struct FeedArgs {
    /// The account whose follows make up the feed.
    fid: u64,
    #[command(flatten)]
    filter: CastFilter,
}

#[derive(Debug, clap::Args)]
pub struct CastsArgs {
    /// The accounts whose casts to show. Repeat or separate with commas.
    #[arg(required = true, value_delimiter = ',')]
    fids: Vec<u64>,
    #[command(flatten)]
    filter: CastFilter,
}

#[derive(Debug, clap::Args)]
pub struct RepliesArgs {
    /// The cast whose replies to show, as FID:HASH.
    #[arg(value_name = "FID:HASH")]
    cast: CastRef,
    /// How many replies to show: the newest LIMIT, oldest first. Every
    /// reply is read before filtering, so filtering by app does not shorten
    /// the list below LIMIT while more replies match.
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..=1000))]
    limit: u32,
    #[command(flatten)]
    apps: AppArgs,
}

#[derive(Debug, clap::Args)]
struct CastFilter {
    /// How many casts to show. Filtering by app looks only at each
    /// account's newest LIMIT casts, so it can show fewer.
    #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u32).range(1..=1000))]
    limit: u32,
    #[command(flatten)]
    apps: AppArgs,
}

#[derive(Debug, clap::Args)]
struct AppArgs {
    /// Only casts posted from these apps, by the app's FID. Repeat the flag
    /// or separate FIDs with commas. Casts from an unknown app are left out.
    #[arg(
        long,
        value_name = "FID",
        value_delimiter = ',',
        conflicts_with = "exclude_app"
    )]
    include_app: Vec<u64>,
    /// Every cast except those posted from these apps, by the app's FID.
    /// Casts from an unknown app are kept.
    #[arg(long, value_name = "FID", value_delimiter = ',')]
    exclude_app: Vec<u64>,
}

impl AppArgs {
    fn filter(&self) -> AppFilter {
        match (self.include_app.is_empty(), self.exclude_app.is_empty()) {
            (false, _) => AppFilter::Only(self.include_app.iter().copied().collect()),
            (true, false) => AppFilter::Not(self.exclude_app.iter().copied().collect()),
            (true, true) => AppFilter::Any,
        }
    }
}

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
#[derive(Serialize)]
struct FeedV1<'a> {
    version: u32,
    fid: u64,
    casts: Vec<CastV1<'a>>,
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
#[derive(Serialize)]
struct CastsV1<'a> {
    version: u32,
    fids: &'a [u64],
    casts: Vec<CastV1<'a>>,
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

pub async fn profile(node: &Node, fid: u64, all_followers: bool, json: bool) -> Result<(), Error> {
    if !node.is_registered(fid).await? {
        return Err(Error::NotRegistered(fid));
    }
    let user_data = node.user_data_by_fid(fid).await?;
    let following = node.all_follows_by_fid(fid).await?.len();
    let cap = (!all_followers).then_some(FOLLOWER_COUNT_CAP);
    let followers = node.follower_count(fid, cap).await?;
    let profile = Profile::assemble(fid, &user_data, following, followers);
    if json {
        crate::out::line(format_args!("{}", to_json(&profile_v1(&profile))));
    } else {
        print_profile(&profile);
    }
    Ok(())
}

pub async fn feed(node: &Node, args: &FeedArgs, json: bool) -> Result<(), Error> {
    let follows = node.all_follows_by_fid(args.fid).await?;
    let items = newest_casts(node, follows, &args.filter).await?;
    let names = mention_names(node, &items).await?;
    if json {
        crate::out::line(format_args!(
            "{}",
            to_json(&feed_v1(args.fid, &items, &names))
        ));
    } else {
        items.iter().for_each(|i| print_item(i, &names));
    }
    Ok(())
}

pub async fn casts(node: &Node, args: &CastsArgs, json: bool) -> Result<(), Error> {
    let items = newest_casts(node, args.fids.clone(), &args.filter).await?;
    let names = mention_names(node, &items).await?;
    if json {
        crate::out::line(format_args!(
            "{}",
            to_json(&casts_v1(&args.fids, &items, &names))
        ));
    } else {
        items.iter().for_each(|i| print_item(i, &names));
    }
    Ok(())
}

pub async fn replies(node: &Node, args: &RepliesArgs, json: bool) -> Result<(), Error> {
    let casts = node.replies(&args.cast).await?;
    let authors: BTreeSet<u64> = casts.iter().map(|c| c.id.fid).collect();
    let signers = fetch_signers(node, authors).await?;
    let limit = args.limit as usize;
    let mut items = feed::assemble(casts, &signers, &args.apps.filter(), clock::now(), limit);
    items.reverse();
    let names = mention_names(node, &items).await?;
    if json {
        crate::out::line(format_args!(
            "{}",
            to_json(&replies_v1(&args.cast, &items, &names))
        ));
    } else {
        items.iter().for_each(|i| print_item(i, &names));
    }
    Ok(())
}

/// The newest casts across `fids`, after the filter.
async fn newest_casts(
    node: &Node,
    fids: Vec<u64>,
    filter: &CastFilter,
) -> Result<Vec<FeedItem>, Error> {
    let page = PageRequest {
        page_size: Some(filter.limit),
        page_token: None,
    };
    let (casts, signers) = fetch_all(node, fids, page).await?;
    let limit = filter.limit as usize;
    Ok(feed::assemble(
        casts,
        &signers,
        &filter.apps.filter(),
        clock::now(),
        limit,
    ))
}

/// Each account's casts and app keys.
async fn fetch_all(
    node: &Node,
    follows: Vec<u64>,
    page: PageRequest,
) -> Result<(Vec<Cast>, Vec<Signer>), Error> {
    let fetched = per_account(follows, |fid| {
        let (node, page) = (node.clone(), page.clone());
        async move {
            let (casts, signers) =
                tokio::try_join!(node.casts_by_fid(fid, &page), node.signers_by_fid(fid))?;
            Ok((casts.items, signers))
        }
    })
    .await?;
    let (casts, signers): (Vec<_>, Vec<_>) = fetched.into_iter().unzip();
    Ok((casts.concat(), signers.concat()))
}

/// The app keys of each of `fids`.
async fn fetch_signers(node: &Node, fids: BTreeSet<u64>) -> Result<Vec<Signer>, Error> {
    let fetched = per_account(fids, |fid| {
        let node = node.clone();
        async move { node.signers_by_fid(fid).await }
    })
    .await?;
    Ok(fetched.concat())
}

/// The username of each account mentioned in `items` that has one.
async fn mention_names(node: &Node, items: &[FeedItem]) -> Result<HashMap<u64, String>, Error> {
    let fids: BTreeSet<u64> = items
        .iter()
        .flat_map(|i| i.cast.mentions.iter().map(|m| m.fid))
        .collect();
    let fetched = per_account(fids, |fid| {
        let node = node.clone();
        async move { Ok((fid, profile::username(&node.user_data_by_fid(fid).await?))) }
    })
    .await?;
    Ok(fetched
        .into_iter()
        .filter_map(|(fid, name)| name.map(|n| (fid, n)))
        .collect())
}

/// `fetch` for each of `fids`, `FETCH_AT_ONCE` at a time, in no set order.
/// The first error stops the rest.
async fn per_account<T, F, Fut>(
    fids: impl IntoIterator<Item = u64>,
    fetch: F,
) -> Result<Vec<T>, Error>
where
    T: Send + 'static,
    F: Fn(u64) -> Fut,
    Fut: Future<Output = Result<T, Error>> + Send + 'static,
{
    let mut pending = fids.into_iter();
    let mut running = JoinSet::new();
    let mut out = Vec::new();
    loop {
        while running.len() < FETCH_AT_ONCE
            && let Some(fid) = pending.next()
        {
            running.spawn(fetch(fid));
        }
        let Some(done) = running.join_next().await else {
            return Ok(out);
        };
        out.push(done.expect("a node fetch does not panic")?);
    }
}

fn profile_v1(p: &Profile) -> ProfileV1<'_> {
    ProfileV1 {
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
    }
}

fn feed_v1<'a>(fid: u64, items: &'a [FeedItem], names: &'a Names) -> FeedV1<'a> {
    FeedV1 {
        version: 1,
        fid,
        casts: casts_json(items, names),
    }
}

fn casts_v1<'a>(fids: &'a [u64], items: &'a [FeedItem], names: &'a Names) -> CastsV1<'a> {
    CastsV1 {
        version: 1,
        fids,
        casts: casts_json(items, names),
    }
}

fn replies_v1<'a>(parent: &CastRef, items: &'a [FeedItem], names: &'a Names) -> RepliesV1<'a> {
    RepliesV1 {
        version: 1,
        parent: ParentV1 {
            fid: parent.fid,
            hash: parent.hash.to_string(),
        },
        casts: casts_json(items, names),
    }
}

/// Usernames of mentioned accounts, by FID; an account with none is absent.
type Names = HashMap<u64, String>;

fn casts_json<'a>(items: &'a [FeedItem], names: &'a Names) -> Vec<CastV1<'a>> {
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

fn print_profile(p: &Profile) {
    let name = p.username.as_deref().unwrap_or("(no username)");
    crate::out::line(format_args!("{name}  fid {}", p.fid));
    let fields = [
        ("name", &p.display_name),
        ("bio", &p.bio),
        ("url", &p.url),
        ("location", &p.location),
        ("twitter", &p.twitter),
        ("github", &p.github),
    ];
    fields
        .iter()
        .filter_map(|(label, value)| value.as_deref().map(|v| (label, v)))
        .for_each(|(label, v)| crate::out::line(format_args!("  {label}: {v}")));
    crate::out::line(format_args!("  following: {}", p.following));
    crate::out::line(format_args!("  followers: {}", followers_line(p.followers)));
}

fn followers_line(f: Followers) -> String {
    match f {
        Followers::Exact(n) => n.to_string(),
        Followers::AtLeast(n) => {
            format!("at least {n} (counting stopped; --all-followers counts them all)")
        }
    }
}

fn print_item(item: &FeedItem, names: &Names) {
    let c = &item.cast;
    let app = item
        .app_fid
        .map_or_else(|| "unknown app".to_owned(), |f| format!("app {f}"));
    crate::out::line(format_args!(
        "{}  fid {}  {}  {app}",
        c.timestamp, c.id.fid, c.id.hash
    ));
    c.display_text(names)
        .unwrap_or_else(|| c.text.clone())
        .lines()
        .for_each(|l| crate::out::line(format_args!("  {l}")));
    crate::out::line("");
}

#[cfg(test)]
mod tests {
    use selfcast_core::cast::{Cast, CastRef, Mention, MessageHash, SignerKey, Target};
    use selfcast_core::feed::FeedItem;
    use selfcast_core::profile::{Followers, Profile};
    use selfcast_core::proto::CastType;
    use selfcast_core::time::Timestamp;

    use super::{casts_v1, feed_v1, profile_v1, replies_v1, to_json};

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

    fn names() -> super::Names {
        [(9, "alice".to_owned())].into()
    }

    fn cast_json() -> String {
        let h = format!("0x{}", "ab".repeat(20));
        let signer = format!("0x{}", "01".repeat(32));
        format!(
            r#"{{"hash":"{h}","fid":2,"timestamp":"2021-01-01T00:00:00Z","text":"hi ","display_text":"hi @alice","mentions":[{{"fid":9,"username":"alice","position":3}}],"embeds":[{{"url":"https://e"}}],"parent":{{"cast":{{"fid":17,"hash":"{h}"}}}},"signer":"{signer}","app_fid":9152}}"#
        )
    }

    #[test]
    fn feed_json_is_version_1() {
        let expected = format!(r#"{{"version":1,"fid":3,"casts":[{}]}}"#, cast_json());
        assert_eq!(to_json(&feed_v1(3, &[item()], &names())), expected);
    }

    #[test]
    fn casts_json_is_version_1() {
        let expected = format!(r#"{{"version":1,"fids":[2,5],"casts":[{}]}}"#, cast_json());
        assert_eq!(to_json(&casts_v1(&[2, 5], &[item()], &names())), expected);
    }

    #[test]
    fn replies_json_is_version_1() {
        let parent = CastRef {
            fid: 37,
            hash: MessageHash::from_bytes(&[0xcd; 20]).expect("20 bytes"),
        };
        let expected = format!(
            r#"{{"version":1,"parent":{{"fid":37,"hash":"0x{}"}},"casts":[{}]}}"#,
            "cd".repeat(20),
            cast_json()
        );
        assert_eq!(to_json(&replies_v1(&parent, &[item()], &names())), expected);
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
            to_json(&profile_v1(&profile)),
            r#"{"version":1,"fid":3,"username":"alice","display_name":null,"bio":null,"pfp_url":null,"url":null,"location":null,"twitter":null,"github":null,"banner_url":null,"following":10,"followers":10002,"followers_exact":false}"#
        );
    }
}
