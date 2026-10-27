//! `paircaster-node status`: which phase the node on this machine is in
//! (downloading the snapshot, unpacking it, stopped, syncing, ready) and how
//! far along.
//!
//! The node is asked first; when nothing answers, the snapshot download is
//! read from its install directory. The disk holding that directory is
//! always reported. A connection this process is not permitted to open, as
//! under an agent's sandbox, is an error: nothing is inferred from it.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use paircaster_core::Node;
use paircaster_core::status::{Synced, duration};
use serde::Serialize;

use crate::NodeError;
use crate::invocation;
use crate::snapshot::{self, Chunk, Download, Observed, Unpack};

/// Download speed is measured over the chunks finished this recently.
const RATE_WINDOW: Duration = Duration::from_secs(300);
/// A running download that has written nothing for longer than this is
/// waiting out a network outage.
const IDLE_AFTER: Duration = Duration::from_secs(90);

#[derive(Debug, clap::Args)]
pub struct StatusArgs {
    /// Snapchain install directory (holds snapchain.sh and .rocks) [default: ~/snapchain]
    #[arg(long, env = "SNAPCHAIN_DIR")]
    dir: Option<PathBuf>,
}

/// `paircaster-node status --json`, version 1. `phase` is one of
/// `not_started`, `downloading`, `unpacking`, `node_stopped`, `syncing`,
/// `ready`, and decides which other fields are present:
///
/// ```json
/// { "version": 1, "phase": "not_started", "running": false }
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
/// lock; `false` means paused or interrupted. `not_started` carries
/// `running` too: `true` while a download has started but saved nothing
/// yet. `bytes_per_second` and
/// `eta_seconds` are `null` while not running, and until a few chunks have
/// finished in the last five minutes; `seconds_since_write` is `null`
/// before anything is written. `ready` has the same fields as `syncing`.
///
/// Every phase also carries `disk`, the filesystem holding the Snapchain
/// install directory, or `null` when that directory does not exist:
/// `"disk": { "free_bytes": 3400000000000, "total_bytes": 3600000000000,
/// "low": false }`. `low` is under 15% free.
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
    NotStarted {
        running: bool,
    },
    Downloading(DownloadingV1),
    Unpacking(UnpackingV1),
    NodeStopped,
    /// `syncing` or `ready`, tagged by [`Synced`] itself.
    #[serde(untagged)]
    Answered(Synced),
}

#[derive(Debug, Serialize)]
struct DownloadingV1 {
    running: bool,
    chunks_done: usize,
    chunks_total: usize,
    bytes_per_second: Option<u64>,
    eta_seconds: Option<u64>,
    seconds_since_write: Option<u64>,
}

#[derive(Debug, Serialize)]
struct UnpackingV1 {
    running: bool,
    shards_done: usize,
    shards_total: usize,
    seconds_since_write: Option<u64>,
}

/// `node_url` is the node's HTTP API on this machine.
pub async fn run(node_url: &str, args: StatusArgs, json: bool) -> Result<(), NodeError> {
    let dir = snapshot::install_dir(args.dir)?;
    let phase = phase(node_url, &dir).await?;
    let disk = local_disk(&dir)?;
    let out = match json {
        true => serde_json::to_string(&StatusV1 {
            version: 1,
            phase,
            disk,
        })
        .expect("a status always serialises"),
        false => format!(
            "{}\n{}",
            describe(&phase, node_url, &dir, disk.is_some()),
            disk_line(disk.as_ref(), &dir)
        ),
    };
    crate::out::line(format_args!("{out}"));
    Ok(())
}

/// The node's own answer; when it does not answer, what its install
/// directory shows.
async fn phase(node_url: &str, dir: &Path) -> Result<Phase, NodeError> {
    match Node::new(node_url)?.sync_state().await {
        Ok(state) => Ok(Phase::Answered(Synced::from_state(state))),
        Err(e) if e.is_denied() => Err(StatusError::NetworkDenied(node_url.to_owned()).into()),
        Err(paircaster_core::Error::Transport(_)) => Ok(on_disk(
            snapshot::observe(dir)?,
            snapshot::holder(dir)?.is_some(),
            SystemTime::now(),
        )),
        Err(e) => Err(e.into()),
    }
}

