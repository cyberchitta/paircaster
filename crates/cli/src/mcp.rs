//! `selfcast mcp`: the viewer as an MCP Apps page (extension
//! `io.modelcontextprotocol/ui`, spec 2026-01-26), served over stdin and
//! stdout, one JSON-RPC message per line.
//!
//! One tool, `view`, opens the page at a cast. The page then reads the node
//! itself, through `selfcast-core` compiled to WebAssembly, so following a
//! reply involves neither the model nor this server. The agent reads through
//! the CLI as the skill describes; nothing here repeats that.
//!
//! The page's metadata asks the host for two things beyond the spec's
//! default policy: to connect to the node (`csp.connectDomains`, in the
//! spec), and to run WebAssembly (`csp.wasm`, not in the spec; Shala is to
//! grant it). A host that refuses either cannot show the viewer.

use reqwest::Url;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use selfcast_core::cast::CastRef;

const VIEWER_URI: &str = "ui://selfcast/viewer.html";
const VIEWER_MIME: &str = "text/html;profile=mcp-app";
/// Built by `bun build.ts` in `viewer/`.
const VIEWER: &str = include_str!("../../../viewer/dist/viewer.html");

/// MCP base protocol versions this server speaks, newest first.
const PROTOCOL_VERSIONS: [&str; 3] = ["2025-11-25", "2025-06-18", "2025-03-26"];

/// JSON-RPC error codes.
const PARSE_ERROR: i64 = -32700;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// The node the page reads, as the host must allow it.
pub struct Origin(String);

impl Origin {
    /// `node` is a URL already accepted by [`selfcast_core::Node::new`].
    pub fn of(node: &str) -> Self {
        let url = Url::parse(node).expect("the node URL was parsed at startup");
        Origin(url.origin().ascii_serialization())
    }
}

/// One message from the client. Responses to requests we never send, and
/// any field we do not use, are ignored.
#[derive(Deserialize)]
struct Incoming {
    id: Option<Value>,
    method: Option<String>,
    params: Option<Value>,
}

type Reply = Result<Value, (i64, String)>;

pub async fn run(node: Origin) -> std::io::Result<()> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut out = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        if let Some(reply) = handle(&node, &line) {
            out.write_all(format!("{reply}\n").as_bytes()).await?;
            out.flush().await?;
        }
    }
    Ok(())
}

/// The reply to one line, or `None` for a notification.
fn handle(node: &Origin, line: &str) -> Option<Value> {
    let Ok(msg) = serde_json::from_str::<Incoming>(line) else {
        return Some(envelope(
            Value::Null,
            Err((PARSE_ERROR, "not JSON-RPC".into())),
        ));
    };
    let (id, method) = (msg.id?, msg.method?);
    let params = msg.params.unwrap_or(Value::Null);
    Some(envelope(id, respond(node, &method, &params)))
}

fn envelope(id: Value, reply: Reply) -> Value {
    match reply {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => {
            json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
        }
    }
}

fn respond(node: &Origin, method: &str, params: &Value) -> Reply {
    match method {
        "initialize" => Ok(initialize(params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": [view_tool()] })),
        "tools/call" => view(node, params),
        "resources/list" => Ok(json!({ "resources": [viewer_entry()] })),
        "resources/read" => read(node, params),
        _ => Err((METHOD_NOT_FOUND, format!("no method {method}"))),
    }
}

/// Answers with the client's protocol version when this server speaks it,
/// else with the newest one it does.
fn initialize(params: &Value) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str);
    let version = PROTOCOL_VERSIONS
        .into_iter()
        .find(|v| Some(*v) == asked)
        .unwrap_or(PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {}, "resources": {} },
        "serverInfo": { "name": "selfcast", "version": env!("CARGO_PKG_VERSION") },
    })
}

fn view_tool() -> Value {
    json!({
        "name": "view",
        "title": "Selfcast viewer",
        "description": "Show the user a Farcaster cast and its thread in the Selfcast viewer, \
            read from their own node. To read casts yourself, use the selfcast CLI.",
        "inputSchema": {
            "type": "object",
            "properties": { "cast": { "type": "string", "description": "The cast, as FID:HASH." } },
            "required": ["cast"]
        },
        "annotations": { "readOnlyHint": true },
        "_meta": {
            "ui": { "resourceUri": VIEWER_URI, "visibility": ["model"] },
            "ui/resourceUri": VIEWER_URI
        }
    })
}

