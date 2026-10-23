//! App keys on disk, under `$XDG_CONFIG_HOME/paircaster/keys`, else
//! `~/.config/paircaster/keys`. The directory is mode 0700 and every file in
//! it 0600; a directory open to others is refused.
//!
//! - `<fid>-<public key hex>.key`: the app key's secret, as hex.
//! - `<fid>-<public key hex>.keyadd`: the signed `KEY_ADD` that registers
//!   it, saved before it is sent and deleted once the node lists the key,
//!   so a failed run resends the same bytes rather than signing again.
//!
//! A key file is loaded only at mode 0600, and only if its secret's public
//! half matches its name.
//!
//! Custody keys and recovery phrases are never written here or anywhere.

use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use paircaster_core::build::AppKey;
use paircaster_core::cast::SignerKey;
use zeroize::Zeroizing;

use crate::invocation;

const PENDING: &str = "keyadd";
const KEY: &str = "key";

#[derive(Debug, thiserror::Error)]
pub enum KeyStoreError {
    #[error("neither XDG_CONFIG_HOME nor HOME is set")]
    NoHome,
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("{path} is open to other users (mode {mode:o}); run: chmod 700 {path}")]
    TooOpen { path: PathBuf, mode: u32 },
    #[error("{path} is readable by other users (mode {mode:o}); run: chmod 600 {path}")]
    KeyTooOpen { path: PathBuf, mode: u32 },
    #[error("{path} is not a Paircaster app key: {reason}")]
    BadKeyFile { path: PathBuf, reason: &'static str },
    #[error("no app key saved{}; run `{} key add` first", for_fid(*.fid), invocation::paircaster())]
    NoKey { fid: Option<u64> },
    #[error("several app keys saved; pass --fid to pick the account, or remove all but one: {}", list(.0))]
    SeveralKeys(Vec<PathBuf>),
}

fn for_fid(fid: Option<u64>) -> String {
    fid.map_or(String::new(), |f| format!(" for FID {f}"))
}

