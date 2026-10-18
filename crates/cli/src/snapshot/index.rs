use std::path::{Path, PathBuf};

use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::SnapshotError;

/// Snapchain's mainnet snapshot bucket (`snapshot_download_url` default in
/// snapchain `src/storage/db/snapshot.rs`).
pub const DEFAULT_URL: &str = "https://pub-d352dd8819104a778e20d08888c5a661.r2.dev";
const NETWORK: &str = "FARCASTER_NETWORK_MAINNET";
/// The node's `shard_ids` plus shard 0, in the order Snapchain unpacks them.
const SHARDS: [u32; 3] = [1, 2, 0];
const INDEX_VERSION: u32 = 1;

/// The pinned list of chunks for one snapshot, saved so that a paused
/// download resumes against the same snapshot rather than a newer one.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Index {
    version: u32,
    pub base_url: String,
    pub shards: Vec<Shard>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shard {
    pub id: u32,
    key_base: String,
    pub chunks: Vec<ChunkName>,
}

/// A chunk file name as listed by the server, checked to be a plain
/// `chunk_NNNN.bin` so a hostile index cannot write outside our directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ChunkName(String);

/// The server's `latest.json` for one shard.
#[derive(Debug, Deserialize)]
struct Latest {
    key_base: String,
    chunks: Vec<String>,
}

impl TryFrom<String> for ChunkName {
    type Error = String;

    fn try_from(s: String) -> Result<Self, String> {
        let digits = s
            .strip_prefix("chunk_")
            .and_then(|r| r.strip_suffix(".bin"));
        match digits {
            Some(d) if !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()) => Ok(Self(s)),
            _ => Err(format!("not a chunk file name: {s:?}")),
        }
    }
}

impl From<ChunkName> for String {
    fn from(c: ChunkName) -> String {
        c.0
    }
}

impl ChunkName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Shard {
    pub fn chunk_url(&self, base_url: &str, chunk: &ChunkName) -> String {
        format!("{base_url}/{}/{}", self.key_base, chunk.as_str())
    }

    pub fn chunk_dir(&self, work: &Path) -> PathBuf {
        work.join(format!("shard-{}", self.id))
    }

    /// Keep only the first `n` chunks, for trying the download path cheaply.
    pub fn truncate(&mut self, n: usize) {
        self.chunks.truncate(n);
    }
}

impl Index {
    pub fn total_chunks(&self) -> usize {
        self.shards.iter().map(|s| s.chunks.len()).sum()
    }
}

/// Loads the pinned index from `path`, or fetches the latest one and pins it.
pub async fn load_or_fetch(
    client: &Client,
    base_url: &str,
    path: &Path,
) -> Result<Index, SnapshotError> {
    match tokio::fs::read(path).await {
        Ok(bytes) => parse_pinned(&bytes, path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let index = fetch(client, base_url).await?;
            let json = serde_json::to_vec_pretty(&index).expect("an Index always serialises");
            tokio::fs::write(path, json)
                .await
                .map_err(SnapshotError::io(path))?;
            Ok(index)
        }
        Err(e) => Err(SnapshotError::io(path)(e)),
    }
}

fn parse_pinned(bytes: &[u8], path: &Path) -> Result<Index, SnapshotError> {
    let index: Index = serde_json::from_slice(bytes).map_err(|e| SnapshotError::BadIndex {
        origin: path.display().to_string(),
        reason: e.to_string(),
    })?;
    match index.version {
        INDEX_VERSION => Ok(index),
        v => Err(SnapshotError::BadIndex {
            origin: path.display().to_string(),
            reason: format!("index version {v}, expected {INDEX_VERSION}"),
        }),
    }
}

async fn fetch(client: &Client, base_url: &str) -> Result<Index, SnapshotError> {
    let mut shards = Vec::with_capacity(SHARDS.len());
    for id in SHARDS {
        shards.push(fetch_shard(client, base_url, id).await?);
    }
    Ok(Index {
        version: INDEX_VERSION,
        base_url: base_url.to_owned(),
        shards,
    })
}

async fn fetch_shard(client: &Client, base_url: &str, id: u32) -> Result<Shard, SnapshotError> {
    let url = format!("{base_url}/{NETWORK}/{id}/latest.json");
    let resp = client.get(&url).send().await?.error_for_status()?;
    let latest: Latest = resp.json().await?;
    let bad = |reason: String| SnapshotError::BadIndex {
        origin: url.clone(),
        reason,
    };
    if latest
        .key_base
        .split('/')
        .any(|seg| seg.is_empty() || seg == "..")
    {
        return Err(bad(format!("unsafe key_base {:?}", latest.key_base)));
    }
    let chunks = latest
        .chunks
        .into_iter()
        .map(ChunkName::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(bad)?;
    Ok(Shard {
        id,
        key_base: latest.key_base,
        chunks,
    })
}

#[cfg(test)]
mod tests {
    use super::ChunkName;

    #[test]
    fn chunk_names_must_be_plain() {
        assert!(ChunkName::try_from("chunk_0001.bin".to_owned()).is_ok());
        for bad in [
            "../x.bin",
            "chunk_.bin",
            "chunk_01.bin/..",
            "chunk_1a.bin",
            "x",
        ] {
            assert!(ChunkName::try_from(bad.to_owned()).is_err(), "{bad}");
        }
    }
}
