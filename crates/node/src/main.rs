//! `paircaster-node`: sets up and watches the Snapchain node on this
//! machine. It holds no keys and sends no messages; that is `paircaster`'s,
//! beside the user. So a server running the node needs only this.

mod clock;
mod control;
mod invocation;
mod out;
mod snapshot;
mod status;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Where the node's HTTP API answers when `--node` is not given: Snapchain's
/// own compose file publishes it on this port.
const DEFAULT_NODE: &str = "http://localhost:3381";

/// Set up and watch the Snapchain node on this machine.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// The node's HTTP API on this machine [default: http://localhost:3381].
    #[arg(long, global = true)]
    node: Option<String>,
    /// Print machine-readable JSON instead of human output (`status`).
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show which phase the node is in (downloading the snapshot, unpacking,
    /// stopped, syncing, ready) and how far along it is.
    Status(status::StatusArgs),
    /// Download the mainnet snapshot so the node can start from it.
    /// Pausable: stop with Ctrl-C and run again to resume.
    FetchSnapshot(snapshot::FetchArgs),
    /// Pause the snapshot download. Everything downloaded stays on disk.
    Pause(control::PauseArgs),
    /// Resume the snapshot download in the background, logging to
    /// paircaster-fetch.log in the install directory.
    Resume(control::ResumeArgs),
}

#[derive(Debug, thiserror::Error)]
enum NodeError {
    #[error(transparent)]
    Core(#[from] paircaster_core::Error),
    #[error(transparent)]
    Snapshot(#[from] snapshot::SnapshotError),
    #[error(transparent)]
    Control(#[from] control::ControlError),
    #[error(transparent)]
    Status(#[from] status::StatusError),
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<(), NodeError> {
    let url = cli.node.as_deref().unwrap_or(DEFAULT_NODE);
    match cli.command {
        Command::Status(args) => status::run(url, args, cli.json).await?,
        Command::FetchSnapshot(args) => snapshot::run(args).await?,
        Command::Pause(args) => control::pause(args).await?,
        Command::Resume(args) => control::resume(args).await?,
    }
    Ok(())
}
