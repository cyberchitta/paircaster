//! Signed messages saved before they are sent, under
//! `$XDG_CONFIG_HOME/selfcast/outbox`, else `~/.config/selfcast/outbox`: one
//! `<hash hex>.msg` file each, mode 0600, in a 0700 directory. A message
//! leaves once the node is seen to hold it. One left behind by a failed send
//! is sent again as the same bytes by `selfcast resend`; signing again would
//! make a second message with a new hash.

use std::path::{Path, PathBuf};

use selfcast_core::build::{DecodeError, SignedMessage};

use crate::keystore::{self, KeyStoreError};

const MESSAGE: &str = "msg";

#[derive(Debug, thiserror::Error)]
pub enum OutboxError {
    #[error(transparent)]
    Store(#[from] KeyStoreError),
    #[error("{path} is not a saved message: {source}")]
    Unreadable { path: PathBuf, source: DecodeError },
}

pub struct Outbox {
    dir: PathBuf,
}

impl Outbox {
    pub fn open() -> Result<Self, OutboxError> {
        Self::at(keystore::config_dir()?.join("selfcast").join("outbox"))
    }

    fn at(dir: PathBuf) -> Result<Self, OutboxError> {
        keystore::private_dir(&dir)?;
        Ok(Self { dir })
    }

    pub fn save(&self, message: &SignedMessage) -> Result<PathBuf, OutboxError> {
        let name = format!("{}.{MESSAGE}", hex::encode(message.hash().as_bytes()));
        let path = self.dir.join(name);
        keystore::write_new(&path, &message.encode())?;
        Ok(path)
    }

    /// Saved messages not yet seen on the node, oldest name first.
    pub fn unsent(&self) -> Result<Vec<PathBuf>, OutboxError> {
        Ok(keystore::files_with_extension(&self.dir, MESSAGE)?)
    }
}

pub fn read(path: &Path) -> Result<SignedMessage, OutboxError> {
    SignedMessage::decode(&keystore::read(path)?).map_err(|source| OutboxError::Unreadable {
        path: path.to_path_buf(),
        source,
    })
}

pub fn remove(path: &Path) -> Result<(), OutboxError> {
    Ok(keystore::remove(path)?)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use selfcast_core::build::{Action, AppKey, Envelope, sign};
    use selfcast_core::proto::FarcasterNetwork;
    use selfcast_core::time::Timestamp;

    use super::{Outbox, read, remove};

    #[test]
    fn a_saved_message_reads_back_as_the_same_bytes_until_removed() {
        let dir = std::env::temp_dir().join(format!("selfcast-outbox-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let outbox = Outbox::at(dir.clone()).expect("opens");
        let envelope = Envelope {
            fid: 7,
            network: FarcasterNetwork::Mainnet,
            timestamp: Timestamp::from_unix(1_792_281_600),
        };
        let key = AppKey::from_secret(&[0x11; 32]);
        let signed = sign(&key, &envelope, &Action::FollowAdd(3)).expect("signs");

        let path = outbox.save(&signed).expect("saves");

        let mode = fs::metadata(&path).expect("exists").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(outbox.unsent().expect("lists"), std::slice::from_ref(&path));
        assert_eq!(read(&path).expect("reads"), signed);
        assert!(outbox.save(&signed).is_err(), "never overwritten");
        remove(&path).expect("removes");
        assert!(outbox.unsent().expect("lists").is_empty());
    }
}
