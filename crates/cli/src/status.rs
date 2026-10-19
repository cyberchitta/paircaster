//! `selfcast node status`: which phase the node is in (downloading the
//! snapshot, unpacking it, stopped, syncing, ready) and how far along.
//!
//! The node is asked first; only when nothing answers is the snapshot
//! download read from disk.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use selfcast_core::Node;
use selfcast_core::node::SyncState;
use serde::Serialize;

use crate::CliError;
use crate::invocation;
use crate::snapshot::{self, Chunk, Download, Observed, Unpack};

/// Download speed is measured over the chunks finished this recently.
const RATE_WINDOW: Duration = Duration::from_secs(300);
/// A running download that has written nothing for longer than this is
/// waiting out a network outage.
const IDLE_AFTER: Duration = Duration::from_secs(90);
/// A node is ready when every shard's newest block is at most this old.
const READY_WITHIN_SECS: u64 = 60;

#[derive(Debug, clap::Args)]
pub struct StatusArgs {
    /// Snapchain install directory (holds snapchain.sh and .rocks) [default: ~/snapchain]
    #[arg(long, env = "SNAPCHAIN_DIR")]
    dir: Option<PathBuf>,
}

/// `selfcast node status --json`, version 1. `phase` is one of
/// `not_started`, `downloading`, `unpacking`, `node_stopped`, `syncing`,
/// `ready`, and decides which other fields are present:
///
/// ```json
/// { "version": 1, "phase": "downloading", "running": true,
///   "chunks_done": 1289, "chunks_total": 5691, "bytes_per_second": 41800000,
///   "eta_seconds": 10260, "seconds_since_write": 2 }
/// { "version": 1, "phase": "unpacking", "running": true, "shards_done": 1,
///   "shards_total": 3, "seconds_since_write": 0 }
/// { "version": 1, "phase": "syncing", "node_version": "0.14.2",
///   "shards": [{ "id": 0, "height": 48340352, "seconds_behind": 4200 }] }
/// ```
///
/// `running` is whether a `fetch-snapshot` process holds the download's
/// lock; `false` means paused or interrupted. `bytes_per_second` and
/// `eta_seconds` are `null` while not running, and until a few chunks have
/// finished in the last five minutes; `seconds_since_write` is `null`
/// before anything is written. `ready` has the same fields as `syncing`.
///
/// Every phase also carries `disk`, the filesystem holding the Snapchain
/// install directory, or `null` when that directory does not exist (as for
/// a remote node): `"disk": { "free_bytes": 3400000000000,
/// "total_bytes": 3600000000000, "low": false }`. `low` is under 15% free.
#[derive(Debug, Serialize)]
struct StatusV1 {
    version: u32,
    #[serde(flatten)]
    phase: Phase,
    disk: Option<DiskV1>,
}

