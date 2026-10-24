//! `paircaster events`: what the node's shards merged, block by block.

use paircaster_core::cast::Target;
use paircaster_core::event::{Change, Content, Event, EventBody};
use paircaster_core::output;
use paircaster_core::time::Timestamp;
use paircaster_core::{Error, Node};

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
    let mut read = node.event_read(args.shard, args.since, args.until).await?;
    while let Some(events) = node.next_events(&mut read).await? {
        events.iter().for_each(|e| print_event(e, json));
    }
    Ok(())
}

fn print_event(e: &Event, json: bool) {
    match json {
        true => crate::out::line(output::event_json(e)),
        false => {
            crate::out::line(format_args!(
                "{}  shard {}  {}",
                e.block_time,
                e.shard,
                describe(&e.body)
            ));
            print_content(&e.body);
        }
    }
}

/// What the message says, indented under its event: a cast's text, as the
/// cast reads give it but without the mentioned names, which the event
/// does not look up; what a reaction is to; whom a link points at.
fn print_content(body: &EventBody) {
    let EventBody::Message {
        content: Some(content),
        ..
    } = body
    else {
        return;
    };
    match content {
        Content::Cast(cast) => cast
            .text
            .lines()
            .for_each(|l| crate::out::line(format_args!("    {l}"))),
        Content::Reaction { kind, target } => crate::out::line(format_args!(
            "    {} {}",
            output::reaction_name(*kind),
            target_text(target)
        )),
        Content::Link { kind, target_fid } => {
            crate::out::line(format_args!("    {kind} fid {target_fid}"))
        }
    }
}

fn target_text(target: &Target) -> String {
    match target {
        Target::Cast(r) => r.to_string(),
        Target::Url(url) => url.clone(),
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
