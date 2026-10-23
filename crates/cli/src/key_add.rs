//! `paircaster key add`: registers a new app key for the account whose
//! recovery phrase is typed at the terminal, with a gasless `KEY_ADD`.
//!
//! The phrase is read only from the terminal, turned into the custody key,
//! and dropped. The custody key signs once and is dropped. What the user
//! confirms is rendered here from the exact terms the custody key is about
//! to sign. The new key and the signed message are saved before sending; a
//! rerun resends that message until the node lists the key, or until its
//! deadline passes.

use std::path::{Path, PathBuf};
use std::time::Duration;

use paircaster_core::Node;
use paircaster_core::build::{
    Action, AppKey, BuildError, DecodeError, Envelope, SignedMessage, sign,
};
use paircaster_core::cast::SignerKey;
use paircaster_core::custody::{CustodyError, CustodyKey, EthAddress};
use paircaster_core::key_add::{KeyAddError, KeyRequest, MAX_TTL_SECS, authorize};
use paircaster_core::profile::{Followers, Profile};
use paircaster_core::proto::{Body, FarcasterNetwork, MessageData, MessageType};
use paircaster_core::time::Timestamp;
use prost::Message as _;
use serde::Serialize;
use zeroize::Zeroizing;

use crate::CliError;
use crate::clock;
use crate::invocation;
use crate::keystore::{self, KeyStore, KeyStoreError};
use crate::tty::{Tty, TtyError};

/// Only what Paircaster sends: casts, reactions and follows, each with its
/// undo. Not profile data, verifications or storage.
const SCOPES: [MessageType; 6] = [
    MessageType::CastAdd,
    MessageType::CastRemove,
    MessageType::ReactionAdd,
    MessageType::ReactionRemove,
    MessageType::LinkAdd,
    MessageType::LinkRemove,
];
/// How long the custody signatures stay valid if sending is delayed.
const DEADLINE: Duration = Duration::from_secs(3600);
/// How long to wait for the node to list the new key.
const CONFIRM_TRIES: u32 = 30;
const CONFIRM_EVERY: Duration = Duration::from_secs(2);

