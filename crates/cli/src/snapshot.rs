//! `selfcast node fetch-snapshot`: a pausable, resumable replacement for the
//! snapshot download Snapchain does on first start. Snapchain's own download
//! cannot resume and, if interrupted after the first shard unpacks, leaves a
//! database it then treats as complete. This one downloads every chunk first,
//! unpacks into `.rocks.partial`, and renames it to `.rocks` only at the end;
//! the node, finding `.rocks` filled, skips its own download.

mod fetch;
mod index;
mod lock;
mod progress;
mod unpack;

pub use lock::holder;
pub use progress::{Chunk, Download, Observed, Unpack};

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use index::{Index, Shard};

#[derive(Debug, clap::Args)]
pub struct FetchArgs {
    /// Snapchain install directory (holds snapchain.sh and .rocks) [default: ~/snapchain]
    #[arg(long, env = "SNAPCHAIN_DIR")]
    dir: Option<PathBuf>,
    /// Chunks to download at once.
    #[arg(long, default_value_t = 4, value_parser = clap::value_parser!(u16).range(1..=32))]
    jobs: u16,
    /// Discard the pinned snapshot and downloaded chunks, and start from the latest snapshot.
    #[arg(long)]
    fresh: bool,
    /// Snapshot server.
    #[arg(long, default_value = index::DEFAULT_URL)]
    url: String,
    /// Fetch only the first N chunks of each shard, then stop before unpacking.
    #[arg(long, hide = true)]
    max_chunks: Option<usize>,
}

#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("HOME is not set; pass --dir")]
    NoHome,
    #[error("{0} already holds a database, so the node needs no snapshot")]
    DbExists(PathBuf),
    #[error("the snapchain container in {0} is running; stop it first: ./snapchain.sh down")]
    NodeRunning(PathBuf),
    #[error("a download is already running{}", .0.map_or(String::new(), |p| format!(" (process {p})")))]
    AlreadyRunning(Option<u32>),
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("network: {0}")]
    Http(#[from] reqwest::Error),
    #[error("{url}: HTTP {status}")]
    Status { url: String, status: u16 },
    #[error(
        "{url} is no longer on the server (the pinned snapshot expired); run again with --fresh"
    )]
    Gone { url: String },
    #[error("{path} failed verification: {reason}")]
    Corrupt { path: PathBuf, reason: String },
    #[error("bad snapshot index from {origin}: {reason}")]
    BadIndex { origin: String, reason: String },
}

impl SnapshotError {
    fn io(path: &Path) -> impl FnOnce(io::Error) -> Self + use<> {
        let path = path.to_path_buf();
        move |source| Self::Io { path, source }
    }

    fn is_transient(&self) -> bool {
        match self {
            Self::Http(_) | Self::Corrupt { .. } => true,
            Self::Status { status, .. } => *status == 429 || *status >= 500,
            _ => false,
        }
    }
}

struct Paths {
    dir: PathBuf,
    db: PathBuf,
    partial: PathBuf,
    work: PathBuf,
}

impl Paths {
    fn new(dir: PathBuf) -> Self {
        Self {
            db: dir.join(".rocks"),
            partial: dir.join(".rocks.partial"),
            work: dir.join(".selfcast-snapshot"),
            dir,
        }
    }

    fn marker(&self, shard: &Shard) -> PathBuf {
        self.work.join(format!("unpacked-{}", shard.id))
    }
}

pub async fn run(args: FetchArgs) -> Result<(), SnapshotError> {
    let paths = Paths::new(install_dir(args.dir)?);
    refuse_if_db_present(&paths.db)?;
    refuse_if_node_running(&paths.dir)?;
    let _lock = lock::acquire(&paths.dir)?;
    if args.fresh {
        remove_dir_if_present(&paths.work)?;
        remove_dir_if_present(&paths.partial)?;
    }
    std::fs::create_dir_all(&paths.work).map_err(SnapshotError::io(&paths.work))?;
    let client = http_client()?;
    let mut index =
        index::load_or_fetch(&client, &args.url, &paths.work.join("index.json")).await?;
    if let Some(n) = args.max_chunks {
        index.shards.iter_mut().for_each(|s| s.truncate(n));
    }
    let todo: Vec<&Shard> = index
        .shards
        .iter()
        .filter(|s| !paths.marker(s).exists())
        .collect();
    fetch::fetch_all(&client, &index, &todo, &paths.work, args.jobs.into()).await?;
    if args.max_chunks.is_some() {
        eprintln!("stopped before unpacking (--max-chunks)");
        return Ok(());
    }
    let index_total = index.total_chunks();
    tokio::task::spawn_blocking(move || unpack_all(&index, &paths).map(|()| paths))
        .await
        .expect("unpack task does not panic")
        .map(|paths| report_done(&paths, index_total))
}