#[derive(Debug, Serialize)]
struct DiskV1 {
    free_bytes: u64,
    total_bytes: u64,
    low: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
enum Phase {
    NotStarted,
    Downloading {
        running: bool,
        chunks_done: usize,
        chunks_total: usize,
        bytes_per_second: Option<u64>,
        eta_seconds: Option<u64>,
        seconds_since_write: Option<u64>,
    },
    Unpacking {
        running: bool,
        shards_done: usize,
        shards_total: usize,
        seconds_since_write: Option<u64>,
    },
    NodeStopped,
    Syncing {
        node_version: String,
        shards: Vec<ShardV1>,
    },
    Ready {
        node_version: String,
        shards: Vec<ShardV1>,
    },
}

#[derive(Debug, Serialize)]
struct ShardV1 {
    id: u32,
    height: u64,
    seconds_behind: u64,
}

pub async fn run(node_url: &str, args: StatusArgs, json: bool) -> Result<(), CliError> {
    let dir = snapshot::install_dir(args.dir)?;
    let phase = match Node::new(node_url)?.sync_state().await {
        Ok(state) => synced(state),
        Err(selfcast_core::Error::Transport(_)) => {
            on_disk(snapshot::observe(&dir)?, SystemTime::now())
        }
        Err(e) => return Err(e.into()),
    };
    let disk = snapshot::disk(&dir)?.map(|d| DiskV1 {
        free_bytes: d.free,
        total_bytes: d.total,
        low: is_low(d.free, d.total),
    });
    let out = match json {
        true => serde_json::to_string(&StatusV1 {
            version: 1,
            phase,
            disk,
        })
        .expect("a status always serialises"),
        false => [
            describe(&phase, node_url, &dir),
            disk_line(disk.as_ref(), &dir),
        ]
        .join("\n"),
    };
    println!("{out}");
    Ok(())
}

/// Under this share of the disk free, the node's growing database is at risk.
const LOW_DISK_PERCENT: u64 = 15;

fn is_low(free: u64, total: u64) -> bool {
    u128::from(free) * 100 < u128::from(total) * u128::from(LOW_DISK_PERCENT)
}

fn disk_line(disk: Option<&DiskV1>, dir: &Path) -> String {
    match disk {
        None => format!("Disk: {} does not exist here.", dir.display()),
        Some(d) => {
            let line = format!(
                "Disk: {:.0} GB free of {:.0} GB.",
                d.free_bytes as f64 / 1e9,
                d.total_bytes as f64 / 1e9
            );
            match d.low {
                true => format!(
                    "{line}\nWarning: under {LOW_DISK_PERCENT}% free. The node's database keeps growing; free space before it fills."
                ),
                false => line,
            }
        }
    }
}

fn synced(state: SyncState) -> Phase {
    let ready = state
        .seconds_behind()
        .is_some_and(|s| s <= READY_WITHIN_SECS);
    let shards = state
        .shards
        .iter()
        .map(|s| ShardV1 {
            id: s.id,
            height: s.height,
            seconds_behind: s.seconds_behind,
        })
        .collect();
    let node_version = state.version;
    match ready {
        true => Phase::Ready {
            node_version,
            shards,
        },
        false => Phase::Syncing {
            node_version,
            shards,
        },
    }
}

fn on_disk(observed: Observed, now: SystemTime) -> Phase {
    match observed {
        Observed::Nothing => Phase::NotStarted,
        Observed::Database => Phase::NodeStopped,
        Observed::Downloading(d) => downloading(&d, now),
        Observed::Unpacking(u) => unpacking(&u, now),
    }
}

fn downloading(d: &Download, now: SystemTime) -> Phase {
    let rate = bytes_per_second(&d.chunks, now).filter(|_| d.running);
    let eta = rate.and_then(|r| eta_seconds(&d.chunks, d.chunks_total - d.chunks_done, r));
    Phase::Downloading {
        running: d.running,
        chunks_done: d.chunks_done,
        chunks_total: d.chunks_total,
        bytes_per_second: rate.map(|r| r as u64),
        eta_seconds: eta,
        seconds_since_write: d.last_write.map(|t| age(now, t).as_secs()),
    }
}

fn unpacking(u: &Unpack, now: SystemTime) -> Phase {
    Phase::Unpacking {
        running: u.running,
        shards_done: u.shards_done,
        shards_total: u.shards_total,
        seconds_since_write: u.last_write.map(|t| age(now, t).as_secs()),
    }
}

/// Bytes per second over the chunks finished within `RATE_WINDOW` of `now`:
/// the bytes after the first such chunk, over the time from it to the last.
/// `None` with fewer than three, too few to measure.
fn bytes_per_second(chunks: &[Chunk], now: SystemTime) -> Option<f64> {
    let mut recent: Vec<&Chunk> = chunks
        .iter()
        .filter(|c| age(now, c.finished) <= RATE_WINDOW)
        .collect();
    recent.sort_by_key(|c| c.finished);
    let (first, rest) = recent.split_first()?;
    let span = rest.last()?.finished.duration_since(first.finished).ok()?;
    let bytes: u64 = rest.iter().map(|c| c.bytes).sum();
    (rest.len() >= 2 && !span.is_zero()).then(|| bytes as f64 / span.as_secs_f64())
}

/// Seconds to fetch `remaining` chunks of the mean size of `done` at `rate`.
fn eta_seconds(done: &[Chunk], remaining: usize, rate: f64) -> Option<u64> {
    (!done.is_empty()).then(|| {
        let mean = done.iter().map(|c| c.bytes).sum::<u64>() as f64 / done.len() as f64;
        (remaining as f64 * mean / rate) as u64
    })
}

/// How long ago `then` was; zero for a time in the future (clock skew).
fn age(now: SystemTime, then: SystemTime) -> Duration {
    now.duration_since(then).unwrap_or(Duration::ZERO)
}

fn describe(phase: &Phase, node_url: &str, dir: &Path) -> String {
    match phase {
        Phase::NotStarted => format!(
            "No snapshot download started, and no node answers at {node_url}.\n\
             Start with: {}",
            invocation::node("fetch-snapshot", dir)
        ),
        Phase::Downloading {
            running,
            chunks_done,
            chunks_total,
            bytes_per_second,
            eta_seconds,
            seconds_since_write,
        } => {
            let pct = chunks_done * 100 / chunks_total;
            let head = format!(
                "Downloading the snapshot: {chunks_done} of {chunks_total} chunks ({pct}%)."
            );
            let tail = match running {
                true => download_tail(*bytes_per_second, *eta_seconds, *seconds_since_write),
                false => format!("Paused. To resume: {}", invocation::node("resume", dir)),
            };
            format!("{head}\n{tail}")
        }
        Phase::Unpacking {
            running,
            shards_done,
            shards_total,
            ..
        } => {
            let head =
                format!("Unpacking the snapshot: {shards_done} of {shards_total} shards done.");
            match running {
                true => head,
                false => format!(
                    "{head}\nInterrupted. To finish (the current shard starts over): {}",
                    invocation::node("resume", dir)
                ),
            }
        }
        Phase::NodeStopped => format!(
            "The snapshot is in place, but no node answers at {node_url}.\n\
             Start it: cd {} && ./snapchain.sh up (it can take a minute to answer)",
            invocation::path(dir)
        ),
        Phase::Syncing {
            node_version,
            shards,
        } => {
            let furthest = shards.iter().map(|s| s.seconds_behind).max().unwrap_or(0);
            let head = format!(
                "Syncing (Snapchain {node_version}): {} behind at most.",
                duration(furthest)
            );
            format!("{head}\n{}", shard_lines(shards))
        }
        Phase::Ready {
            node_version,
            shards,
        } => format!(
            "Ready (Snapchain {node_version}): every shard within a minute of its newest block.\n{}",
            shard_lines(shards)
        ),
    }
}

fn download_tail(rate: Option<u64>, eta: Option<u64>, since_write: Option<u64>) -> String {
    match (since_write, rate, eta) {
        (None, _, _) => "Starting; nothing downloaded yet.".to_owned(),
        (Some(s), _, _) if s > IDLE_AFTER.as_secs() => format!(
            "Running, but nothing written for {}: waiting for the network.",
            duration(s)
        ),
        (Some(_), Some(r), Some(e)) => format!(
            "{:.1} MB/s over the last 5 minutes; about {} left.",
            r as f64 / 1e6,
            duration(e)
        ),
        (Some(_), _, _) => "Running; too few chunks finished yet to measure speed.".to_owned(),
    }
}

fn shard_lines(shards: &[ShardV1]) -> String {
    shards
        .iter()
        .map(|s| {
            format!(
                "  shard {}: height {}, {} behind",
                s.id,
                s.height,
                duration(s.seconds_behind)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `3h12m`, `4m`, `45s`.
fn duration(secs: u64) -> String {
    match (secs / 3600, secs % 3600 / 60) {
        (0, 0) => format!("{secs}s"),
        (0, m) => format!("{m}m"),
        (h, m) => format!("{h}h{m:02}m"),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use selfcast_core::node::{ShardSync, SyncState};

    use super::{Chunk, Phase, bytes_per_second, duration, eta_seconds, is_low, synced};

    const MB: u64 = 1_000_000;

    fn finished(now: SystemTime, secs_ago: u64, bytes: u64) -> Chunk {
        Chunk {
            finished: now - Duration::from_secs(secs_ago),
            bytes,
        }
    }

    #[test]
    fn rate_counts_only_chunks_finished_in_the_last_five_minutes() {
        let now = SystemTime::now();
        let chunks = [
            finished(now, 3_000, 900 * MB),
            finished(now, 30, 90 * MB),
            finished(now, 20, 90 * MB),
            finished(now, 10, 90 * MB),
        ];
        let rate = bytes_per_second(&chunks, now).expect("three recent chunks");
        assert!((rate - 9.0 * MB as f64).abs() < 1.0, "{rate}");
    }

    #[test]
    fn rate_needs_three_recent_chunks() {
        let now = SystemTime::now();
        let chunks = [finished(now, 20, 90 * MB), finished(now, 10, 90 * MB)];
        assert_eq!(bytes_per_second(&chunks, now), None);
        assert_eq!(bytes_per_second(&[finished(now, 600, MB)], now), None);
    }

    #[test]
    fn eta_is_remaining_chunks_at_the_mean_size() {
        let now = SystemTime::now();
        let done = [finished(now, 1, 80 * MB), finished(now, 2, 100 * MB)];
        assert_eq!(eta_seconds(&done, 10, 9.0 * MB as f64), Some(100));
        assert_eq!(eta_seconds(&[], 10, 9.0 * MB as f64), None);
    }

    fn state(behind: &[u64]) -> SyncState {
        SyncState {
            version: "0.14.3".to_owned(),
            shards: (0..)
                .zip(behind)
                .map(|(id, &seconds_behind)| ShardSync {
                    id,
                    height: 1,
                    seconds_behind,
                })
                .collect(),
        }
    }

    #[test]
    fn ready_only_when_every_shard_is_within_a_minute() {
        assert!(matches!(synced(state(&[1, 60, 0])), Phase::Ready { .. }));
        assert!(matches!(synced(state(&[1, 61, 0])), Phase::Syncing { .. }));
        assert!(matches!(synced(state(&[])), Phase::Syncing { .. }));
    }

    #[test]
    fn disk_is_low_under_fifteen_percent_free() {
        assert!(!is_low(15, 100));
        assert!(is_low(14, 100));
        assert!(!is_low(3_400_000_000_000, 3_600_000_000_000));
        assert!(!is_low(u64::MAX, u64::MAX));
    }

    #[test]
    fn durations_read_as_hours_minutes_or_seconds() {
        assert_eq!(duration(45), "45s");
        assert_eq!(duration(240), "4m");
        assert_eq!(duration(3 * 3600 + 12 * 60 + 5), "3h12m");
        assert_eq!(duration(3600 + 5 * 60), "1h05m");
    }
}
