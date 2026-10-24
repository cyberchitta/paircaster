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
use paircaster_core::Node;
use paircaster_core::account::Account;

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
    /// Show the addresses an account has verified, oldest first.
    Addresses(reads::AddressesArgs),
    /// Show the accounts that follow an account, newest follow first.
    Followers(reads::FollowsArgs),
    /// Show the accounts an account follows, newest follow first.
    Following(reads::FollowsArgs),
    /// Show the replies to a cast, oldest first.
    Replies(reads::RepliesArgs),
    /// Show a cast with the casts it replies to, root first, and its replies.
    Thread(reads::ThreadArgs),
    /// Show one cast, without the casts around it.
    Show(reads::ShowArgs),
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
    /// paircaster-fetch.log in the install directory.
    Resume(control::ResumeArgs),
}

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error(transparent)]
    Core(#[from] paircaster_core::Error),
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
    let conn = Node::new(node)?;
    let result = dispatch(&conn, named, cli.command, cli.json).await;
    note_skipped(&conn);
    result
}

async fn dispatch(
    conn: &Node,
    named: Option<&str>,
    command: Command,
    json: bool,
) -> Result<(), CliError> {
    let node = named.unwrap_or(DEFAULT_NODE);
    match command {
        Command::Info => print(&conn.info().await?, json),
        Command::Profile {
            account,
            all_followers,
        } => reads::profile(conn, &account, all_followers, json).await?,
        Command::Feed(args) => reads::feed(conn, &args, json).await?,
        Command::Casts(args) => reads::casts(conn, &args, json).await?,
        Command::Channel(args) => reads::channel(conn, &args, json).await?,
        Command::Addresses(args) => reads::addresses(conn, &args, json).await?,
        Command::Followers(args) => reads::follows(conn, &args, true, json).await?,
        Command::Following(args) => reads::follows(conn, &args, false, json).await?,
        Command::Replies(args) => reads::replies(conn, &args, json).await?,
        Command::Thread(args) => reads::thread(conn, &args, json).await?,
        Command::Show(args) => reads::show(conn, &args, json).await?,
        Command::Reactions(args) => reads::reactions(conn, &args, json).await?,
        Command::Events(args) => events::run(conn, &args, json).await?,
        Command::Cast(args) => send(conn, args.into_parts(), json).await?,
        Command::Delete(args) => send(conn, args.into_parts(), json).await?,
        Command::React(args) => send(conn, args.into_parts(false), json).await?,
        Command::Unreact(args) => send(conn, args.into_parts(true), json).await?,
        Command::Follow(args) => follow(conn, args, false, json).await?,
        Command::Unfollow(args) => follow(conn, args, true, json).await?,
        Command::Resend => write::resend(conn, json).await?,
        Command::Viewer(command) => viewer::run(&command, node).await?,
        Command::Key(KeyCommand::Add) => key_add::run(conn, json).await?,
        Command::Node(NodeCommand::FetchSnapshot(args)) => snapshot::run(args).await?,
        Command::Node(NodeCommand::Status(args)) => {
            status::run(status::target(named, DEFAULT_NODE), args, json).await?
        }
        Command::Node(NodeCommand::Pause(args)) => control::pause(args).await?,
        Command::Node(NodeCommand::Resume(args)) => control::resume(args).await?,
    }
    Ok(())
}

/// One line on stderr when a read left messages out, so the answer is
/// not taken as complete.
fn note_skipped(node: &Node) {
    match node.skipped() {
        0 => {}
        n => eprintln!(
            "note: left out {n} message(s) holding a value this version of paircaster does not read; a newer paircaster may read them"
        ),
    }
}

async fn send(
    node: &Node,
    (action, flags): (paircaster_core::build::Action, write::WriteFlags),
    json: bool,
) -> Result<(), CliError> {
    write::send(node, action, flags, json).await
}

async fn follow(
    node: &Node,
    args: write::FollowArgs,
    undo: bool,
    json: bool,
) -> Result<(), CliError> {
    let (action, flags) = args.into_parts(node, undo).await?;
    write::send(node, action, flags, json).await
}

fn print(value: &serde_json::Value, json: bool) {
    let out = if json {
        value.to_string()
    } else {
        serde_json::to_string_pretty(value).expect("a Value always serialises")
    };
    crate::out::line(format_args!("{out}"));
}
