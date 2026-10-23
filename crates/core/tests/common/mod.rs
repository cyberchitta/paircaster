//! Saved node responses, found through `PAIRCASTER_FIXTURES`. They are never
//! committed, so tests that read them are `#[ignore]`d and run with
//! `--include-ignored` where the directory exists.

use std::path::PathBuf;

pub fn fixture(name: &str) -> serde_json::Value {
    let dir = std::env::var_os("PAIRCASTER_FIXTURES")
        .expect("PAIRCASTER_FIXTURES must name the fixtures directory");
    let path = PathBuf::from(dir).join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture {} unreadable: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("fixture {name} is not JSON: {e}"))
}

/// The messages in a fixture: a paged response's `messages`, or the one
/// message a lookup by id returns.
pub fn fixture_messages(name: &str) -> Vec<serde_json::Value> {
    match fixture(name) {
        serde_json::Value::Object(mut page) if page.contains_key("messages") => {
            match page.remove("messages") {
                Some(serde_json::Value::Array(messages)) => messages,
                _ => panic!("fixture {name}: messages is not an array"),
            }
        }
        single => vec![single],
    }
}