fn list(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// An app key read back from the store.
pub struct LoadedKey {
    pub fid: u64,
    pub key: AppKey,
    pub path: PathBuf,
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
        Self::at(config_dir()?.join("paircaster").join("keys"))
    }

    /// The store in `dir`, created 0700 if missing.
    fn at(dir: PathBuf) -> Result<Self, KeyStoreError> {
        private_dir(&dir)?;
        Ok(Self { dir })
    }

    /// The one saved key for `fid`, or the only saved key when `fid` is
    /// `None`.
    pub fn load(&self, fid: Option<u64>) -> Result<LoadedKey, KeyStoreError> {
        let mut found: Vec<(u64, PathBuf)> = self
            .with_extension(KEY)?
            .into_iter()
            .filter_map(|p| key_fid(&p).map(|f| (f, p)))
            .filter(|(f, _)| fid.is_none_or(|want| *f == want))
            .collect();
        match found.len() {
            0 => Err(KeyStoreError::NoKey { fid }),
            1 => {
                let (fid, path) = found.remove(0);
                Ok(LoadedKey {
                    fid,
                    key: read_key(&path)?,
                    path,
                })
            }
            _ => Err(KeyStoreError::SeveralKeys(
                found.into_iter().map(|(_, p)| p).collect(),
            )),
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
        self.with_extension(PENDING)
    }

    fn with_extension(&self, extension: &str) -> Result<Vec<PathBuf>, KeyStoreError> {
        files_with_extension(&self.dir, extension)
    }
}

/// The files in `dir` named `*.<extension>`, sorted.
pub fn files_with_extension(dir: &Path, extension: &str) -> Result<Vec<PathBuf>, KeyStoreError> {
    let entries = fs::read_dir(dir).map_err(io_err(dir))?;
    let mut paths = entries
        .map(|e| e.map(|e| e.path()).map_err(io_err(dir)))
        .filter(|p| {
            p.as_ref()
                .map_or(true, |p| p.extension().is_some_and(|e| e == extension))
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    Ok(paths)
}

/// `dir`, created 0700 if missing, refused if open to others.
pub fn private_dir(dir: &Path) -> Result<(), KeyStoreError> {
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(io_err(dir))?;
    let mode = mode_of(dir)?;
    match mode & 0o077 {
        0 => Ok(()),
        _ => Err(KeyStoreError::TooOpen {
            path: dir.to_path_buf(),
            mode,
        }),
    }
}

fn mode_of(path: &Path) -> Result<u32, KeyStoreError> {
    Ok(fs::metadata(path)
        .map_err(io_err(path))?
        .permissions()
        .mode()
        & 0o777)
}

/// The FID a key file's name starts with: `<fid>-<public key hex>.key`.
fn key_fid(path: &Path) -> Option<u64> {
    let stem = path.file_stem()?.to_str()?;
    stem.split_once('-')?.0.parse().ok()
}

fn read_key(path: &Path) -> Result<AppKey, KeyStoreError> {
    let bad = |reason| KeyStoreError::BadKeyFile {
        path: path.to_path_buf(),
        reason,
    };
    let mode = mode_of(path)?;
    if mode & 0o077 != 0 {
        return Err(KeyStoreError::KeyTooOpen {
            path: path.to_path_buf(),
            mode,
        });
    }
    let text = Zeroizing::new(fs::read_to_string(path).map_err(io_err(path))?);
    let mut secret = Zeroizing::new([0u8; 32]);
    hex::decode_to_slice(text.trim(), &mut *secret).map_err(|_| bad("not 64 hex digits"))?;
    let key = AppKey::from_secret(&secret);
    let named = path
        .file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.split_once('-'));
    match named {
        Some((_, public)) if public == hex::encode(key.public().as_bytes()) => Ok(key),
        _ => Err(bad("its name does not match its key")),
    }
}

pub fn read(path: &Path) -> Result<Vec<u8>, KeyStoreError> {
    fs::read(path).map_err(io_err(path))
}

pub fn remove(path: &Path) -> Result<(), KeyStoreError> {
    fs::remove_file(path).map_err(io_err(path))
}

pub fn config_dir() -> Result<PathBuf, KeyStoreError> {
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
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<(), KeyStoreError> {
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

    use paircaster_core::build::AppKey;

    use super::{KeyStore, KeyStoreError};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("paircaster-ks-{name}-{}", std::process::id()));
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
    fn a_saved_key_loads_back_by_its_account() {
        let store = KeyStore::at(scratch("load")).expect("opens");
        let key = AppKey::from_secret(&[0x11; 32]);
        let path = store.save_key(7, &key).expect("saves");
        let loaded = store.load(None).expect("the only key loads");
        assert_eq!((loaded.fid, loaded.key.public()), (7, key.public()));
        assert_eq!(loaded.path, path);
        assert!(store.load(Some(7)).is_ok());
        assert!(matches!(
            store.load(Some(8)),
            Err(KeyStoreError::NoKey { fid: Some(8) })
        ));
        store
            .save_key(8, &AppKey::from_secret(&[0x22; 32]))
            .expect("saves");
        assert!(matches!(
            store.load(None),
            Err(KeyStoreError::SeveralKeys(_))
        ));
        assert_eq!(store.load(Some(8)).expect("loads").fid, 8);
    }

    #[test]
    fn a_key_readable_by_others_or_misnamed_is_refused() {
        let store = KeyStore::at(scratch("refuse")).expect("opens");
        let path = store
            .save_key(7, &AppKey::from_secret(&[0x11; 32]))
            .expect("saves");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("chmod");
        assert!(matches!(
            store.load(None),
            Err(KeyStoreError::KeyTooOpen { mode: 0o644, .. })
        ));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("chmod");
        fs::write(&path, format!("{}\n", hex::encode([0x22; 32]))).expect("writes");
        assert!(matches!(
            store.load(None),
            Err(KeyStoreError::BadKeyFile { .. })
        ));
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
