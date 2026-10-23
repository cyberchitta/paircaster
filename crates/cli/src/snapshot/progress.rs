//! What a snapshot download has left on disk, read for `paircaster node status`
//! without touching anything `fetch-snapshot` writes. A chunk is renamed from
//! `.part` only once complete, so its modification time is when it finished.

use std::fs::Metadata;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::index::{self, Shard};
use super::{Paths, SnapshotError, lock};

/// One downloaded chunk: when it finished and how big it is.
#[derive(Debug, Clone, Copy)]
pub struct Chunk {
    pub finished: SystemTime,
    pub bytes: u64,
}

/// Where a snapshot download stands: from its files, and from its lock
/// whether a `fetch-snapshot` process is working on it.
#[derive(Debug)]
pub enum Observed {
    /// No download started and no database.
    Nothing,
    Downloading(Download),
    Unpacking(Unpack),
    /// `.rocks` holds a database.
    Database,
}

#[derive(Debug)]
pub struct Download {
    pub running: bool,
    pub chunks_total: usize,
    pub chunks_done: usize,
    /// The downloaded chunks still on disk.
    pub chunks: Vec<Chunk>,
    pub last_write: Option<SystemTime>,
}

#[derive(Debug)]
pub struct Unpack {
    pub running: bool,
    pub shards_total: usize,
    pub shards_done: usize,
    pub last_write: Option<SystemTime>,
}

struct ShardState {
    unpacked: bool,
    done: usize,
    chunks: Vec<Chunk>,
    last_write: Option<SystemTime>,
}

pub(super) fn observe(paths: &Paths) -> Result<Observed, SnapshotError> {
    if super::db_filled(&paths.db)? {
        return Ok(Observed::Database);
    }
    let Some(index) = index::load_pinned(&paths.work.join("index.json"))? else {
        return Ok(Observed::Nothing);
    };
    let shards = index
        .shards
        .iter()
        .map(|s| shard_state(s, paths))
        .collect::<Result<Vec<_>, _>>()?;
    let running = lock::holder(&paths.dir)?.is_some();
    let chunks_done = shards.iter().map(|s| s.done).sum();
    if chunks_done < index.total_chunks() {
        return Ok(Observed::Downloading(Download {
            running,
            chunks_total: index.total_chunks(),
            chunks_done,
            last_write: shards.iter().filter_map(|s| s.last_write).max(),
            chunks: shards.into_iter().flat_map(|s| s.chunks).collect(),
        }));
    }
    Ok(Observed::Unpacking(Unpack {
        running,
        shards_total: shards.len(),
        shards_done: shards.iter().filter(|s| s.unpacked).count(),
        last_write: newest_under(&paths.partial, 2)?,
    }))
}

fn shard_state(shard: &Shard, paths: &Paths) -> Result<ShardState, SnapshotError> {
    if paths.marker(shard).exists() {
        return Ok(ShardState {
            unpacked: true,
            done: shard.chunks.len(),
            chunks: Vec::new(),
            last_write: None,
        });
    }
    let files = list(&shard.chunk_dir(&paths.work))?;
    let last_write = files.iter().filter_map(|(_, m)| m.modified().ok()).max();
    let chunks: Vec<Chunk> = files
        .iter()
        .filter(|(path, _)| path.extension().is_some_and(|e| e == "bin"))
        .filter_map(|(_, m)| chunk(m))
        .collect();
    Ok(ShardState {
        unpacked: false,
        done: chunks.len(),
        chunks,
        last_write,
    })
}

fn chunk(meta: &Metadata) -> Option<Chunk> {
    meta.modified().ok().map(|finished| Chunk {
        finished,
        bytes: meta.len(),
    })
}

/// The files in `dir` with their metadata; empty if `dir` does not exist. A
/// file renamed between listing and reading (a `.part` completing) is skipped.
fn list(dir: &Path) -> Result<Vec<(PathBuf, Metadata)>, SnapshotError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(SnapshotError::io(dir)(e)),
    };
    entries
        .map(|entry| entry.map_err(SnapshotError::io(dir)))
        .filter_map(|entry| match entry {
            Ok(e) => match e.metadata() {
                Ok(m) => Some(Ok((e.path(), m))),
                Err(err) if err.kind() == io::ErrorKind::NotFound => None,
                Err(err) => Some(Err(SnapshotError::io(&e.path())(err))),
            },
            Err(err) => Some(Err(err)),
        })
        .collect()
}

/// The newest modification time of `dir` and what lies up to `depth`
/// levels below it; `None` if `dir` does not exist.
fn newest_under(dir: &Path, depth: u32) -> Result<Option<SystemTime>, SnapshotError> {
    let own = match std::fs::metadata(dir) {
        Ok(m) => m.modified().ok(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(SnapshotError::io(dir)(e)),
    };
    if depth == 0 || !dir.is_dir() {
        return Ok(own);
    }
    let below = list(dir)?
        .iter()
        .map(|(path, _)| newest_under(path, depth - 1))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(below.into_iter().flatten().chain(own).max())
}
