//! `selfcast-core`'s reads for the viewer page and pages built on it. Each
//! function is named after a CLI read, takes what the CLI takes, and returns
//! the JSON that `selfcast <read> --json` prints (`selfcast_core::output`),
//! so the skill that documents the CLI documents these too. `node` is the
//! node's URL; `now` is the page's clock in Unix seconds, used as the CLI
//! uses its own: casts dated more than ten minutes past it are left out.

use std::collections::BTreeSet;

use wasm_bindgen::prelude::*;

use selfcast_core::Node;
use selfcast_core::account::Account;
use selfcast_core::cast::{CastRef, ChannelUrl};
use selfcast_core::event::Event;
use selfcast_core::feed::AppFilter;
use selfcast_core::node::reads::EventRead;
use selfcast_core::output;
use selfcast_core::profile::FOLLOWER_COUNT_CAP;
use selfcast_core::time::Timestamp;

/// The most casts or replies one read returns, as in the CLI.
const MAX_CASTS: u32 = 1000;

/// The most followers or follows one read returns, as in the CLI.
const MAX_FOLLOWS: u32 = 10_000;

/// `selfcast profile <account> [--all-followers] --json`.
#[wasm_bindgen]
pub async fn profile(
    node: String,
    account: String,
    all_followers: bool,
) -> Result<String, JsError> {
    let node = Node::new(&node)?;
    let fid = node.resolve(&account.parse()?).await?;
    let cap = (!all_followers).then_some(FOLLOWER_COUNT_CAP);
    Ok(output::profile_json(&node.profile(fid, cap).await?))
}

/// `selfcast feed <account> --limit <limit> [--include-app …] [--exclude-app …] --json`.
#[wasm_bindgen]
pub async fn feed(
    node: String,
    account: String,
    limit: u32,
    include_app: Vec<f64>,
    exclude_app: Vec<f64>,
    now: f64,
) -> Result<String, JsError> {
    let node = Node::new(&node)?;
    let fid = node.resolve(&account.parse()?).await?;
    let apps = app_filter(&include_app, &exclude_app)?;
    let items = node
        .feed(fid, limit_of(limit, MAX_CASTS)?, &apps, time(now)?)
        .await?;
    let users = node.accounts_of(&items).await?;
    Ok(output::feed_json(fid, &items, &users))
}

/// `selfcast casts <account>,… --limit <limit> [app filter] --json`.
#[wasm_bindgen]
pub async fn casts(
    node: String,
    accounts: Vec<String>,
    limit: u32,
    include_app: Vec<f64>,
    exclude_app: Vec<f64>,
    now: f64,
) -> Result<String, JsError> {
    let node = Node::new(&node)?;
    let accounts = parse_accounts(&accounts)?;
    let fids = node.resolve_all(&accounts).await?;
    let apps = app_filter(&include_app, &exclude_app)?;
    let items = node
        .newest_casts(fids.clone(), limit_of(limit, MAX_CASTS)?, &apps, time(now)?)
        .await?;
    let users = node.accounts_of(&items).await?;
    Ok(output::casts_json(&fids, &items, &users))
}

/// `selfcast channel <url> --limit <limit> [app filter] --json`.
#[wasm_bindgen]
pub async fn channel(
    node: String,
    url: String,
    limit: u32,
    include_app: Vec<f64>,
    exclude_app: Vec<f64>,
    now: f64,
) -> Result<String, JsError> {
    let node = Node::new(&node)?;
    let channel: ChannelUrl = url.parse()?;
    let apps = app_filter(&include_app, &exclude_app)?;
    let items = node
        .channel(&channel, limit_of(limit, MAX_CASTS)?, &apps, time(now)?)
        .await?;
    let users = node.accounts_of(&items).await?;
    Ok(output::channel_json(&channel, &items, &users))
}

/// `selfcast addresses <account> --json`.
#[wasm_bindgen]
pub async fn addresses(node: String, account: String) -> Result<String, JsError> {
    let node = Node::new(&node)?;
    let fid = node.resolve(&account.parse()?).await?;
    Ok(output::addresses_json(fid, &node.addresses(fid).await?))
}

/// `selfcast followers <account> --limit <limit> --json`.
#[wasm_bindgen]
pub async fn followers(node: String, account: String, limit: u32) -> Result<String, JsError> {
    follows(node, account, true, limit).await
}

/// `selfcast following <account> --limit <limit> --json`.
#[wasm_bindgen]
pub async fn following(node: String, account: String, limit: u32) -> Result<String, JsError> {
    follows(node, account, false, limit).await
}

async fn follows(
    node: String,
    account: String,
    followers: bool,
    limit: u32,
) -> Result<String, JsError> {
    let node = Node::new(&node)?;
    let fid = node.resolve(&account.parse()?).await?;
    let limit = limit_of(limit, MAX_FOLLOWS)?;
    let (follows, users) = node.follow_list(fid, followers, limit).await?;
    Ok(output::follows_json(fid, followers, &follows, &users))
}