fn local_disk(dir: &Path) -> Result<Option<DiskV1>, NodeError> {
    Ok(snapshot::disk(dir)?.map(|d| DiskV1 {
        free_bytes: d.free,
        total_bytes: d.total,
        low: is_low(d.free, d.total),
    }))
}

/// `status` could not learn the node's state.
#[derive(Debug, thiserror::Error)]
pub enum StatusError {
    #[error(
        "not permitted to connect to {0}, so the node's state is unknown; an agent's sandbox may be blocking network access for paircaster-node"
    )]
    NetworkDenied(String),
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

/// `locked`: a download holds the lock, which it takes before saving anything.
fn on_disk(observed: Observed, locked: bool, now: SystemTime) -> Phase {
    match observed {
        Observed::Nothing => Phase::NotStarted { running: locked },
        Observed::Database => Phase::NodeStopped,
        Observed::Downloading(d) => downloading(&d, now),
        Observed::Unpacking(u) => unpacking(&u, now),
    }
}

fn downloading(d: &Download, now: SystemTime) -> Phase {
    let rate = bytes_per_second(&d.chunks, now).filter(|_| d.running);
    let eta = rate.and_then(|r| eta_seconds(&d.chunks, d.chunks_total - d.chunks_done, r));
    Phase::Downloading(DownloadingV1 {
        running: d.running,
        chunks_done: d.chunks_done,
        chunks_total: d.chunks_total,
        bytes_per_second: rate.map(|r| r as u64),
        eta_seconds: eta,
        seconds_since_write: d.last_write.map(|t| age(now, t).as_secs()),
    })
}

