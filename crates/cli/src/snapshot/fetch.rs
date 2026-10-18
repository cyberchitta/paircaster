use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use reqwest::{Client, StatusCode, header::RANGE};
use tokio::io::AsyncWriteExt;
use tokio::task::JoinSet;

use super::SnapshotError;
use super::index::{Index, Shard};
use super::unpack::verify_chunk;

const FIRST_RETRY: Duration = Duration::from_secs(2);
const MAX_RETRY: Duration = Duration::from_secs(60);

/// One chunk still to download: a `.part` file grows until it verifies,
/// then is renamed to its final name, so a final name means "complete".
struct Job {
    label: String,
    url: String,
    part: PathBuf,
    done: PathBuf,
}

struct Progress {
    total: usize,
    done: AtomicUsize,
    fetched_this_run: AtomicUsize,
    bytes_this_run: AtomicU64,
    start: Instant,
}

/// Downloads every chunk of `shards` not already complete under `work`,
/// `jobs` at a time. Network errors are retried forever with backoff, so a
/// sleeping laptop or a dropped connection just waits; Ctrl-C pauses.
pub async fn fetch_all(
    client: &Client,
    index: &Index,
    shards: &[&Shard],
    work: &Path,
    jobs: usize,
) -> Result<(), SnapshotError> {
    let pending = pending_jobs(index, shards, work).await?;
    let total: usize = shards.iter().map(|s| s.chunks.len()).sum();
    let progress = Arc::new(Progress::new(total, total - pending.len()));
    eprintln!(
        "{} of {total} chunks already on disk",
        total - pending.len()
    );
    run_jobs(client, pending, jobs, progress).await
}

async fn pending_jobs(
    index: &Index,
    shards: &[&Shard],
    work: &Path,
) -> Result<Vec<Job>, SnapshotError> {
    let mut jobs = Vec::new();
    for shard in shards {
        let dir = shard.chunk_dir(work);
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(SnapshotError::io(&dir))?;
        for chunk in &shard.chunks {
            let done = dir.join(chunk.as_str());
            if !tokio::fs::try_exists(&done)
                .await
                .map_err(SnapshotError::io(&done))?
            {
                jobs.push(Job {
                    label: format!("shard {} {}", shard.id, chunk.as_str()),
                    url: shard.chunk_url(&index.base_url, chunk),
                    part: dir.join(format!("{}.part", chunk.as_str())),
                    done,
                });
            }
        }
    }
    Ok(jobs)
}

async fn run_jobs(
    client: &Client,
    pending: Vec<Job>,
    jobs: usize,
    progress: Arc<Progress>,
) -> Result<(), SnapshotError> {
    let mut queue = pending.into_iter();
    let mut running = JoinSet::new();
    loop {
        while running.len() < jobs {
            let Some(job) = queue.next() else { break };
            running.spawn(fetch_with_retry(client.clone(), job, progress.clone()));
        }
        match running.join_next().await {
            Some(result) => result.expect("download task does not panic")?,
            None => return Ok(()),
        }
    }
}

async fn fetch_with_retry(
    client: Client,
    job: Job,
    progress: Arc<Progress>,
) -> Result<(), SnapshotError> {
    let mut delay = FIRST_RETRY;
    loop {
        match fetch_once(&client, &job, &progress).await {
            Ok(()) => {
                progress.chunk_done(&job.label);
                return Ok(());
            }
            Err(e) if e.is_transient() => {
                eprintln!("{}: {e}; retrying in {}s", job.label, delay.as_secs());
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(MAX_RETRY);
            }
            Err(e) => return Err(e),
        }
    }
}

async fn fetch_once(client: &Client, job: &Job, progress: &Progress) -> Result<(), SnapshotError> {
    download(client, &job.url, &job.part, progress).await?;
    let part = job.part.clone();
    let verified = tokio::task::spawn_blocking(move || verify_chunk(&part))
        .await
        .expect("verify task does not panic");
    if let Err(e) = verified {
        tokio::fs::remove_file(&job.part)
            .await
            .map_err(SnapshotError::io(&job.part))?;
        return Err(SnapshotError::Corrupt {
            path: job.part.clone(),
            reason: e.to_string(),
        });
    }
    tokio::fs::rename(&job.part, &job.done)
        .await
        .map_err(SnapshotError::io(&job.done))
}

/// Appends to `part` from where it stopped, using an HTTP range request.
async fn download(
    client: &Client,
    url: &str,
    part: &Path,
    progress: &Progress,
) -> Result<(), SnapshotError> {
    let have = existing_len(part).await?;
    let request = match have {
        0 => client.get(url),
        n => client.get(url).header(RANGE, format!("bytes={n}-")),
    };
    let mut resp = request.send().await?;
    let opened = match resp.status() {
        StatusCode::PARTIAL_CONTENT => {
            eprintln!("resuming {} at {:.1} MB", part.display(), have as f64 / 1e6);
            tokio::fs::OpenOptions::new().append(true).open(part).await
        }
        StatusCode::OK => tokio::fs::File::create(part).await,
        StatusCode::RANGE_NOT_SATISFIABLE => return Ok(()),
        StatusCode::NOT_FOUND => {
            return Err(SnapshotError::Gone {
                url: url.to_owned(),
            });
        }
        s => {
            return Err(SnapshotError::Status {
                url: url.to_owned(),
                status: s.as_u16(),
            });
        }
    };
    let mut file = opened.map_err(SnapshotError::io(part))?;
    while let Some(bytes) = resp.chunk().await? {
        file.write_all(&bytes)
            .await
            .map_err(SnapshotError::io(part))?;
        progress
            .bytes_this_run
            .fetch_add(bytes.len() as u64, Ordering::Relaxed);
    }
    file.flush().await.map_err(SnapshotError::io(part))
}

async fn existing_len(path: &Path) -> Result<u64, SnapshotError> {
    match tokio::fs::metadata(path).await {
        Ok(meta) => Ok(meta.len()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(SnapshotError::io(path)(e)),
    }
}

impl Progress {
    fn new(total: usize, done: usize) -> Self {
        Self {
            total,
            done: AtomicUsize::new(done),
            fetched_this_run: AtomicUsize::new(0),
            bytes_this_run: AtomicU64::new(0),
            start: Instant::now(),
        }
    }

    fn chunk_done(&self, label: &str) {
        let done = self.done.fetch_add(1, Ordering::Relaxed) + 1;
        let fetched = self.fetched_this_run.fetch_add(1, Ordering::Relaxed) + 1;
        let secs = self.start.elapsed().as_secs_f64().max(1.0);
        let mb_per_s = self.bytes_this_run.load(Ordering::Relaxed) as f64 / secs / 1e6;
        let eta = secs / fetched as f64 * (self.total - done) as f64;
        eprintln!(
            "[{done}/{}] {label}  {mb_per_s:.1} MB/s  ETA {}",
            self.total,
            hours_minutes(eta)
        );
    }
}

fn hours_minutes(secs: f64) -> String {
    let mins = (secs / 60.0).round() as u64;
    format!("{}h{:02}m", mins / 60, mins % 60)
}