/// `selfcast replies <cast> --limit <limit> [app filter] --json`.
#[wasm_bindgen]
pub async fn replies(
    node: String,
    cast: String,
    limit: u32,
    include_app: Vec<f64>,
    exclude_app: Vec<f64>,
    now: f64,
) -> Result<String, JsError> {
    let node = Node::new(&node)?;
    let id: CastRef = cast.parse()?;
    let apps = app_filter(&include_app, &exclude_app)?;
    let items = node
        .newest_replies(&id, limit_of(limit, MAX_CASTS)?, &apps, time(now)?)
        .await?;
    let users = node.accounts_of(&items).await?;
    Ok(output::replies_json(&id, &items, &users))
}

/// `selfcast thread <cast> --limit <limit> --json`.
#[wasm_bindgen]
pub async fn thread(node: String, cast: String, limit: u32, now: f64) -> Result<String, JsError> {
    let node = Node::new(&node)?;
    let id: CastRef = cast.parse()?;
    let limit = limit_of(limit, MAX_CASTS)? as usize;
    let (thread, users) = node.thread(&id, limit, time(now)?).await?;
    Ok(output::thread_json(&thread, &users))
}

/// `selfcast reactions <cast> --json`.
#[wasm_bindgen]
pub async fn reactions(node: String, cast: String) -> Result<String, JsError> {
    let node = Node::new(&node)?;
    let id: CastRef = cast.parse()?;
    let (reactions, users) = node.reactions(&id).await?;
    Ok(output::reactions_json(&id, &reactions, &users))
}

/// `selfcast events [--since …] [--until …] [--shard …] --json`, a page at
/// a time: [`Events::next`] gives the next page.
#[wasm_bindgen]
pub async fn events(
    node: String,
    shard: Option<u32>,
    since: Option<String>,
    until: Option<String>,
) -> Result<Events, JsError> {
    let node = Node::new(&node)?;
    let since = since.map(|t| t.parse()).transpose()?;
    let until = until.map(|t| t.parse()).transpose()?;
    let read = node.event_read(shard, since, until).await?;
    Ok(Events { node, read })
}

/// A read of events under way, from [`events`].
#[wasm_bindgen]
pub struct Events {
    node: Node,
    read: EventRead,
}

#[wasm_bindgen]
impl Events {
    /// The next page as a JSON array of what `selfcast events --json`
    /// prints, one element per line; it may be empty. `undefined` once
    /// every shard is read.
    pub async fn next(&mut self) -> Result<Option<String>, JsError> {
        let page = self.node.next_events(&mut self.read).await?;
        Ok(page.map(|events| events_json(&events)))
    }
}

fn events_json(events: &[Event]) -> String {
    let lines: Vec<String> = events.iter().map(output::event_json).collect();
    format!("[{}]", lines.join(","))
}

fn parse_accounts(accounts: &[String]) -> Result<Vec<Account>, JsError> {
    match accounts.is_empty() {
        true => Err(JsError::new("casts needs at least one account")),
        false => Ok(accounts
            .iter()
            .map(|a| a.parse())
            .collect::<Result<_, _>>()?),
    }
}

/// The filter the CLI's `--include-app` and `--exclude-app` make. Both at
/// once is refused, as the CLI refuses it.
fn app_filter(include: &[f64], exclude: &[f64]) -> Result<AppFilter, JsError> {
    let fids = |list: &[f64]| -> Result<BTreeSet<u64>, JsError> {
        list.iter().map(|&f| fid_of(f)).collect()
    };
    match (include.is_empty(), exclude.is_empty()) {
        (false, false) => Err(JsError::new(
            "includeApp and excludeApp cannot both be given",
        )),
        (false, true) => Ok(AppFilter::Only(fids(include)?)),
        (true, false) => Ok(AppFilter::Not(fids(exclude)?)),
        (true, true) => Ok(AppFilter::Any),
    }
}

fn fid_of(n: f64) -> Result<u64, JsError> {
    whole(n).ok_or_else(|| JsError::new(&format!("{n} is not a FID")))
}

/// `n` as a whole number from 0 up to the largest a JavaScript number
/// holds exactly.
fn whole(n: f64) -> Option<u64> {
    const MAX_EXACT: f64 = 9_007_199_254_740_991.0;
    (n.fract() == 0.0 && (0.0..=MAX_EXACT).contains(&n)).then_some(n as u64)
}

fn limit_of(limit: u32, max: u32) -> Result<u32, JsError> {
    match (1..=max).contains(&limit) {
        true => Ok(limit),
        false => Err(JsError::new(&format!(
            "limit must be 1 to {max}, not {limit}"
        ))),
    }
}

fn time(now: f64) -> Result<Timestamp, JsError> {
    whole(now)
        .map(Timestamp::from_unix)
        .ok_or_else(|| JsError::new(&format!("{now} is not a time in Unix seconds")))
}
