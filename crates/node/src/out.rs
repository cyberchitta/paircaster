//! Standard output. A reader that closes the pipe early, as `| head` does,
//! is not a failure: the command has done its work, so it stops quietly
//! with exit 0 rather than panicking as `println!` does. That matters most
//! after a write, which an agent seeing a crash might send again.
//! Restoring SIGPIPE's default instead would need `unsafe`, which the
//! workspace forbids, and would let a closed socket kill the process
//! mid-write.

use std::fmt::Display;
use std::io::{ErrorKind, Write};
use std::process::exit;

/// Prints `text` and a newline to stdout.
pub fn line(text: impl Display) {
    let written = writeln!(std::io::stdout().lock(), "{text}");
    match written {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::BrokenPipe => exit(0),
        Err(e) => {
            eprintln!("error: writing output: {e}");
            exit(1)
        }
    }
}
