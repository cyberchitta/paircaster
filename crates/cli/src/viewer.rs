//! `selfcast viewer`: the viewer page in Shala, the app that hosts it today.
//!
//! The page reads the node itself, through `selfcast-core` compiled to
//! WebAssembly. These commands only point it at a cast and ask it what it
//! shows, through Shala's own CLI (`shala reveal`, `shala tell`), whose
//! output they pass on as it is.
//!
//! Shala hosts the page in the project whose folder holds the build, so the
//! moniker names the `selfcast` checkout.

use std::process::Command;

use reqwest::Url;
use serde_json::json;

use selfcast_core::cast::CastRef;

const PROJECT: &str = "selfcast";
const SCHEME: &str = "selfcast";
/// The page refuses a body with any other number.
const PROTOCOL: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum ViewerError {
    #[error("could not run shala ({0}); the viewer is hosted by Shala, see the skill")]
    Spawn(std::io::Error),
    #[error("shala could not carry this to the page (its message is above)")]
    Refused,
}

#[derive(Debug, clap::Subcommand)]
pub enum ViewerCommand {
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
pub fn run(command: &ViewerCommand, node: &str) -> Result<(), ViewerError> {
    shala(&match command {
        ViewerCommand::Open { cast } => reveal_args(cast, node),
        ViewerCommand::State => tell_args(),
    })
}

fn shala(args: &[String]) -> Result<(), ViewerError> {
    let status = Command::new("shala")
        .args(args)
        .status()
        .map_err(ViewerError::Spawn)?;
    status.success().then_some(()).ok_or(ViewerError::Refused)
}

fn moniker(cast: &CastRef) -> String {
    format!("{SCHEME}://{PROJECT}/thread/{cast}")
}

fn reveal_args(cast: &CastRef, node: &str) -> Vec<String> {
    let url = Url::parse(node).expect("the node URL was parsed at startup");
    let state = json!({ "node": url.origin().ascii_serialization() });
    vec![
        "reveal".into(),
        "--state".into(),
        state.to_string(),
        moniker(cast),
    ]
}

fn tell_args() -> Vec<String> {
    let body = json!({ "selfcast": PROTOCOL, "get": "state" });
    vec!["tell".into(), PROJECT.into(), body.to_string()]
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
        let args = reveal_args(&cast(), "http://localhost:3381/");
        assert_eq!(args[..2], ["reveal", "--state"]);
        assert_eq!(args[2], r#"{"node":"http://localhost:3381"}"#);
        assert_eq!(
            args[3],
            "selfcast://selfcast/thread/37:0xcd34bb74e0a1f0c4f2d6e3b9a8c7d6e5f4a3b2c1"
        );
    }

    #[test]
    fn state_asks_the_page_in_its_own_protocol() {
        let args = tell_args();
        assert_eq!(args[..2], ["tell", "selfcast"]);
        let body: serde_json::Value = serde_json::from_str(&args[2]).expect("JSON body");
        assert_eq!(body, json!({ "selfcast": 1, "get": "state" }));
    }
}
