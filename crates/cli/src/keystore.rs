//! App keys on disk, under `$XDG_CONFIG_HOME/selfcast/keys`, else
//! `~/.config/selfcast/keys`. The directory is mode 0700 and every file in
//! it 0600; a directory open to others is refused.
//!
//! - `<fid>-<public key hex>.key`: the app key's secret, as hex.
//! - `<fid>-<public key hex>.keyadd`: the signed `KEY_ADD` that registers
//!   it, saved before it is sent and deleted once the node lists the key,
//!   so a failed run resends the same bytes rather than signing again.
//!
//! Custody keys and recovery phrases are never written here or anywhere.

use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use selfcast_core::build::AppKey;
use selfcast_core::cast::SignerKey;
use zeroize::Zeroizing;

const PENDING: &str = "keyadd";

#[derive(Debug, thiserror::Error)]
pub enum KeyStoreError {
    #[error("neither XDG_CONFIG_HOME nor HOME is set")]
    NoHome,
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("{path} is open to other users (mode {mode:o}); run: chmod 700 {path}")]
    TooOpen { path: PathBuf, mode: u32 },
}

fn io_err(path: &Path) -> impl FnOnce(io::Error) -> KeyStoreError + use<> {
    let path = path.to_path_buf();
    move |source| KeyStoreError::Io { path, source }
}

pub struct KeyStore {
    dir: PathBuf,
}

impl KeyStore {
    pub fn open() -> Result<Self, KeyStoreError> {
        Self::at(config_dir()?.join("selfcast").join("keys"))
    }

    /// The store in `dir`, created 0700 if missing.
    fn at(dir: PathBuf) -> Result<Self, KeyStoreError> {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .map_err(io_err(&dir))?;
        let mode = fs::metadata(&dir)
            .map_err(io_err(&dir))?
            .permissions()
            .mode()
            & 0o777;
        match mode & 0o077 {
            0 => Ok(Self { dir }),
            _ => Err(KeyStoreError::TooOpen { path: dir, mode }),
        }
    }

    pub fn key_path(&self, fid: u64, key: &SignerKey) -> PathBuf {
        self.dir
            .join(format!("{fid}-{}.key", hex::encode(key.as_bytes())))
    }

    fn pending_path(&self, fid: u64, key: &SignerKey) -> PathBuf {
        self.key_path(fid, key).with_extension(PENDING)
    }

    pub fn save_key(&self, fid: u64, key: &AppKey) -> Result<PathBuf, KeyStoreError> {
        let path = self.key_path(fid, &key.public());
        let text = Zeroizing::new(format!("{}\n", hex::encode(*key.to_secret())));
        write_new(&path, text.as_bytes())?;
        Ok(path)
    }

    pub fn save_pending(
        &self,
        fid: u64,
        key: &SignerKey,
        message: &[u8],
    ) -> Result<PathBuf, KeyStoreError> {
        let path = self.pending_path(fid, key);
        write_new(&path, message)?;
        Ok(path)
    }

    /// Saved `KEY_ADD`s not yet seen registered.
    pub fn pending(&self) -> Result<Vec<PathBuf>, KeyStoreError> {
        let entries = fs::read_dir(&self.dir).map_err(io_err(&self.dir))?;
        let mut paths = entries
            .map(|e| e.map(|e| e.path()).map_err(io_err(&self.dir)))
            .filter(|p| {
                p.as_ref()
                    .map_or(true, |p| p.extension().is_some_and(|e| e == PENDING))
            })
            .collect::<Result<Vec<_>, _>>()?;
        paths.sort();
        Ok(paths)
    }
}

pub fn read(path: &Path) -> Result<Vec<u8>, KeyStoreError> {
    fs::read(path).map_err(io_err(path))
}

pub fn remove(path: &Path) -> Result<(), KeyStoreError> {
    fs::remove_file(path).map_err(io_err(path))
}

fn config_dir() -> Result<PathBuf, KeyStoreError> {
    match (
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
    ) {
        (Some(xdg), _) if !xdg.is_empty() => Ok(PathBuf::from(xdg)),
        (_, Some(home)) => Ok(PathBuf::from(home).join(".config")),
        _ => Err(KeyStoreError::NoHome),
    }
}

/// Creates `path` with mode 0600, refusing to overwrite.
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), KeyStoreError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(io_err(path))?;
    file.write_all(bytes).map_err(io_err(path))?;
    file.sync_all().map_err(io_err(path))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    use selfcast_core::build::AppKey;

    use super::{KeyStore, KeyStoreError};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("selfcast-ks-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn mode(path: &std::path::Path) -> u32 {
        fs::metadata(path).expect("exists").permissions().mode() & 0o777
    }

    #[test]
    fn keys_are_written_0600_in_a_0700_directory_and_never_overwritten() {
        let dir = scratch("modes");
        let store = KeyStore::at(dir.join("keys")).expect("opens");
        let key = AppKey::from_secret(&[0x11; 32]);
        let path = store.save_key(7, &key).expect("saves");
        assert_eq!(mode(&dir.join("keys")), 0o700);
        assert_eq!(mode(&path), 0o600);
        let text = fs::read_to_string(&path).expect("reads");
        assert_eq!(text.trim(), hex::encode([0x11; 32]));
        assert!(store.save_key(7, &key).is_err());
    }

    #[test]
    fn pending_lists_only_saved_key_adds() {
        let store = KeyStore::at(scratch("pending")).expect("opens");
        let key = AppKey::from_secret(&[0x11; 32]);
        store.save_key(7, &key).expect("saves key");
        let pending = store.save_pending(7, &key.public(), b"msg").expect("saves");
        assert_eq!(store.pending().expect("lists"), [pending]);
    }

    #[test]
    fn a_directory_open_to_others_is_refused() {
        let dir = scratch("open");
        fs::create_dir_all(&dir).expect("creates");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).expect("chmod");
        assert!(matches!(
            KeyStore::at(dir),
            Err(KeyStoreError::TooOpen { mode: 0o755, .. })
        ));
    }
}
