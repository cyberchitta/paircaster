//! `selfcast-core`'s reads for the viewer page and pages built on it. Each
//! function is named after a CLI read and returns the JSON that
//! `selfcast <read> --json` prints (`selfcast_core::output`), so the skill
//! that documents the CLI documents these too.

use wasm_bindgen::prelude::*;

use selfcast_core::Node;
use selfcast_core::cast::CastRef;
use selfcast_core::output;
use selfcast_core::time::Timestamp;

/// `selfcast thread <cast> --limit <limit> --json`, read from the node at
/// `node`. `now` is the page's clock in Unix seconds; replies dated more than
/// ten minutes past it are left out, as in the CLI.
#[wasm_bindgen]
pub async fn thread(node: String, cast: String, limit: u32, now: f64) -> Result<String, JsError> {
    let node = Node::new(&node)?;
    let id: CastRef = cast.parse()?;
    let (thread, users) = node
        .thread(&id, limit as usize, Timestamp::from_unix(now as u64))
        .await?;
    Ok(output::thread_json(&thread, &users))
}
