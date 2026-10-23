mod clock;
mod control;
mod events;
mod invocation;
mod key_add;
mod keystore;
mod out;
mod outbox;
mod reads;
mod snapshot;
mod status;
mod tty;
mod viewer;
mod write;

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use selfcast_core::Node;
use selfcast_core::account::Account;

/// Where a node's HTTP API answers when `--node` is not given: Snapchain's
/// own compose file publishes it on this port.
const DEFAULT_NODE: &str = "http://localhost:3381";

/// Farcaster from your own Snapchain node.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Node HTTP API base URL [default: http://localhost:3381, the node on
    /// this machine].
    #[arg(long, global = true)]
    node: Option<String>,
    /// Print machine-readable JSON instead of human output.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show the node's version and sync status.
    Info,
    /// Show an account's profile.
    Profile {
        /// The account, as a FID or a username (dwr, vitalik.eth). Digits
        /// alone are a FID; write @123 for a username made of digits.
        #[arg(value_name = "ACCOUNT")]
        account: Account,
        /// Count every follower, however many. By default counting stops
        /// at 10000 and the count is a lower bound.
        #[arg(long)]
        all_followers: bool,
    },
    /// Show recent casts from the accounts an account follows, newest first.
    Feed(reads::FeedArgs),
    /// Show recent casts from the accounts named, newest first.
    Casts(reads::CastsArgs),
    /// Show the newest casts posted in a channel, newest first.
    Channel(reads::ChannelArgs),
    /// Show the accounts that follow an account, newest follow first.
    Followers(reads::FollowsArgs),
    /// Show the accounts an account follows, newest follow first.
    Following(reads::FollowsArgs),
    /// Show the replies to a cast, oldest first.
    Replies(reads::RepliesArgs),
    /// Show a cast with the casts it replies to, root first, and its replies.
    Thread(reads::ThreadArgs),
    /// Show who liked and who recast a cast, oldest first.
    Reactions(reads::ReactionsArgs),
    /// List what the node's shards merged, pruned and revoked, block by
    /// block: messages, username proofs and on-chain events.
    Events(events::EventsArgs),
    /// Post a cast, or a reply with --reply-to.
    Cast(write::CastArgs),
    /// Delete one of the account's casts.
    Delete(write::DeleteArgs),
    /// Like or recast a cast.
    React(write::ReactArgs),
    /// Undo a like or a recast.
    Unreact(write::ReactArgs),
    /// Follow an account.
    Follow(write::FollowArgs),
    /// Unfollow an account.
    Unfollow(write::FollowArgs),
    /// Send again any message an earlier write saved but did not see sent.
    Resend,
    /// Serve the viewer page, or drive it: open a thread, read what it
    /// shows.
    #[command(subcommand)]
    Viewer(viewer::ViewerCommand),
    /// Manage the account's app keys.
    #[command(subcommand)]
    Key(KeyCommand),
    /// Set up and manage the Snapchain node itself.
    #[command(subcommand)]
    Node(NodeCommand),
}

#[derive(Subcommand)]
enum KeyCommand {
    /// Register a new app key, authorised by the account's recovery phrase,
    /// which is read once from the terminal and never stored. Mainnet only.
    Add,
}

#[derive(Subcommand)]
enum NodeCommand {
    /// Download the mainnet snapshot so the node can start from it.
    /// Pausable: stop with Ctrl-C and run again to resume.
    FetchSnapshot(snapshot::FetchArgs),
    /// Show which phase the node is in (downloading the snapshot, unpacking,
    /// stopped, syncing, ready) and how far along it is.
    Status(status::StatusArgs),
    /// Pause the snapshot download. Everything downloaded stays on disk.
    Pause(control::PauseArgs),
    /// Resume the snapshot download in the background, logging to
    /// selfcast-fetch.log in the install directory.
    Resume(control::ResumeArgs),
}

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error(transparent)]
    Core(#[from] selfcast_core::Error),
    #[error(transparent)]
    Snapshot(#[from] snapshot::SnapshotError),
    #[error(transparent)]
    KeyAdd(#[from] key_add::KeyAddCliError),
    #[error(transparent)]
    Control(#[from] control::ControlError),
    #[error(transparent)]
    Write(#[from] write::WriteError),
    #[error(transparent)]
    Status(#[from] status::StatusError),
    #[error(transparent)]
    Viewer(#[from] viewer::ViewerError),
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<(), CliError> {
    let named = cli.node.as_deref();
    let node = named.unwrap_or(DEFAULT_NODE);
    match cli.command {
        Command::Info => print(&Node::new(node)?.info().await?, cli.json),
        Command::Profile {
            account,
            all_followers,
        } => reads::profile(&Node::new(node)?, &account, all_followers, cli.json).await?,
        Command::Feed(args) => reads::feed(&Node::new(node)?, &args, cli.json).await?,
        Command::Casts(args) => reads::casts(&Node::new(node)?, &args, cli.json).await?,
        Command::Channel(args) => reads::channel(&Node::new(node)?, &args, cli.json).await?,
        Command::Followers(args) => {
            reads::follows(&Node::new(node)?, &args, true, cli.json).await?
        }
        Command::Following(args) => {
            reads::follows(&Node::new(node)?, &args, false, cli.json).await?
        }
        Command::Replies(args) => reads::replies(&Node::new(node)?, &args, cli.json).await?,
        Command::Thread(args) => reads::thread(&Node::new(node)?, &args, cli.json).await?,
        Command::Reactions(args) => reads::reactions(&Node::new(node)?, &args, cli.json).await?,
        Command::Events(args) => events::run(&Node::new(node)?, &args, cli.json).await?,
        Command::Cast(args) => send(node, args.into_parts(), cli.json).await?,
        Command::Delete(args) => send(node, args.into_parts(), cli.json).await?,
        Command::React(args) => send(node, args.into_parts(false), cli.json).await?,
        Command::Unreact(args) => send(node, args.into_parts(true), cli.json).await?,
        Command::Follow(args) => follow(node, args, false, cli.json).await?,
        Command::Unfollow(args) => follow(node, args, true, cli.json).await?,
        Command::Resend => write::resend(&Node::new(node)?, cli.json).await?,
        Command::Viewer(command) => {
            Node::new(node)?;
            viewer::run(&command, node).await?
        }
        Command::Key(KeyCommand::Add) => key_add::run(&Node::new(node)?, cli.json).await?,
        Command::Node(NodeCommand::FetchSnapshot(args)) => snapshot::run(args).await?,
        Command::Node(NodeCommand::Status(args)) => {
            status::run(status::target(named, DEFAULT_NODE), args, cli.json).await?
        }
        Command::Node(NodeCommand::Pause(args)) => control::pause(args).await?,
        Command::Node(NodeCommand::Resume(args)) => control::resume(args).await?,
    }
    Ok(())
}

async fn send(
    node: &str,
    (action, flags): (selfcast_core::build::Action, write::WriteFlags),
    json: bool,
) -> Result<(), CliError> {
    write::send(&Node::new(node)?, action, flags, json).await
}

async fn follow(
    node: &str,
    args: write::FollowArgs,
    undo: bool,
    json: bool,
) -> Result<(), CliError> {
    let node = Node::new(node)?;
    let (action, flags) = args.into_parts(&node, undo).await?;
    write::send(&node, action, flags, json).await
}

fn print(value: &serde_json::Value, json: bool) {
    let out = if json {
        value.to_string()
    } else {
        serde_json::to_string_pretty(value).expect("a Value always serialises")
    };
    crate::out::line(format_args!("{out}"));
}