#[derive(Deserialize)]
struct CallParams {
    name: String,
    arguments: Option<ViewInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewInput {
    cast: String,
}

/// What the page is opened at: the cast, and the node to read it from.
fn view(node: &Origin, params: &Value) -> Reply {
    let invalid = |m: String| (INVALID_PARAMS, m);
    let call = CallParams::deserialize(params).map_err(|e| invalid(e.to_string()))?;
    if call.name != "view" {
        return Err(invalid(format!("no tool {}", call.name)));
    }
    let input = call
        .arguments
        .ok_or_else(|| invalid("view needs a cast".into()))?;
    let cast: CastRef = input.cast.parse().map_err(|e| invalid(format!("{e}")))?;
    Ok(json!({
        "content": [{ "type": "text", "text": format!("Showing cast {cast} in the Selfcast viewer.") }],
        "structuredContent": { "cast": cast.to_string(), "node": node.0 },
    }))
}

fn viewer_entry() -> Value {
    json!({ "uri": VIEWER_URI, "name": "viewer", "title": "Selfcast viewer", "mimeType": VIEWER_MIME })
}

fn read(node: &Origin, params: &Value) -> Reply {
    match params.get("uri").and_then(Value::as_str) {
        Some(VIEWER_URI) => Ok(json!({
            "contents": [{
                "uri": VIEWER_URI,
                "mimeType": VIEWER_MIME,
                "text": VIEWER,
                "_meta": { "ui": {
                    "csp": { "connectDomains": [node.0], "wasm": true },
                    "prefersBorder": false
                } }
            }]
        })),
        _ => Err((INVALID_PARAMS, "no such resource".into())),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{Origin, VIEWER_URI, handle};

    fn origin() -> Origin {
        Origin::of("http://localhost:3381/")
    }

    fn ask(method: &str, params: Value) -> Value {
        let line = json!({ "jsonrpc": "2.0", "id": 7, "method": method, "params": params });
        handle(&origin(), &line.to_string()).expect("a request gets a reply")
    }

    #[test]
    fn initialize_echoes_a_known_version_and_names_the_server() {
        let r = ask("initialize", json!({ "protocolVersion": "2025-06-18" }));
        assert_eq!(r["id"], 7);
        assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(r["result"]["serverInfo"]["name"], "selfcast");
        let r = ask("initialize", json!({ "protocolVersion": "1999-01-01" }));
        assert_eq!(r["result"]["protocolVersion"], "2025-11-25");
    }

    #[test]
    fn view_is_the_only_tool_and_only_the_model_calls_it() {
        let r = ask("tools/list", json!({}));
        let tools = r["result"]["tools"].as_array().expect("a list");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "view");
        assert_eq!(tools[0]["_meta"]["ui"]["resourceUri"], VIEWER_URI);
        assert_eq!(tools[0]["_meta"]["ui"]["visibility"], json!(["model"]));
    }

    #[test]
    fn view_hands_the_page_the_cast_and_the_node() {
        let cast = format!("37:0x{}", "cd".repeat(20));
        let r = ask(
            "tools/call",
            json!({ "name": "view", "arguments": { "cast": cast } }),
        );
        let shown = &r["result"]["structuredContent"];
        assert_eq!(shown["cast"], cast);
        assert_eq!(shown["node"], "http://localhost:3381");
    }

    #[test]
    fn the_page_declares_the_node_and_webassembly() {
        let r = ask("resources/read", json!({ "uri": VIEWER_URI }));
        let content = &r["result"]["contents"][0];
        assert_eq!(content["mimeType"], "text/html;profile=mcp-app");
        assert!(
            content["text"]
                .as_str()
                .expect("text")
                .starts_with("<!doctype html>")
        );
        let csp = &content["_meta"]["ui"]["csp"];
        assert_eq!(csp["connectDomains"], json!(["http://localhost:3381"]));
        assert_eq!(csp["wasm"], true);
        let r = ask("resources/read", json!({ "uri": "ui://other" }));
        assert_eq!(r["error"]["code"], -32602);
    }

    #[test]
    fn malformed_calls_are_refused() {
        let bad = [
            json!({ "name": "thread", "arguments": { "cast": "1:0x00" } }),
            json!({ "name": "view" }),
            json!({ "name": "view", "arguments": { "cast": "nope" } }),
            json!({ "name": "view", "arguments": { "cast": format!("1:0x{}", "ab".repeat(20)), "limit": 5 } }),
        ];
        for params in bad {
            let r = ask("tools/call", params.clone());
            assert_eq!(r["error"]["code"], -32602, "{params}");
        }
    }

    #[test]
    fn notifications_get_no_reply_and_unknown_methods_an_error() {
        let note = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        assert_eq!(handle(&origin(), &note.to_string()), None);
        assert_eq!(
            ask("sampling/createMessage", json!({}))["error"]["code"],
            -32601
        );
        assert_eq!(
            handle(&origin(), "{").expect("a reply")["error"]["code"],
            -32700
        );
    }
}