/// The snapshot state in the install directory `dir`.
pub fn observe(dir: &Path) -> Result<Observed, SnapshotError> {
    progress::observe(&Paths::new(dir.to_path_buf()))
}

/// Space on the filesystem holding `dir`, in bytes.
#[derive(Debug, Clone, Copy)]
pub struct Disk {
    pub free: u64,
    pub total: u64,
}

/// Space on the filesystem holding `dir`; `None` if `dir` does not exist.
pub fn disk(dir: &Path) -> Result<Option<Disk>, SnapshotError> {
    match rustix::fs::statvfs(dir) {
        Ok(s) => Ok(Some(Disk {
            free: s.f_bavail.saturating_mul(s.f_frsize),
            total: s.f_blocks.saturating_mul(s.f_frsize),
        })),
        Err(rustix::io::Errno::NOENT) => Ok(None),
        Err(e) => Err(SnapshotError::io(dir)(e.into())),
    }
}

/// `dir`, or `~/snapchain` when `None`.
pub fn install_dir(dir: Option<PathBuf>) -> Result<PathBuf, SnapshotError> {
    match dir {
        Some(d) => Ok(d),
        None => std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join("snapchain"))
            .ok_or(SnapshotError::NoHome),
    }
}

fn refuse_if_db_present(db: &Path) -> Result<(), SnapshotError> {
    match db_filled(db)? {
        true => Err(SnapshotError::DbExists(db.to_path_buf())),
        false => Ok(()),
    }
}

fn db_filled(db: &Path) -> Result<bool, SnapshotError> {
    match std::fs::read_dir(db).map(|mut entries| entries.next().is_some()) {
        Ok(filled) => Ok(filled),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(SnapshotError::io(db)(e)),
    }
}

/// Best effort: without docker on the PATH there is no container to collide with.
fn refuse_if_node_running(dir: &Path) -> Result<(), SnapshotError> {
    let output = Command::new("docker")
        .args([
            "compose",
            "ps",
            "--status",
            "running",
            "--quiet",
            "snapchain",
        ])
        .current_dir(dir)
        .output();
    match output {
        Ok(out) if out.status.success() && !out.stdout.trim_ascii().is_empty() => {
            Err(SnapshotError::NodeRunning(dir.to_path_buf()))
        }
        _ => Ok(()),
    }
}

fn http_client() -> Result<reqwest::Client, SnapshotError> {
    // A read timeout turns a connection that died while the machine slept
    // into an error we retry, instead of a download that hangs forever.
    Ok(reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(60))
        .build()?)
}

fn unpack_all(index: &Index, paths: &Paths) -> Result<(), SnapshotError> {
    for shard in &index.shards {
        let marker = paths.marker(shard);
        if marker.exists() {
            continue;
        }
        eprintln!(
            "unpacking shard {} ({} chunks)",
            shard.id,
            shard.chunks.len()
        );
        let dir = shard.chunk_dir(&paths.work);
        let chunks: Vec<PathBuf> = shard.chunks.iter().map(|c| dir.join(c.as_str())).collect();
        unpack::unpack_shard(&chunks, &paths.partial).map_err(SnapshotError::io(&paths.partial))?;
        std::fs::write(&marker, b"").map_err(SnapshotError::io(&marker))?;
        std::fs::remove_dir_all(&dir).map_err(SnapshotError::io(&dir))?;
    }
    finish(paths)
}

fn finish(paths: &Paths) -> Result<(), SnapshotError> {
    if paths.db.exists() {
        // Only an empty directory reaches here (refuse_if_db_present).
        std::fs::remove_dir(&paths.db).map_err(SnapshotError::io(&paths.db))?;
    }
    std::fs::rename(&paths.partial, &paths.db).map_err(SnapshotError::io(&paths.db))?;
    remove_dir_if_present(&paths.work)
}

fn remove_dir_if_present(dir: &Path) -> Result<(), SnapshotError> {
    match std::fs::remove_dir_all(dir) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(SnapshotError::io(dir)(e)),
        _ => Ok(()),
    }
}

fn report_done(paths: &Paths, chunks: usize) {
    eprintln!(
        "snapshot ready: {chunks} chunks unpacked into {}\nstart the node: cd {} && ./snapchain.sh up",
        paths.db.display(),
        paths.dir.display()
    );
}