#[derive(Debug, thiserror::Error)]
pub enum KeyAddCliError {
    #[error(transparent)]
    Tty(#[from] TtyError),
    #[error(transparent)]
    Store(#[from] KeyStoreError),
    #[error(transparent)]
    Custody(#[from] CustodyError),
    #[error(transparent)]
    Terms(#[from] KeyAddError),
    #[error(transparent)]
    Build(#[from] BuildError),
    #[error("no Farcaster account has custody address {0}; check the recovery phrase")]
    NoAccount(EthAddress),
    #[error("not confirmed; nothing was signed or sent")]
    Declined,
    #[error("the account's key nonce is at its maximum")]
    NonceExhausted,
    #[error("could not get random bytes for the new key: {0}")]
    Random(String),
    #[error("{path} is not a saved KEY_ADD: {reason}")]
    BadPending { path: PathBuf, reason: String },
    #[error(
        "sent, but the node did not list the key within a minute. Run `{paircaster} key add` again \
         to check and resend; the key is saved at {}",
        .key_file.display()
    )]
    NotConfirmed {
        key_file: PathBuf,
        paircaster: String,
    },
}

/// `paircaster key add --json`, version 1:
///
/// ```json
/// { "version": 1, "status": "added", "fid": 1234,
///   "key": "0x…", "key_file": "/home/…/.config/paircaster/keys/1234-….key" }
/// ```
///
/// `status` is `added`, or `already_added` when a rerun finds the key from
/// an earlier run already listed.
#[derive(Debug, Serialize)]
struct KeyAddV1 {
    version: u32,
    status: Status,
    fid: u64,
    key: String,
    key_file: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Added,
    AlreadyAdded,
}

struct Outcome {
    status: Status,
    fid: u64,
    key: SignerKey,
    key_file: PathBuf,
}

pub async fn run(node: &Node, json: bool) -> Result<(), CliError> {
    let store = KeyStore::open().map_err(KeyAddCliError::from)?;
    let resumed = resume(node, &store).await?;
    let outcomes = match resumed.is_empty() {
        true => vec![add(node, &store).await?],
        false => resumed,
    };
    outcomes.iter().for_each(|o| report(o, json));
    Ok(())
}

async fn add(node: &Node, store: &KeyStore) -> Result<Outcome, CliError> {
    let mut tty = Tty::open().map_err(KeyAddCliError::from)?;
    let custody = custody_key(&mut tty)?;
    let address = custody.address();
    let fid = node
        .fid_of_custody(&address)
        .await?
        .ok_or(KeyAddCliError::NoAccount(address))?;
    let username = Profile::assemble(
        fid,
        &node.user_data_by_fid(fid).await?,
        0,
        Followers::Exact(0),
    )
    .username;
    let nonce = node
        .user_nonce(fid)
        .await?
        .checked_add(1)
        .ok_or(KeyAddCliError::NonceExhausted)?;
    let app = new_app_key()?;
    let now = clock::now();
    let request = KeyRequest {
        fid,
        key: app.public(),
        scopes: SCOPES.to_vec(),
        ttl_secs: MAX_TTL_SECS,
        nonce,
        deadline: Timestamp::from_unix(now.unix_seconds() + DEADLINE.as_secs()),
    };
    let key_file = store.key_path(fid, &request.key);
    tty.say(&confirmation(
        &request,
        &address,
        username.as_deref(),
        &key_file,
    ))
    .map_err(KeyAddCliError::from)?;
    if tty
        .ask("Type yes to sign and send: ")
        .map_err(KeyAddCliError::from)?
        .trim()
        != "yes"
    {
        return Err(KeyAddCliError::Declined.into());
    }
    let body = authorize(&custody, &request).map_err(KeyAddCliError::from)?;
    drop(custody);
    let envelope = Envelope {
        fid,
        network: FarcasterNetwork::Mainnet,
        timestamp: now,
    };
    let signed = sign(&app, &envelope, &Action::KeyAdd(body)).map_err(KeyAddCliError::from)?;
    let key_file = store.save_key(fid, &app).map_err(KeyAddCliError::from)?;
    let pending = store
        .save_pending(fid, &request.key, &signed.encode())
        .map_err(KeyAddCliError::from)?;
    send_and_confirm(node, &signed, fid, &pending, &key_file).await?;
    Ok(Outcome {
        status: Status::Added,
        fid,
        key: request.key,
        key_file,
    })
}

fn custody_key(tty: &mut Tty) -> Result<CustodyKey, KeyAddCliError> {
    tty.say(
        "Paircaster needs your recovery phrase once, to authorise a new app key.\n\
         It is not stored, and the app key it authorises cannot move funds or change custody.\n",
    )?;
    let phrase = tty.ask_secret("Recovery phrase (hidden): ")?;
    Ok(CustodyKey::from_phrase(&phrase)?)
}

fn new_app_key() -> Result<AppKey, KeyAddCliError> {
    let mut secret = Zeroizing::new([0u8; 32]);
    getrandom::fill(&mut *secret).map_err(|e| KeyAddCliError::Random(e.to_string()))?;
    Ok(AppKey::from_secret(&secret))
}

/// What the custody key will sign, from the request itself.
fn confirmation(
    request: &KeyRequest,
    custody: &EthAddress,
    username: Option<&str>,
    key_file: &Path,
) -> String {
    let who = username.map_or(String::new(), |u| format!(" @{u}"));
    let scopes: Vec<&str> = request.scopes.iter().map(|s| scope_name(*s)).collect();
    format!(
        "\nAdd an app key to account{who} (FID {fid}), custody address {custody}.\n\n\
         \x20 New key:     0x{key}\n\
         \x20 May sign:    {scopes}\n\
         \x20 Expires:     {days} days after it was last used\n\
         \x20 Nonce:       {nonce}\n\
         \x20 Sign by:     {deadline} (the authorisation lapses after this)\n\
         \x20 Saved to:    {path}\n\n\
         It cannot change the account, move funds, or add or remove other keys.\n",
        fid = request.fid,
        key = hex::encode(request.key.as_bytes()),
        scopes = scopes.join(", "),
        days = request.ttl_secs / 86_400,
        nonce = request.nonce,
        deadline = request.deadline,
        path = key_file.display(),
    )
}

fn scope_name(scope: MessageType) -> &'static str {
    match scope {
        MessageType::CastAdd => "casts",
        MessageType::CastRemove => "deleting casts",
        MessageType::ReactionAdd => "likes and recasts",
        MessageType::ReactionRemove => "undoing them",
        MessageType::LinkAdd => "follows",
        MessageType::LinkRemove => "unfollows",
        MessageType::UserDataAdd => "profile changes",
        MessageType::KeyAdd => "adding keys",
        MessageType::None => "nothing",
    }
}

/// Sends `signed`, then waits for the node to list its key. Once listed,
/// the saved message at `pending` is no longer needed.
async fn send_and_confirm(
    node: &Node,
    signed: &SignedMessage,
    fid: u64,
    pending: &Path,
    key_file: &Path,
) -> Result<(), CliError> {
    node.submit_message(signed).await?;
    for _ in 0..CONFIRM_TRIES {
        if is_listed(node, fid, &signed.signer()).await? {
            keystore::remove(pending).map_err(KeyAddCliError::from)?;
            return Ok(());
        }
        tokio::time::sleep(CONFIRM_EVERY).await;
    }
    Err(KeyAddCliError::NotConfirmed {
        key_file: key_file.to_path_buf(),
        paircaster: invocation::paircaster(),
    }
    .into())
}

async fn is_listed(node: &Node, fid: u64, key: &SignerKey) -> Result<bool, CliError> {
    Ok(node
        .signers_by_fid(fid)
        .await?
        .iter()
        .any(|s| s.key == *key))
}

/// Finishes `KEY_ADD`s saved by earlier runs: each is confirmed if already
/// listed, resent if still within its deadline, or discarded with its
/// never-registered key once the deadline has passed.
async fn resume(node: &Node, store: &KeyStore) -> Result<Vec<Outcome>, CliError> {
    let mut outcomes = Vec::new();
    for pending in store.pending().map_err(KeyAddCliError::from)? {
        let saved = Saved::read(&pending)?;
        let key_file = store.key_path(saved.fid, &saved.message.signer());
        if is_listed(node, saved.fid, &saved.message.signer()).await? {
            keystore::remove(&pending).map_err(KeyAddCliError::from)?;
            outcomes.push(saved.outcome(Status::AlreadyAdded, key_file));
        } else if saved.deadline > clock::now() {
            send_and_confirm(node, &saved.message, saved.fid, &pending, &key_file).await?;
            outcomes.push(saved.outcome(Status::Added, key_file));
        } else {
            eprintln!(
                "discarding an unsent key from an earlier run: its authorisation lapsed at {}",
                saved.deadline
            );
            keystore::remove(&pending).map_err(KeyAddCliError::from)?;
            keystore::remove(&key_file).map_err(KeyAddCliError::from)?;
        }
    }
    Ok(outcomes)
}

struct Saved {
    message: SignedMessage,
    fid: u64,
    deadline: Timestamp,
}

impl Saved {
    fn read(path: &Path) -> Result<Self, KeyAddCliError> {
        let bad = |reason: String| KeyAddCliError::BadPending {
            path: path.to_path_buf(),
            reason,
        };
        let message = SignedMessage::decode(&keystore::read(path)?)
            .map_err(|e: DecodeError| bad(e.to_string()))?;
        let data = MessageData::decode(message.data_bytes()).map_err(|e| bad(e.to_string()))?;
        match data.body {
            Some(Body::KeyAdd(body)) => Ok(Self {
                fid: data.fid,
                deadline: Timestamp::from_farcaster(body.deadline),
                message,
            }),
            _ => Err(bad("not a KEY_ADD".to_owned())),
        }
    }

    fn outcome(&self, status: Status, key_file: PathBuf) -> Outcome {
        Outcome {
            status,
            fid: self.fid,
            key: self.message.signer(),
            key_file,
        }
    }
}

fn report(outcome: &Outcome, json: bool) {
    let key = format!("0x{}", hex::encode(outcome.key.as_bytes()));
    let key_file = outcome.key_file.display().to_string();
    let out = match json {
        true => serde_json::to_string(&KeyAddV1 {
            version: 1,
            status: outcome.status,
            fid: outcome.fid,
            key,
            key_file,
        })
        .expect("a report always serialises"),
        false => {
            let verb = match outcome.status {
                Status::Added => "Added",
                Status::AlreadyAdded => "Already added",
            };
            format!(
                "{verb}: app key {key} for FID {}.\nSaved at {key_file}.",
                outcome.fid
            )
        }
    };
    crate::out::line(format_args!("{out}"));
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use paircaster_core::build::{Action, AppKey, Envelope, sign};
    use paircaster_core::custody::CustodyKey;
    use paircaster_core::key_add::{KeyRequest, MAX_TTL_SECS, authorize};
    use paircaster_core::proto::FarcasterNetwork;
    use paircaster_core::time::Timestamp;

    use super::{SCOPES, Saved, confirmation};

    /// Hardhat's published test phrase. A mainnet account uses its address
    /// (FID 1607387), so nothing signed with it is ever sent.
    const PHRASE: &str = "test test test test test test test test test test test junk";

    fn request() -> KeyRequest {
        KeyRequest {
            fid: 1234,
            key: AppKey::from_secret(&[0x11; 32]).public(),
            scopes: SCOPES.to_vec(),
            ttl_secs: MAX_TTL_SECS,
            nonce: 3,
            deadline: Timestamp::from_unix(1_792_285_200),
        }
    }

    #[test]
    fn the_confirmation_shows_every_signed_term() {
        let custody = CustodyKey::from_phrase(PHRASE).expect("phrase");
        let text = confirmation(
            &request(),
            &custody.address(),
            Some("alice"),
            Path::new("/k"),
        );
        for part in [
            "@alice (FID 1234)",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "casts, deleting casts, likes and recasts, undoing them, follows, unfollows",
            "90 days",
            "Nonce:       3",
            "2026-10-18T01:00:00Z",
            "/k",
        ] {
            assert!(text.contains(part), "missing {part:?} in:\n{text}");
        }
        assert!(text.contains(&hex::encode(request().key.as_bytes())));
    }

    #[test]
    fn a_saved_key_add_reads_back_with_its_account_and_deadline() {
        let custody = CustodyKey::from_phrase(PHRASE).expect("phrase");
        let app = AppKey::from_secret(&[0x11; 32]);
        let body = authorize(&custody, &request()).expect("authorizes");
        let envelope = Envelope {
            fid: 1234,
            network: FarcasterNetwork::Mainnet,
            timestamp: Timestamp::from_unix(1_792_281_600),
        };
        let signed = sign(&app, &envelope, &Action::KeyAdd(body)).expect("signs");
        let path = std::env::temp_dir().join(format!("paircaster-saved-{}", std::process::id()));
        std::fs::write(&path, signed.encode()).expect("writes");
        let saved = Saved::read(&path).expect("reads back");
        assert_eq!(saved.fid, 1234);
        assert_eq!(saved.deadline, Timestamp::from_unix(1_792_285_200));
        assert_eq!(saved.message, signed);
        std::fs::write(&path, b"not a message").expect("writes");
        assert!(Saved::read(&path).is_err());
    }
}
