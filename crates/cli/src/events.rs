//! `selfcast events`: what the node's shards merged, block by block.

use selfcast_core::event::{Change, Event, EventBody, EventWalk};
use selfcast_core::node::ShardSync;
use selfcast_core::output;
use selfcast_core::time::Timestamp;
use selfcast_core::{Error, Node};

/// How many events one request asks for.
const PAGE_SIZE: u32 = 10_000;

#[derive(Debug, clap::Args)]
pub struct EventsArgs {
    /// Only events in blocks from this time on, UTC: 2026-10-22 or
    /// 2026-10-22T06:00:00Z. By default, from the oldest the node holds.
    #[arg(long, value_name = "TIME")]
    since: Option<Timestamp>,
    /// Only events in blocks before this time, UTC. By default, up to the
    /// newest.
    #[arg(long, value_name = "TIME")]
    until: Option<Timestamp>,
    /// Only this shard. By default every shard with events, one after
    /// the other.
    #[arg(long, value_name = "N")]
    shard: Option<u32>,
}

pub async fn run(node: &Node, args: &EventsArgs, json: bool) -> Result<(), Error> {
    let state = node.sync_state().await?;
    let shards: Vec<&ShardSync> = state
        .shards
        .iter()
        .filter(|s| s.id != 0 && args.shard.is_none_or(|n| n == s.id))
        .collect();
    if let Some(n) = args.shard
        && shards.is_empty()
    {
        return Err(Error::NoShard(n));
    }
    for shard in shards {
        walk_shard(node, shard, args, json).await?;
    }
    Ok(())
}

async fn walk_shard(
    node: &Node,
    shard: &ShardSync,
    args: &EventsArgs,
    json: bool,
) -> Result<(), Error> {
    let start = match args.since {
        Some(t) => node.first_event_since(shard.id, t, shard.height).await?,
        None => 0,
    };
    let mut walk = EventWalk::new(shard.id, start, args.since, args.until);
    while let Some(from) = walk.next_id() {
        let page = node.events(shard.id, from, PAGE_SIZE).await?;
        walk.add_page(page)?
            .iter()
            .for_each(|e| print_event(e, json));
    }
    Ok(())
}

fn print_event(e: &Event, json: bool) {
    match json {
        true => crate::out::line(output::event_json(e)),
        false => crate::out::line(format_args!(
            "{}  shard {}  {}",
            e.block_time,
            e.shard,
            describe(&e.body)
        )),
    }
}

fn describe(body: &EventBody) -> String {
    match body {
        EventBody::Message {
            change,
            kind,
            fid,
            hash,
            ..
        } => format!("{} {kind}  fid {fid}  {hash}", change_name(*change)),
        EventBody::UsernameProof { fid, name, kind } => {
            format!("username_proof {kind}  fid {fid}  {name}")
        }
        EventBody::OnChain { fid, kind } => format!("onchain {kind}  fid {fid}"),
        EventBody::Other { node_type } => format!("other {node_type}"),
    }
}

fn change_name(c: Change) -> &'static str {
    match c {
        Change::Merge => "merge",
        Change::Prune => "prune",
        Change::Revoke => "revoke",
    }
}
