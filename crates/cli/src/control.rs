//! `selfcast node pause` and `selfcast node resume`: stop and restart the
//! snapshot download without finding its process by hand.
//!
//! The running download is the process holding the download lock
//! (`snapshot::holder`). Pausing interrupts it; everything downloaded stays
//! on disk, including the part of each chunk in flight. Resuming starts
//! `fetch-snapshot` detached, in its own process group, so closing the
//! terminal or session that started it does not stop it. On macOS it runs
//! under `caffeinate -i`, so the machine does not sleep while it works.
//!
//! The unpack is not paused: it is local, and stopping it midway is
//! untested.

use std::fs::{File, OpenOptions};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rustix::process::{Pid, Signal, kill_process};
use selfcast_core::time::Timestamp;
use std::io::Write;

use crate::CliError;
use crate::snapshot::{self, Observed};

const CAFFEINATE: &str = "/usr/bin/caffeinate";
const LOG: &str = "selfcast-fetch.log";
/// How long to wait for a download to stop, or to start.
const WAIT_TRIES: u32 = 50;
const WAIT_EVERY: Duration = Duration::from_millis(200);

#[derive(Debug, clap::Args)]
pub struct PauseArgs {
    /// Snapchain install directory (holds snapchain.sh and .rocks) [default: ~/snapchain]
    #[arg(long, env = "SNAPCHAIN_DIR")]
    dir: Option<PathBuf>,
}

#[derive(Debug, clap::Args)]
pub struct ResumeArgs {
    /// Snapchain install directory (holds snapchain.sh and .rocks) [default: ~/snapchain]
    #[arg(long, env = "SNAPCHAIN_DIR")]
    dir: Option<PathBuf>,
    /// Chunks to download at once.
    #[arg(long, default_value_t = 4, value_parser = clap::value_parser!(u16).range(1..=32))]
    jobs: u16,
}

#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    #[error(
        "the snapshot is unpacking; that is local and not paused (stopping it midway is untested)"
    )]
    Unpacking,
    #[error("process {0} holds the download lock but cannot be signalled: {1}")]
    Signal(u32, std::io::Error),
    #[error("process {0} did not stop within 10 seconds")]
    DidNotStop(u32),
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("the download did not start; see {}", .0.display())]
    DidNotStart(PathBuf),
}

pub async fn pause(args: PauseArgs) -> Result<(), CliError> {
    let dir = snapshot::install_dir(args.dir)?;
    if let Observed::Unpacking(_) = snapshot::observe(&dir)? {
        return Err(ControlError::Unpacking.into());
    }
    let Some(pid) = snapshot::holder(&dir)? else {
        println!("No download is running.");
        return Ok(());
    };
    let target = Pid::from_raw(pid as i32).ok_or(ControlError::DidNotStop(pid))?;
    kill_process(target, Signal::INT).map_err(|e| ControlError::Signal(pid, e.into()))?;
    for _ in 0..WAIT_TRIES {
        if snapshot::holder(&dir)?.is_none() {
            println!("Paused. {}", progress(&dir)?);
            return Ok(());
        }
        tokio::time::sleep(WAIT_EVERY).await;
    }
    Err(ControlError::DidNotStop(pid).into())
}

pub async fn resume(args: ResumeArgs) -> Result<(), CliError> {
    let dir = snapshot::install_dir(args.dir)?;
    if let Some(pid) = snapshot::holder(&dir)? {
        println!("Already running (process {pid}).");
        return Ok(());
    }
    if let Observed::Database = snapshot::observe(&dir)? {
        println!("The snapshot is already unpacked; nothing to resume.");
        return Ok(());
    }
    let log = dir.join(LOG);
    let mut child = spawn(&dir, args.jobs, &log)?;
    for _ in 0..WAIT_TRIES {
        if let Some(pid) = snapshot::holder(&dir)? {
            println!(
                "Resumed (process {pid}). {}\nLog: {}\nCheck: selfcast node status\nPause: selfcast node pause",
                progress(&dir)?,
                log.display()
            );
            return Ok(());
        }
        if child.try_wait().map_err(io_err(&log))?.is_some() {
            break;
        }
        tokio::time::sleep(WAIT_EVERY).await;
    }
    Err(ControlError::DidNotStart(log).into())
}

/// `fetch-snapshot`, detached, appending its output to `log`.
fn spawn(dir: &Path, jobs: u16, log: &Path) -> Result<Child, ControlError> {
    let mut out = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .map_err(io_err(log))?;
    writeln!(out, "=== resume {} --jobs {jobs}", now()).map_err(io_err(log))?;
    let err: File = out.try_clone().map_err(io_err(log))?;
    let exe = std::env::current_exe().map_err(io_err(log))?;
    let mut command = match Path::new(CAFFEINATE).exists() {
        true => {
            let mut c = Command::new(CAFFEINATE);
            c.arg("-i").arg(exe);
            c
        }
        false => Command::new(exe),
    };
    command
        .args([
            "node",
            "fetch-snapshot",
            "--jobs",
            &jobs.to_string(),
            "--dir",
        ])
        .arg(dir)
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        .process_group(0)
        .spawn()
        .map_err(io_err(log))
}

/// One line on how far the download has got.
fn progress(dir: &Path) -> Result<String, CliError> {
    Ok(match snapshot::observe(dir)? {
        Observed::Downloading(d) => {
            format!("{} of {} chunks on disk.", d.chunks_done, d.chunks_total)
        }
        Observed::Unpacking(u) => format!(
            "Unpacking: {} of {} shards done.",
            u.shards_done, u.shards_total
        ),
        Observed::Nothing => "Nothing downloaded yet.".to_owned(),
        Observed::Database => "The snapshot is unpacked.".to_owned(),
    })
}

fn now() -> Timestamp {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_secs();
    Timestamp::from_unix(secs)
}

fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> ControlError + use<> {
    let path = path.to_path_buf();
    move |source| ControlError::Io { path, source }
}
