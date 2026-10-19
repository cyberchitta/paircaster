mod reads;
mod snapshot;
mod status;

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use selfcast_core::Node;

/// Farcaster from your own Snapchain node.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Node HTTP API base URL.
    #[arg(
        long,
        env = "SELFCAST_NODE",
        default_value = "http://localhost:3381",
        global = true
    )]
    node: String,
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
        /// The account's FID.
        fid: u64,
    },
    /// Show recent casts from the accounts an account follows, newest first.
    Feed(reads::FeedArgs),
    /// Set up and manage the Snapchain node itself.
    #[command(subcommand)]
    Node(NodeCommand),
}

#[derive(Subcommand)]
enum NodeCommand {
    /// Download the mainnet snapshot so the node can start from it.
    /// Pausable: stop with Ctrl-C and run again to resume.
    FetchSnapshot(snapshot::FetchArgs),
    /// Show which phase the node is in (downloading the snapshot, unpacking,
    /// stopped, syncing, ready) and how far along it is.
    Status(status::StatusArgs),
}

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error(transparent)]
    Core(#[from] selfcast_core::Error),
    #[error(transparent)]
    Snapshot(#[from] snapshot::SnapshotError),
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
    match cli.command {
        Command::Info => print(&Node::new(&cli.node)?.info().await?, cli.json),
        Command::Profile { fid } => reads::profile(&Node::new(&cli.node)?, fid, cli.json).await?,
        Command::Feed(args) => reads::feed(&Node::new(&cli.node)?, &args, cli.json).await?,
        Command::Node(NodeCommand::FetchSnapshot(args)) => snapshot::run(args).await?,
        Command::Node(NodeCommand::Status(args)) => status::run(&cli.node, args, cli.json).await?,
    }
    Ok(())
}

fn print(value: &serde_json::Value, json: bool) {
    let out = if json {
        value.to_string()
    } else {
        serde_json::to_string_pretty(value).expect("a Value always serialises")
    };
    println!("{out}");
}
