use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;

/// Unpacks one shard: each chunk is its own gzip stream of a slice of one
/// tar archive. Mirrors snapchain `download_snapshots`, which gunzips every
/// chunk, concatenates them and unpacks with the same `tar` crate defaults,
/// but streams instead of writing the intermediate tar to disk.
pub fn unpack_shard(chunks: &[PathBuf], dest: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dest)?;
    tar::Archive::new(GzChain::new(chunks.to_vec())).unpack(dest)
}

/// Reads the decompressed chunks back to back, opening one file at a time.
struct GzChain {
    rest: std::vec::IntoIter<PathBuf>,
    current: Option<GzDecoder<BufReader<File>>>,
}

impl GzChain {
    fn new(paths: Vec<PathBuf>) -> Self {
        Self {
            rest: paths.into_iter(),
            current: None,
        }
    }
}

impl Read for GzChain {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if let Some(decoder) = &mut self.current {
                let n = decoder.read(buf)?;
                if n > 0 || buf.is_empty() {
                    return Ok(n);
                }
            }
            match self.rest.next() {
                Some(path) => {
                    self.current = Some(GzDecoder::new(BufReader::new(File::open(path)?)))
                }
                None => return Ok(0),
            }
        }
    }
}

/// Checks that a downloaded chunk is a complete, uncorrupted gzip stream.
pub fn verify_chunk(path: &Path) -> io::Result<()> {
    let mut decoder = GzDecoder::new(BufReader::new(File::open(path)?));
    io::copy(&mut decoder, &mut io::sink()).map(|_| ())
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::path::{Path, PathBuf};

    use flate2::{Compression, write::GzEncoder};

    use super::{unpack_shard, verify_chunk};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("paircaster-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir is writable");
        dir
    }

    fn tar_of(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        for (name, data) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, name, data.as_slice())
                .expect("append");
        }
        builder.into_inner().expect("finish tar")
    }

    fn write_chunks(tar: &[u8], size: usize, dir: &Path) -> Vec<PathBuf> {
        tar.chunks(size)
            .enumerate()
            .map(|(i, slice)| {
                let path = dir.join(format!("chunk_{:04}.bin", i + 1));
                let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
                gz.write_all(slice).expect("gzip");
                std::fs::write(&path, gz.finish().expect("gzip")).expect("write chunk");
                path
            })
            .collect()
    }

    #[test]
    fn chunks_split_mid_file_unpack_to_the_original_tree() {
        let dir = scratch("unpack");
        let big: Vec<u8> = (0..100_000u32).flat_map(|i| i.to_le_bytes()).collect();
        let files = [
            ("shard-1/000001.sst", big.clone()),
            ("shard-1/CURRENT", b"MANIFEST-1\n".to_vec()),
        ];
        let chunks = write_chunks(&tar_of(&files), 7_000, &dir);
        assert!(chunks.len() > 10);
        chunks
            .iter()
            .for_each(|c| verify_chunk(c).expect("chunk verifies"));
        unpack_shard(&chunks, &dir.join("db")).expect("unpack");
        assert_eq!(
            std::fs::read(dir.join("db/shard-1/000001.sst")).expect("read"),
            big
        );
        assert_eq!(
            std::fs::read(dir.join("db/shard-1/CURRENT")).expect("read"),
            b"MANIFEST-1\n"
        );
    }

    #[test]
    fn a_truncated_chunk_fails_verification() {
        let dir = scratch("verify");
        let chunks = write_chunks(&tar_of(&[("a", vec![7; 50_000])]), 1 << 20, &dir);
        let bytes = std::fs::read(&chunks[0]).expect("read");
        std::fs::write(&chunks[0], &bytes[..bytes.len() / 2]).expect("truncate");
        assert!(verify_chunk(&chunks[0]).is_err());
    }
}
