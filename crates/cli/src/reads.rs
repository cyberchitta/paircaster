//! `selfcast profile` and `selfcast feed`, and their `--json` output.
//!
//! The JSON shapes are an external format the agent skill depends on. Each is
//! version 1; a change to a shape bumps its `version`.

use selfcast_core::cast::{Cast, Target};
use selfcast_core::feed::{self, AppFilter, FeedItem};
use selfcast_core::node::PageRequest;
use selfcast_core::profile::Profile;
use selfcast_core::{Error, Node};
use serde::Serialize;

#[derive(Debug, clap::Args)]
pub struct FeedArgs {
    /// The account whose follows make up the feed.
    fid: u64,
    /// How many casts to show. Filtering by app looks only at each followed
    /// account's newest LIMIT casts, so it can show fewer.
    #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u32).range(1..=1000))]
    limit: u32,
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

impl FeedArgs {
    fn apps(&self) -> AppFilter {
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
///   "following": 10
/// }
/// ```
///
/// Each text field is `null` when the account has not set it, or set it
/// empty. `following` is how many accounts it follows.
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
///       "text": "…",
///       "mentions": [{ "fid": 1236, "position": 5 }],
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
/// `{ "url": … }`. `mentions[].position` is a byte offset into `text`.
/// `app_fid` is the account of the app whose key signed the cast, or `null`
/// when that key is no longer listed or its registration names no app.
#[derive(Serialize)]
struct FeedV1<'a> {
    version: u32,
    fid: u64,
    casts: Vec<CastV1<'a>>,
}

#[derive(Serialize)]
struct CastV1<'a> {
    hash: String,
    fid: u64,
    timestamp: String,
    text: &'a str,
    mentions: Vec<MentionV1>,
    embeds: Vec<TargetV1<'a>>,
    parent: Option<TargetV1<'a>>,
    signer: String,
    app_fid: Option<u64>,
}

#[derive(Serialize)]
struct MentionV1 {
    fid: u64,
    position: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum TargetV1<'a> {
    Cast { fid: u64, hash: String },
    Url(&'a str),
}

pub async fn profile(node: &Node, fid: u64, json: bool) -> Result<(), Error> {
    let user_data = node.user_data_by_fid(fid).await?;
    let following = node.all_follows_by_fid(fid).await?.len();
    let profile = Profile::assemble(fid, &user_data, following);
    if json {
        println!("{}", to_json(&profile_v1(&profile)));
    } else {
        print_profile(&profile);
    }
    Ok(())
}

pub async fn feed(node: &Node, args: &FeedArgs, json: bool) -> Result<(), Error> {
    let page = PageRequest {
        page_size: Some(args.limit),
        page_token: None,
    };
    let (mut casts, mut signers) = (Vec::new(), Vec::new());
    for followed in node.all_follows_by_fid(args.fid).await? {
        casts.extend(node.casts_by_fid(followed, &page).await?.items);
        signers.extend(node.signers_by_fid(followed).await?);
    }
    let items = feed::assemble(casts, &signers, &args.apps(), args.limit as usize);
    if json {
        println!("{}", to_json(&feed_v1(args.fid, &items)));
    } else {
        items.iter().for_each(print_item);
    }
    Ok(())
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
    }
}

fn feed_v1(fid: u64, items: &[FeedItem]) -> FeedV1<'_> {
    FeedV1 {
        version: 1,
        fid,
        casts: items.iter().map(|i| cast_v1(&i.cast, i.app_fid)).collect(),
    }
}

fn cast_v1(c: &Cast, app_fid: Option<u64>) -> CastV1<'_> {
    CastV1 {
        hash: c.id.hash.to_string(),
        fid: c.id.fid,
        timestamp: c.timestamp.to_string(),
        text: &c.text,
        mentions: c
            .mentions
            .iter()
            .map(|m| MentionV1 {
                fid: m.fid,
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
    println!("{name}  fid {}", p.fid);
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
        .for_each(|(label, v)| println!("  {label}: {v}"));
    println!("  following: {}", p.following);
}

fn print_item(item: &FeedItem) {
    let c = &item.cast;
    let app = item
        .app_fid
        .map_or_else(|| "unknown app".to_owned(), |f| format!("app {f}"));
    println!("{}  fid {}  {}  {app}", c.timestamp, c.id.fid, c.id.hash);
    c.text.lines().for_each(|l| println!("  {l}"));
    println!();
}

#[cfg(test)]
mod tests {
    use selfcast_core::cast::{Cast, CastRef, Mention, MessageHash, SignerKey, Target};
    use selfcast_core::feed::FeedItem;
    use selfcast_core::profile::Profile;
    use selfcast_core::proto::CastType;
    use selfcast_core::time::Timestamp;

    use super::{feed_v1, profile_v1, to_json};

    #[test]
    fn feed_json_is_version_1() {
        let hash = MessageHash::from_bytes(&[0xab; 20]).expect("20 bytes");
        let item = FeedItem {
            cast: Cast {
                id: CastRef { fid: 2, hash },
                timestamp: Timestamp::from_farcaster(0),
                kind: CastType::Cast,
                text: "hi @x".to_owned(),
                mentions: vec![Mention {
                    fid: 9,
                    position: 3,
                }],
                embeds: vec![Target::Url("https://e".to_owned())],
                parent: Some(Target::Cast(CastRef { fid: 17, hash })),
                signer: SignerKey::from_bytes(&[1; 32]).expect("32 bytes"),
            },
            app_fid: Some(9152),
        };
        let h = format!("0x{}", "ab".repeat(20));
        let signer = format!("0x{}", "01".repeat(32));
        let expected = format!(
            r#"{{"version":1,"fid":3,"casts":[{{"hash":"{h}","fid":2,"timestamp":"2021-01-01T00:00:00Z","text":"hi @x","mentions":[{{"fid":9,"position":3}}],"embeds":[{{"url":"https://e"}}],"parent":{{"cast":{{"fid":17,"hash":"{h}"}}}},"signer":"{signer}","app_fid":9152}}]}}"#
        );
        assert_eq!(to_json(&feed_v1(3, &[item])), expected);
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
        };
        assert_eq!(
            to_json(&profile_v1(&profile)),
            r#"{"version":1,"fid":3,"username":"alice","display_name":null,"bio":null,"pfp_url":null,"url":null,"location":null,"twitter":null,"github":null,"banner_url":null,"following":10}"#
        );
    }
}
