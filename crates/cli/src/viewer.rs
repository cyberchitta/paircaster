//! `selfcast viewer`: the viewer page, hosted by pair-ui in a browser or by
//! Shala.
//!
//! The page reads the node itself, through `selfcast-core` compiled to
//! WebAssembly. `open` and `state` only point it at a cast and ask it what it
//! shows. They go to a pair-ui host when one is running for this project
//! (`selfcast viewer serve`), else through Shala's own CLI (`shala reveal`,
//! `shala tell`), whose output they pass on as it is. Both hosts take the
//! same requests.
//!
//! `serve` hosts the build compiled in with the `viewer` feature.

use std::process::Command;

use pair_ui::client::{self, ClientError};
use pair_ui::{
    Answer, BadOrigin, Build, Config, Host, HostError, Moniker, Origin, Policy, socket_path,
};
use reqwest::Url;
use serde_json::{Value, json};

use selfcast_core::cast::CastRef;

use crate::out;

const PROJECT: &str = "selfcast";
const SCHEME: &str = "selfcast";
/// The page refuses a body with any other number.
const PROTOCOL: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum ViewerError {
    #[error(
        "no viewer is open: run `selfcast viewer serve` and open its address, or open selfcast in Shala ({0})"
    )]
    NoHost(std::io::Error),
    #[error("shala could not carry this to the page (its message is above)")]
    ShalaRefused,
    #[error("the viewer could not do it: {0}")]
    PageRefused(String),
    #[error("the viewer's host: {0}")]
    Client(#[from] ClientError),
    #[error("this selfcast was built without the viewer (the `viewer` feature)")]
    NotBuiltIn,
    #[error("the viewer cannot reach this node: {0}")]
    BadNode(#[from] BadOrigin),
    #[error("could not start the viewer: {0}")]
    Host(#[from] HostError),
    #[error("the viewer stopped: {0}")]
    Serve(std::io::Error),
}

#[derive(Debug, clap::Subcommand)]
pub enum ViewerCommand {
    /// Serve the viewer on 127.0.0.1, reading from --node, until stopped.
    /// Prints the address to open it at.
    Serve,
    /// Open a cast's thread in the viewer, reading from --node.
    Open {
        /// The cast to show, as FID:HASH.
        #[arg(value_name = "FID:HASH")]
        cast: CastRef,
    },
    /// Print what the viewer shows: the thread, the cast selected in it, and
    /// the trail Back returns along.
    State,
}

/// `node` is a URL already accepted by [`selfcast_core::Node::new`].
pub async fn run(command: &ViewerCommand, node: &str) -> Result<(), ViewerError> {
    match command {
        ViewerCommand::Serve => serve(node).await,
        ViewerCommand::Open { cast } => open(cast, node).await,
        ViewerCommand::State => state().await,
    }
}

async fn open(cast: &CastRef, node: &str) -> Result<(), ViewerError> {
    let (moniker, view) = (moniker(cast), view_state(node));
    match client::reveal(&socket_path(PROJECT), &moniker, Some(&view)).await {
        Err(ClientError::NoHost(_)) => shala(&reveal_args(&moniker, &view)),
        answer => print(answer?, &format!("revealed {moniker}")),
    }
}

async fn state() -> Result<(), ViewerError> {
    let body = state_body();
    match client::tell(&socket_path(PROJECT), PROJECT, &body).await {
        Err(ClientError::NoHost(_)) => shala(&["tell".into(), PROJECT.into(), body.to_string()]),
        answer => print(answer?, &format!("told {PROJECT}")),
    }
}

/// Prints the page's answer as `shala` prints one, so either host reads the
/// same to an agent.
fn print(answer: Answer, done: &str) -> Result<(), ViewerError> {
    if !answer.ok {
        return Err(ViewerError::PageRefused(answer.message));
    }
    out::line(done);
    out::line(format!("message: {}", answer.message));
    if let Some(result) = answer.result {
        out::line(format!("result: {result}"));
    }
    Ok(())
}

fn shala(args: &[String]) -> Result<(), ViewerError> {
    let status = Command::new("shala")
        .args(args)
        .status()
        .map_err(ViewerError::NoHost)?;
    status
        .success()
        .then_some(())
        .ok_or(ViewerError::ShalaRefused)
}

fn moniker(cast: &CastRef) -> Moniker {
    format!("{SCHEME}://{PROJECT}/thread/{cast}")
        .parse()
        .expect("a scheme, a project and a cast ref always make a moniker")
}

fn node_origin(node: &str) -> String {
    let url = Url::parse(node).expect("the node URL was parsed at startup");
    url.origin().ascii_serialization()
}

fn view_state(node: &str) -> Value {
    json!({ "node": node_origin(node) })
}

fn reveal_args(moniker: &Moniker, view: &Value) -> Vec<String> {
    vec![
        "reveal".into(),
        "--state".into(),
        view.to_string(),
        moniker.to_string(),
    ]
}

fn state_body() -> Value {
    json!({ "selfcast": PROTOCOL, "get": "state" })
}

async fn serve(node: &str) -> Result<(), ViewerError> {
    let build = embedded().ok_or(ViewerError::NotBuiltIn)?;
    let node: Origin = node_origin(node).parse()?;
    let host = Host::bind(Config {
        project: PROJECT.into(),
        build,
        policy: Policy {
            connect: vec![node],
            wasm: true,
        },
        socket: socket_path(PROJECT),
    })
    .await?;
    out::line(format!("open the viewer at {}", host.page_url()));
    out::line("stop it with Ctrl-C");
    host.serve().await.map_err(ViewerError::Serve)
}

#[cfg(feature = "viewer")]
fn embedded() -> Option<Build> {
    // Written by `bun build.ts` in viewer/; build.rs refuses to build without them.
    static FILES: &[(&str, &[u8])] = &[
        (
            "index.html",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../viewer/dist/index.html"
            )),
        ),
        (
            "viewer.js",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../viewer/dist/viewer.js"
            )),
        ),
    ];
    Some(Build::Embedded(FILES))
}

#[cfg(not(feature = "viewer"))]
fn embedded() -> Option<Build> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cast() -> CastRef {
        "37:0xcd34bb74e0a1f0c4f2d6e3b9a8c7d6e5f4a3b2c1"
            .parse()
            .expect("valid cast ref")
    }

    #[test]
    fn open_reveals_the_thread_moniker_with_the_node_origin() {
        let view = view_state("http://localhost:3381/");
        let args = reveal_args(&moniker(&cast()), &view);
        assert_eq!(args[..2], ["reveal", "--state"]);
        assert_eq!(args[2], r#"{"node":"http://localhost:3381"}"#);
        assert_eq!(
            args[3],
            "selfcast://selfcast/thread/37:0xcd34bb74e0a1f0c4f2d6e3b9a8c7d6e5f4a3b2c1"
        );
    }

    #[test]
    fn state_asks_the_page_in_its_own_protocol() {
        assert_eq!(state_body(), json!({ "selfcast": 1, "get": "state" }));
    }
}
