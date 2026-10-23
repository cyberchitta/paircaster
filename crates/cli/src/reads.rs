//! `selfcast profile`, `selfcast feed`, `selfcast casts`,
//! `selfcast replies`, `selfcast thread` and `selfcast reactions`. The reads and their `--json`
//! shapes live in the core (`node::reads`, `output`), shared with the viewer.

use selfcast_core::account::Account;
use selfcast_core::cast::CastRef;
use selfcast_core::cast::Reaction;
use selfcast_core::feed::{self, AppFilter, FeedItem};
use selfcast_core::node::PageRequest;
use selfcast_core::node::reads::MAX_ANCESTORS;
use selfcast_core::output;
use selfcast_core::profile::{FOLLOWER_COUNT_CAP, Followers, Names, Profile, names_of};
use selfcast_core::reaction::CastReactions;
use selfcast_core::thread::{ChainTop, Thread};
use selfcast_core::{Error, Node};

use crate::clock;

#[derive(Debug, clap::Args)]
pub struct FeedArgs {
    /// The account whose follows make up the feed, as a FID or a username.
    #[arg(value_name = "ACCOUNT")]
    account: Account,
    #[command(flatten)]
    filter: CastFilter,
}

#[derive(Debug, clap::Args)]
pub struct CastsArgs {
    /// The accounts whose casts to show, as FIDs or usernames. Repeat or
    /// separate with commas.
    #[arg(value_name = "ACCOUNT", required = true, value_delimiter = ',')]
    accounts: Vec<Account>,
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
pub struct ThreadArgs {
    /// The cast to show, as FID:HASH.
    #[arg(value_name = "FID:HASH")]
    cast: CastRef,
    /// How many replies to show: the newest LIMIT, oldest first.
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..=1000))]
    limit: u32,
}

#[derive(Debug, clap::Args)]
pub struct ReactionsArgs {
    /// The cast whose likes and recasts to show, as FID:HASH.
    #[arg(value_name = "FID:HASH")]
    cast: CastRef,
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

pub async fn profile(
    node: &Node,
    account: &Account,
    all_followers: bool,
    json: bool,
) -> Result<(), Error> {
    let fid = node.resolve(account).await?;
    if !node.is_registered(fid).await? {
        return Err(Error::NotRegistered(fid));
    }
    let user_data = node.user_data_by_fid(fid).await?;
    let following = node.all_follows_by_fid(fid).await?.len();
    let cap = (!all_followers).then_some(FOLLOWER_COUNT_CAP);
    let followers = node.follower_count(fid, cap).await?;
    let profile = Profile::assemble(fid, &user_data, following, followers);
    if json {
        crate::out::line(output::profile_json(&profile));
    } else {
        print_profile(&profile);
    }
    Ok(())
}

pub async fn feed(node: &Node, args: &FeedArgs, json: bool) -> Result<(), Error> {
    let fid = node.resolve(&args.account).await?;
    let follows = node.all_follows_by_fid(fid).await?;
    let items = newest_casts(node, follows, &args.filter).await?;
    let names = node.mention_names(&items).await?;
    if json {
        crate::out::line(output::feed_json(fid, &items, &names));
    } else {
        items.iter().for_each(|i| print_item(i, &names));
    }
    Ok(())
}

pub async fn casts(node: &Node, args: &CastsArgs, json: bool) -> Result<(), Error> {
    let fids = node.resolve_all(&args.accounts).await?;
    let items = newest_casts(node, fids.clone(), &args.filter).await?;
    let names = node.mention_names(&items).await?;
    if json {
        crate::out::line(output::casts_json(&fids, &items, &names));
    } else {
        items.iter().for_each(|i| print_item(i, &names));
    }
    Ok(())
}

pub async fn replies(node: &Node, args: &RepliesArgs, json: bool) -> Result<(), Error> {
    let casts = node.replies(&args.cast).await?;
    let signers = node
        .signers_of(casts.iter().map(|c| c.id.fid).collect())
        .await?;
    let limit = args.limit as usize;
    let mut items = feed::assemble(casts, &signers, &args.apps.filter(), clock::now(), limit);
    items.reverse();
    let names = node.mention_names(&items).await?;
    if json {
        crate::out::line(output::replies_json(&args.cast, &items, &names));
    } else {
        items.iter().for_each(|i| print_item(i, &names));
    }
    Ok(())
}

pub async fn thread(node: &Node, args: &ThreadArgs, json: bool) -> Result<(), Error> {
    let limit = args.limit as usize;
    let (thread, users) = node.thread(&args.cast, limit, clock::now()).await?;
    if json {
        crate::out::line(output::thread_json(&thread, &users));
    } else {
        print_thread(&thread, &names_of(&users));
    }
    Ok(())
}

pub async fn reactions(node: &Node, args: &ReactionsArgs, json: bool) -> Result<(), Error> {
    let (reactions, users) = node.reactions(&args.cast).await?;
    if json {
        crate::out::line(output::reactions_json(&args.cast, &reactions, &users));
    } else {
        print_reactions(&reactions, &names_of(&users));
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
    let (casts, signers) = node.casts_and_signers(fids, &page).await?;
    let limit = filter.limit as usize;
    Ok(feed::assemble(
        casts,
        &signers,
        &filter.apps.filter(),
        clock::now(),
        limit,
    ))
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

fn print_thread(t: &Thread, names: &Names) {
    match t.top {
        ChainTop::Root => {}
        ChainTop::Missing(r) => crate::out::line(format_args!(
            "(replies to {r}, which this node does not hold)\n"
        )),
        ChainTop::Cut(r) => crate::out::line(format_args!(
            "(replies to {r}; only the {MAX_ANCESTORS} casts below it are shown)\n"
        )),
    }
    t.ancestors.iter().for_each(|i| print_item(i, names));
    crate::out::line("--- the cast ---");
    print_item(&t.cast, names);
    crate::out::line(format_args!("--- {} replies ---", t.replies.len()));
    t.replies.iter().for_each(|i| print_item(i, names));
}

fn print_reactions(r: &CastReactions, names: &Names) {
    crate::out::line(format_args!(
        "{} likes, {} recasts",
        r.likes.len(),
        r.recasts.len()
    ));
    [("likes", &r.likes), ("recasts", &r.recasts)]
        .iter()
        .filter(|(_, list)| !list.is_empty())
        .for_each(|(label, list)| {
            crate::out::line(format_args!("\n--- {label} ---"));
            list.iter().for_each(|x| print_reaction(x, names));
        });
}

fn print_reaction(r: &Reaction, names: &Names) {
    let name = names
        .get(&r.fid)
        .map_or_else(|| "(no username)".to_owned(), |n| format!("@{n}"));
    crate::out::line(format_args!("{}  {name}  fid {}", r.timestamp, r.fid));
}
