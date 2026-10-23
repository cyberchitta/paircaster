//! One download at a time, and a way to find it. `fetch-snapshot` holds an
//! exclusive lock on `<install dir>/.paircaster-fetch.lock` for as long as it
//! runs, with its process ID written inside. The operating system drops the
//! lock when the process exits, however it exits, so a leftover file never
//! reads as a running download.

use std::fs::{File, OpenOptions};
use std::io::{self, ErrorKind, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use rustix::fs::{FlockOperation, flock};

use super::SnapshotError;

const NAME: &str = ".paircaster-fetch.lock";
/// A status check holds a shared lock for an instant; a starting download
/// waits that out instead of reporting itself already running.
const TRIES: u32 = 10;
const EVERY: Duration = Duration::from_millis(100);

/// Held for as long as a download runs.
pub struct FetchLock(#[allow(dead_code, reason = "held for its lock")] File);

fn path(dir: &Path) -> PathBuf {
    dir.join(NAME)
}

/// Takes the lock for this process, or reports the process holding it.
pub fn acquire(dir: &Path) -> Result<FetchLock, SnapshotError> {
    let path = path(dir);
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(SnapshotError::io(&path))?;
    for _ in 0..TRIES {
        match flock(&file, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => {
                write_pid(&mut file).map_err(SnapshotError::io(&path))?;
                return Ok(FetchLock(file));
            }
            Err(e) if io::Error::from(e).kind() == ErrorKind::WouldBlock => thread::sleep(EVERY),
            Err(e) => return Err(SnapshotError::io(&path)(e.into())),
        }
    }
    Err(SnapshotError::AlreadyRunning(holder(dir)?))
}

fn write_pid(file: &mut File) -> io::Result<()> {
    file.set_len(0)?;
    file.rewind()?;
    writeln!(file, "{}", std::process::id())?;
    file.sync_all()
}

/// The process ID of the running download, or `None` if none is running.
pub fn holder(dir: &Path) -> Result<Option<u32>, SnapshotError> {
    let path = path(dir);
    let mut file = match File::open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(SnapshotError::io(&path)(e)),
    };
    match flock(&file, FlockOperation::NonBlockingLockShared) {
        Ok(()) => Ok(None),
        Err(e) if io::Error::from(e).kind() == ErrorKind::WouldBlock => read_pid(&mut file)
            .map(Some)
            .map_err(SnapshotError::io(&path)),
        Err(e) => Err(SnapshotError::io(&path)(e.into())),
    }
}

/// The holder writes its ID just after locking, so an empty file means it
/// is about to; wait for it briefly.
fn read_pid(file: &mut File) -> io::Result<u32> {
    for _ in 0..TRIES {
        let mut text = String::new();
        file.rewind()?;
        file.read_to_string(&mut text)?;
        if let Ok(pid) = text.trim().parse() {
            return Ok(pid);
        }
        thread::sleep(EVERY);
    }
    Err(io::Error::new(
        ErrorKind::InvalidData,
        "locked, but holds no process ID",
    ))
}

#[cfg(test)]
mod tests {
    use super::{SnapshotError, acquire, holder};

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("paircaster-lock-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir is writable");
        dir
    }

    #[test]
    fn the_holder_is_known_until_the_lock_is_dropped() {
        let dir = scratch("holder");
        assert_eq!(holder(&dir).expect("reads"), None);
        let lock = acquire(&dir).expect("locks");
        assert_eq!(holder(&dir).expect("reads"), Some(std::process::id()));
        drop(lock);
        assert_eq!(holder(&dir).expect("reads"), None);
    }

    #[test]
    fn a_second_download_is_refused_while_one_holds_the_lock() {
        let dir = scratch("second");
        let _lock = acquire(&dir).expect("locks");
        assert!(matches!(
            acquire(&dir),
            Err(SnapshotError::AlreadyRunning(Some(pid))) if pid == std::process::id()
        ));
    }
}
