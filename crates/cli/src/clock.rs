//! The one place the CLI reads the clock; the core takes time as an argument.

use std::time::{SystemTime, UNIX_EPOCH};

use paircaster_core::time::Timestamp;

pub fn now() -> Timestamp {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_secs();
    Timestamp::from_unix(secs)
}