fn unpacking(u: &Unpack, now: SystemTime) -> Phase {
    Phase::Unpacking(UnpackingV1 {
        running: u.running,
        shards_done: u.shards_done,
        shards_total: u.shards_total,
        seconds_since_write: u.last_write.map(|t| age(now, t).as_secs()),
    })
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

fn not_started(node_url: &str, dir: &Path, installed: bool) -> String {
    let fetch = invocation::node("fetch-snapshot", dir);
    let next = match installed {
        true => format!("Start with: {fetch}"),
        false => format!(
            "Snapchain is not installed at {} (pass --dir if it is elsewhere).\n\
             Install it, stop the node it starts, then run: {fetch}",
            dir.display()
        ),
    };
    format!("No snapshot download started, and no node answers at {node_url}.\n{next}")
}

/// How long ago `then` was; zero for a time in the future (clock skew).
fn age(now: SystemTime, then: SystemTime) -> Duration {
    now.duration_since(then).unwrap_or(Duration::ZERO)
}

/// `installed`: the install directory exists on this machine.
fn describe(phase: &Phase, node_url: &str, dir: &Path, installed: bool) -> String {
    match phase {
        Phase::NotStarted { running: true } => format!(
            "The snapshot download is starting; nothing saved yet.\n\
             Check again shortly: {}",
            invocation::node("status", dir)
        ),
        Phase::NotStarted { running: false } => not_started(node_url, dir, installed),
        Phase::Downloading(d) => describe_download(d, dir),
        Phase::Unpacking(u) => describe_unpack(u, dir),
        Phase::NodeStopped => format!(
            "The snapshot is in place, but no node answers at {node_url}.\n\
             Start it: cd {} && ./snapchain.sh up (it can take a minute to answer)",
            invocation::path(dir)
        ),
        Phase::Answered(synced) => synced.describe(),
    }
}

fn describe_download(d: &DownloadingV1, dir: &Path) -> String {
    let (done, total) = (d.chunks_done, d.chunks_total);
    let pct = done * 100 / total;
    let head = format!("Downloading the snapshot: {done} of {total} chunks ({pct}%).");
    let tail = match d.running {
        true => download_tail(d.bytes_per_second, d.eta_seconds, d.seconds_since_write),
        false => format!("Paused. To resume: {}", invocation::node("resume", dir)),
    };
    format!("{head}\n{tail}")
}

fn describe_unpack(u: &UnpackingV1, dir: &Path) -> String {
    let (done, total) = (u.shards_done, u.shards_total);
    let head = format!("Unpacking the snapshot: {done} of {total} shards done.");
    match u.running {
        true => head,
        false => format!(
            "{head}\nInterrupted. To finish (the current shard starts over): {}",
            invocation::node("resume", dir)
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

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    use paircaster_core::node::{ShardSync, SyncState};

    use super::{
        Chunk, Download, Observed, Phase, StatusV1, Synced, Unpack, bytes_per_second, eta_seconds,
        is_low, not_started, on_disk,
    };

    const MB: u64 = 1_000_000;

    fn json(phase: Phase) -> String {
        serde_json::to_string(&StatusV1 {
            version: 1,
            phase,
            disk: None,
        })
        .expect("serialises")
    }

    #[test]
    fn json_keeps_its_shape_for_each_snapshot_phase() {
        let now = SystemTime::now();
        let download = Download {
            running: true,
            chunks_done: 2,
            chunks_total: 5,
            chunks: vec![],
            last_write: Some(now),
        };
        let unpack = Unpack {
            running: false,
            shards_done: 1,
            shards_total: 3,
            last_write: None,
        };
        assert_eq!(
            json(on_disk(Observed::Downloading(download), true, now)),
            r#"{"version":1,"phase":"downloading","running":true,"chunks_done":2,"chunks_total":5,"bytes_per_second":null,"eta_seconds":null,"seconds_since_write":0,"disk":null}"#
        );
        assert_eq!(
            json(on_disk(Observed::Unpacking(unpack), false, now)),
            r#"{"version":1,"phase":"unpacking","running":false,"shards_done":1,"shards_total":3,"seconds_since_write":null,"disk":null}"#
        );
    }

    #[test]
    fn json_keeps_its_shape_once_the_node_answers() {
        let state = SyncState {
            version: "0.15.0".to_owned(),
            shards: vec![ShardSync {
                id: 1,
                height: 7,
                seconds_behind: 90,
            }],
        };
        assert_eq!(
            json(Phase::Answered(Synced::from_state(state))),
            r#"{"version":1,"phase":"syncing","node_version":"0.15.0","shards":[{"id":1,"height":7,"seconds_behind":90}],"disk":null}"#
        );
    }

    #[test]
    fn not_started_without_an_install_says_to_install_snapchain_first() {
        let text = not_started(
            "http://localhost:3381",
            Path::new("/home/u/snapchain"),
            false,
        );
        assert!(text.contains("Snapchain is not installed at /home/u/snapchain"));
        assert!(text.contains("Install it, stop the node it starts, then run:"));
    }

    #[test]
    fn not_started_with_an_install_says_to_fetch_the_snapshot() {
        let text = not_started(
            "http://localhost:3381",
            Path::new("/home/u/snapchain"),
            true,
        );
        assert!(!text.contains("not installed"));
        assert!(text.contains("Start with:"));
        assert!(text.contains("fetch-snapshot"));
    }

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

    #[test]
    fn nothing_on_disk_is_running_while_a_download_holds_the_lock() {
        let now = SystemTime::now();
        assert!(matches!(
            on_disk(Observed::Nothing, true, now),
            Phase::NotStarted { running: true }
        ));
        assert!(matches!(
            on_disk(Observed::Nothing, false, now),
            Phase::NotStarted { running: false }
        ));
    }

    #[test]
    fn disk_is_low_under_fifteen_percent_free() {
        assert!(!is_low(15, 100));
        assert!(is_low(14, 100));
        assert!(!is_low(3_400_000_000_000, 3_600_000_000_000));
        assert!(!is_low(u64::MAX, u64::MAX));
    }
}
