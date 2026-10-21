//! The write commands: `cast`, `delete`, `react`, `unreact`, `follow`,
//! `unfollow`, and `resend`.
//!
//! Each signs one message with the account's saved app key. What the user
//! confirms is rendered from the signed bytes decoded back, not from the
//! command line. The message is saved to the outbox before it is sent, and
//! the write counts as done only once the node is seen to hold it.
//!
//! The confirmation is typed at the terminal. `--yes` skips it, for an agent
//! whose harness asks the user before each write command; the harness's
//! prompt, showing the command, is then the only confirmation. `--dry-run`
//! signs and asks the node's `validateMessage`, and sends nothing.

use std::path::{Path, PathBuf};
use std::time::Duration;

use prost::Message as _;
use selfcast_core::Node;
use selfcast_core::build::{
    Action, BuildError, CAST_MAX_BYTES, Envelope, LONG_CAST_MAX_BYTES, NewCast, SignedMessage,
    cast_type_for, sign,
};
use selfcast_core::cast::{CastRef, MessageHash, ParseRefError, Target};
use selfcast_core::profile::{Followers, Profile};
use selfcast_core::proto::{
    Body, CastParent, CastType, Embed, EmbedTarget, FarcasterNetwork, LinkTarget, MessageData,
    MessageType, ReactionTarget, ReactionType,
};
use selfcast_core::time::Timestamp;
use serde::Serialize;

use crate::CliError;
use crate::clock;
use crate::invocation;
use crate::keystore::{KeyStore, KeyStoreError, LoadedKey};
use crate::outbox::{self, Outbox, OutboxError};
use crate::tty::{Tty, TtyError};

/// How long to wait for the node to show a sent message.
const CONFIRM_TRIES: u32 = 30;
const CONFIRM_EVERY: Duration = Duration::from_secs(2);

/// What every write command takes.
#[derive(Debug, clap::Args)]
pub struct WriteFlags {
    /// The account to write as, when keys for several are saved.
    #[arg(long)]
    fid: Option<u64>,
    /// Send without asking at the terminal. For an agent whose harness asks
    /// the user before each write command.
    #[arg(long)]
    yes: bool,
    /// Sign, show, and ask the node whether it would accept the message.
    /// Nothing is saved or sent.
    #[arg(long, conflicts_with = "yes")]
    dry_run: bool,
}

#[derive(Debug, clap::Args)]
pub struct CastArgs {
    /// The text, exactly as it will be posted. `@name` is not turned into a
    /// mention. Over 320 bytes it is sent as a long cast, up to 1024.
    #[arg(value_parser = cast_text)]
    text: CastText,
    /// The cast this replies to, as FID:HASH.
    #[arg(long, value_name = "FID:HASH")]
    reply_to: Option<CastRef>,
    /// A URL, or a cast as FID:HASH, to embed. Repeatable.
    #[arg(long = "embed", value_name = "URL|FID:HASH", value_parser = embed_arg)]
    embeds: Vec<Target>,
    #[command(flatten)]
    flags: WriteFlags,
}

#[derive(Debug, clap::Args)]
pub struct DeleteArgs {
    /// The hash of one of the account's own casts.
    hash: MessageHash,
    #[command(flatten)]
    flags: WriteFlags,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Kind {
    Like,
    Recast,
}

#[derive(Debug, clap::Args)]
pub struct ReactArgs {
    kind: Kind,
    /// The cast, as FID:HASH.
    #[arg(value_name = "FID:HASH")]
    cast: CastRef,
    #[command(flatten)]
    flags: WriteFlags,
}

#[derive(Debug, clap::Args)]
pub struct FollowArgs {
    /// The account to follow or unfollow.
    #[arg(value_name = "FID")]
    account: u64,
    #[command(flatten)]
    flags: WriteFlags,
}

#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error(transparent)]
    Store(#[from] KeyStoreError),
    #[error(transparent)]
    Outbox(#[from] OutboxError),
    #[error(transparent)]
    Tty(#[from] TtyError),
    #[error(transparent)]
    Build(#[from] BuildError),
    #[error("{0} is not a message Selfcast writes")]
    Unknown(String),
    #[error(
        "the app key in {} is not registered to FID {fid} on this node; \
         finish `{selfcast} key add`, or check --node",
        .path.display()
    )]
    NotRegistered {
        fid: u64,
        path: PathBuf,
        selfcast: String,
    },
    #[error(
        "{count} message(s) from an earlier write are saved but not seen sent; \
         run `{selfcast} resend` first"
    )]
    Unsent { count: usize, selfcast: String },
    #[error("not confirmed; nothing was sent")]
    Declined,
    #[error("the node says it would not accept this message; nothing was sent")]
    Invalid,
    #[error(
        "sent, but the node did not show it within a minute. It is saved at {}; \
         run `{selfcast} resend` to check and send it again",
        .path.display()
    )]
    NotConfirmed { path: PathBuf, selfcast: String },
}

/// `--json` output of a write command, version 1:
///
/// ```json
/// { "version": 1, "status": "sent", "kind": "cast", "fid": 1234, "hash": "0x…" }
/// ```
///
/// `status` is `sent` (the node shows it), or with `--dry-run` `valid` or
/// `invalid` (nothing sent). `kind` is one of `cast`, `delete`, `like`,
/// `recast`, `unlike`, `unrecast`, `follow`, `unfollow`. `resend` prints
/// `{ "version": 1, "messages": [ … ] }`, one entry per message it sent.
#[derive(Debug, Serialize)]
struct WriteV1 {
    version: u32,
    status: Status,
    kind: &'static str,
    fid: u64,
    hash: String,
}

#[derive(Debug, Serialize)]
struct ResendV1 {
    version: u32,
    messages: Vec<WriteV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Sent,
    Valid,
    Invalid,
}

/// A decoded message, in the terms Selfcast writes.
#[derive(Debug, Clone, PartialEq, Eq)]
enum What {
    Cast {
        text: String,
        parent: Option<Target>,
        embeds: Vec<Target>,
    },
    Delete(MessageHash),
    React {
        undo: bool,
        kind: ReactionType,
        cast: CastRef,
    },
    Follow {
        undo: bool,
        account: u64,
    },
}

impl CastArgs {
    pub fn into_parts(self) -> (Action, WriteFlags) {
        let cast = NewCast {
            text: self.text.text,
            mentions: Vec::new(),
            embeds: self.embeds,
            parent: self.reply_to.map(Target::Cast),
            kind: self.text.kind,
        };
        (Action::CastAdd(cast), self.flags)
    }
}

impl DeleteArgs {
    pub fn into_parts(self) -> (Action, WriteFlags) {
        (Action::CastRemove(self.hash), self.flags)
    }
}

impl ReactArgs {
    pub fn into_parts(self, undo: bool) -> (Action, WriteFlags) {
        let kind = match self.kind {
            Kind::Like => ReactionType::Like,
            Kind::Recast => ReactionType::Recast,
        };
        let target = Target::Cast(self.cast);
        let action = match undo {
            false => Action::ReactionAdd(kind, target),
            true => Action::ReactionRemove(kind, target),
        };
        (action, self.flags)
    }
}

impl FollowArgs {
    pub fn into_parts(self, undo: bool) -> (Action, WriteFlags) {
        let action = match undo {
            false => Action::FollowAdd(self.account),
            true => Action::FollowRemove(self.account),
        };
        (action, self.flags)
    }
}

/// Cast text within a node's limits, with the cast type its length needs.
#[derive(Debug, Clone)]
struct CastText {
    text: String,
    kind: CastType,
}

fn cast_text(s: &str) -> Result<CastText, String> {
    cast_type_for(s)
        .map(|kind| CastText {
            text: s.to_owned(),
            kind,
        })
        .ok_or_else(|| {
            format!(
                "{} bytes; a cast holds at most {LONG_CAST_MAX_BYTES} (over {CAST_MAX_BYTES} is sent as a long cast)",
                s.len()
            )
        })
}

fn embed_arg(s: &str) -> Result<Target, ParseRefError> {
    match s.contains("://") {
        true => Ok(Target::Url(s.to_owned())),
        false => s.parse().map(Target::Cast),
    }
}

/// Signs `action` as the saved account, confirms it, sends it, and waits to
/// see it on the node.
pub async fn send(
    node: &Node,
    action: Action,
    flags: WriteFlags,
    json: bool,
) -> Result<(), CliError> {
    let loaded = KeyStore::open()
        .and_then(|s| s.load(flags.fid))
        .map_err(WriteError::from)?;
    if flags.dry_run {
        let (signed, data, what, shown) = prepare(node, &loaded, &action).await?;
        return dry_run(node, &signed, &what, data.fid, &shown, json).await;
    }
    let outbox = Outbox::open().map_err(WriteError::from)?;
    refuse_if_unsent(&outbox)?;
    ensure_registered(node, &loaded).await?;
    let (signed, data, what, shown) = prepare(node, &loaded, &action).await?;
    confirm(&shown, flags.yes)?;
    let saved = outbox.save(&signed).map_err(WriteError::from)?;
    deliver(node, &signed, &data, &what, &saved).await?;
    report(Status::Sent, &what, data.fid, signed.hash(), json);
    Ok(())
}

/// Signs `action` now, and renders what it does from the signed bytes.
async fn prepare(
    node: &Node,
    loaded: &LoadedKey,
    action: &Action,
) -> Result<(SignedMessage, MessageData, What, String), CliError> {
    let envelope = Envelope {
        fid: loaded.fid,
        network: FarcasterNetwork::Mainnet,
        timestamp: clock::now(),
    };
    let signed = sign(&loaded.key, &envelope, action).map_err(WriteError::from)?;
    let data = decode(&signed)?;
    let what = what(&data)?;
    let shown = confirmation(&data, &what, &signed, username(node, loaded.fid).await?);
    Ok((signed, data, what, shown))
}

/// Sends again every message the outbox still holds, as saved.
pub async fn resend(node: &Node, json: bool) -> Result<(), CliError> {
    let outbox = Outbox::open().map_err(WriteError::from)?;
    let mut sent = Vec::new();
    for path in outbox.unsent().map_err(WriteError::from)? {
        let signed = outbox::read(&path).map_err(WriteError::from)?;
        let data = decode(&signed)?;
        let what = what(&data)?;
        eprintln!(
            "Resending {} as FID {}: {}",
            signed.hash(),
            data.fid,
            summary(&what)
        );
        deliver(node, &signed, &data, &what, &path).await?;
        sent.push(entry(Status::Sent, &what, data.fid, signed.hash()));
    }
    match json {
        true => println!(
            "{}",
            serde_json::to_string(&ResendV1 {
                version: 1,
                messages: sent,
            })
            .expect("a report always serialises")
        ),
        false if sent.is_empty() => println!("Nothing to resend."),
        false => println!("Resent {} message(s); the node shows each.", sent.len()),
    }
    Ok(())
}

fn refuse_if_unsent(outbox: &Outbox) -> Result<(), WriteError> {
    match outbox.unsent()?.len() {
        0 => Ok(()),
        count => Err(WriteError::Unsent {
            count,
            selfcast: invocation::selfcast(),
        }),
    }
}

async fn ensure_registered(node: &Node, loaded: &LoadedKey) -> Result<(), CliError> {
    let public = loaded.key.public();
    let listed = node
        .signers_by_fid(loaded.fid)
        .await?
        .iter()
        .any(|s| s.key == public);
    match listed {
        true => Ok(()),
        false => Err(WriteError::NotRegistered {
            fid: loaded.fid,
            path: loaded.path.clone(),
            selfcast: invocation::selfcast(),
        }
        .into()),
    }
}

async fn username(node: &Node, fid: u64) -> Result<Option<String>, CliError> {
    Ok(Profile::assemble(
        fid,
        &node.user_data_by_fid(fid).await?,
        0,
        Followers::Exact(0),
    )
    .username)
}

fn decode(signed: &SignedMessage) -> Result<MessageData, WriteError> {
    MessageData::decode(signed.data_bytes()).map_err(|e| WriteError::Unknown(e.to_string()))
}

/// What `data` does, read from the message itself.
fn what(data: &MessageData) -> Result<What, WriteError> {
    let undo = [MessageType::ReactionRemove, MessageType::LinkRemove]
        .map(i32::from)
        .contains(&data.r#type);
    match data.body.as_ref() {
        Some(Body::CastAdd(add)) => Ok(What::Cast {
            text: add.text.clone(),
            parent: add.parent.as_ref().map(parent_target).transpose()?,
            embeds: add
                .embeds
                .iter()
                .map(embed_target)
                .collect::<Result<_, _>>()?,
        }),
        Some(Body::CastRemove(remove)) => MessageHash::from_bytes(&remove.target_hash)
            .map(What::Delete)
            .ok_or_else(|| unknown("a delete with a bad hash")),
        Some(Body::Reaction(r)) => Ok(What::React {
            undo,
            kind: ReactionType::try_from(r.r#type).map_err(|_| unknown("a reaction"))?,
            cast: match &r.target {
                Some(ReactionTarget::CastId(id)) => cast_ref(id.fid, &id.hash)?,
                _ => return Err(unknown("a reaction to a URL")),
            },
        }),
        Some(Body::Link(link)) => match (&link.target, link.r#type.as_str()) {
            (Some(LinkTarget::Fid(account)), "follow") => Ok(What::Follow {
                undo,
                account: *account,
            }),
            _ => Err(unknown("a link that is not a follow")),
        },
        _ => Err(unknown("this message type")),
    }
}

fn unknown(what: &str) -> WriteError {
    WriteError::Unknown(what.to_owned())
}

fn parent_target(parent: &CastParent) -> Result<Target, WriteError> {
    match parent {
        CastParent::CastId(id) => cast_ref(id.fid, &id.hash).map(Target::Cast),
        CastParent::Url(url) => Ok(Target::Url(url.clone())),
    }
}

fn embed_target(embed: &Embed) -> Result<Target, WriteError> {
    match &embed.embed {
        Some(EmbedTarget::CastId(id)) => cast_ref(id.fid, &id.hash).map(Target::Cast),
        Some(EmbedTarget::Url(url)) => Ok(Target::Url(url.clone())),
        None => Err(unknown("an empty embed")),
    }
}

fn cast_ref(fid: u64, hash: &[u8]) -> Result<CastRef, WriteError> {
    let hash = MessageHash::from_bytes(hash).ok_or_else(|| unknown("a cast with a bad hash"))?;
    Ok(CastRef { fid, hash })
}

/// What the user is asked to approve, from the decoded message.
fn confirmation(
    data: &MessageData,
    what: &What,
    signed: &SignedMessage,
    username: Option<String>,
) -> String {
    let who = username.map_or(String::new(), |u| format!("@{u} "));
    format!(
        "\nSend as {who}(FID {fid}), signed by app key {key}, dated {when}:\n\n{body}\n",
        fid = data.fid,
        key = signed.signer(),
        when = Timestamp::from_farcaster(data.timestamp),
        body = body_lines(what),
    )
}

fn body_lines(what: &What) -> String {
    match what {
        What::Cast {
            text,
            parent,
            embeds,
        } => {
            let quoted = text
                .split('\n')
                .map(|line| format!("    | {}", visible(line)))
                .collect::<Vec<_>>()
                .join("\n");
            let reply = parent.iter().map(|p| format!("  Reply to: {}", target(p)));
            let embeds = embeds.iter().map(|e| format!("  Embed:    {}", target(e)));
            std::iter::once(format!("  Cast:\n{quoted}"))
                .chain(reply)
                .chain(embeds)
                .collect::<Vec<_>>()
                .join("\n")
        }
        other => format!("  {}", summary(other)),
    }
}

/// `line` with every control character escaped, so text cannot move the
/// cursor or recolour the terminal to hide what it says.
fn visible(line: &str) -> String {
    line.chars()
        .map(|c| match c.is_control() {
            true => c.escape_default().to_string(),
            false => c.to_string(),
        })
        .collect()
}

fn target(t: &Target) -> String {
    match t {
        Target::Cast(r) => format!("cast {r}"),
        Target::Url(url) => visible(url),
    }
}

/// One line saying what a message does.
fn summary(what: &What) -> String {
    match what {
        What::Cast { text, .. } => format!("cast {:?}", text),
        What::Delete(hash) => format!("Delete your cast {hash}"),
        What::React { undo, kind, cast } => {
            let verb = match (undo, kind) {
                (false, ReactionType::Recast) => "Recast",
                (true, ReactionType::Recast) => "Undo the recast of",
                (false, _) => "Like",
                (true, _) => "Undo the like of",
            };
            format!("{verb} cast {cast}")
        }
        What::Follow { undo, account } => match undo {
            false => format!("Follow FID {account}"),
            true => format!("Unfollow FID {account}"),
        },
    }
}

fn kind_name(what: &What) -> &'static str {
    match what {
        What::Cast { .. } => "cast",
        What::Delete(_) => "delete",
        What::React {
            undo: false,
            kind: ReactionType::Recast,
            ..
        } => "recast",
        What::React {
            undo: true,
            kind: ReactionType::Recast,
            ..
        } => "unrecast",
        What::React { undo: false, .. } => "like",
        What::React { undo: true, .. } => "unlike",
        What::Follow { undo: false, .. } => "follow",
        What::Follow { undo: true, .. } => "unfollow",
    }
}

fn confirm(shown: &str, yes: bool) -> Result<(), WriteError> {
    if yes {
        eprint!("{shown}");
        return Ok(());
    }
    let mut tty = Tty::open()?;
    tty.say(shown)?;
    match tty.ask("Type yes to send: ")?.trim() {
        "yes" => Ok(()),
        _ => Err(WriteError::Declined),
    }
}

async fn dry_run(
    node: &Node,
    signed: &SignedMessage,
    what: &What,
    fid: u64,
    shown: &str,
    json: bool,
) -> Result<(), CliError> {
    eprint!("{shown}");
    let status = match node.validate_message(signed).await? {
        true => Status::Valid,
        false => Status::Invalid,
    };
    report(status, what, fid, signed.hash(), json);
    match status {
        Status::Invalid => Err(WriteError::Invalid.into()),
        _ => Ok(()),
    }
}

/// Sends `signed` and waits until the node shows it; then removes it from
/// the outbox.
async fn deliver(
    node: &Node,
    signed: &SignedMessage,
    data: &MessageData,
    what: &What,
    saved: &Path,
) -> Result<(), CliError> {
    node.submit_message(signed).await?;
    for _ in 0..CONFIRM_TRIES {
        if landed(node, data.fid, what, signed.hash()).await? {
            outbox::remove(saved).map_err(WriteError::from)?;
            return Ok(());
        }
        tokio::time::sleep(CONFIRM_EVERY).await;
    }
    Err(WriteError::NotConfirmed {
        path: saved.to_path_buf(),
        selfcast: invocation::selfcast(),
    }
    .into())
}

/// Whether the node shows the message with hash `sent` in effect. An add
/// has landed when the node holds that very message. An undo has landed
/// when the node holds no add there, or holds the undo itself.
async fn landed(node: &Node, fid: u64, what: &What, sent: MessageHash) -> Result<bool, CliError> {
    let (held, undo) = match what {
        What::Cast { .. } => (node.cast_hash(&CastRef { fid, hash: sent }).await?, false),
        What::Delete(hash) => (node.cast_hash(&CastRef { fid, hash: *hash }).await?, true),
        What::React { undo, kind, cast } => (node.reaction_hash(fid, *kind, cast).await?, *undo),
        What::Follow { undo, account } => (node.follow_hash(fid, *account).await?, *undo),
    };
    Ok(shows(held, undo, sent))
}

fn shows(held: Option<MessageHash>, undo: bool, sent: MessageHash) -> bool {
    held == Some(sent) || (undo && held.is_none())
}

fn entry(status: Status, what: &What, fid: u64, hash: MessageHash) -> WriteV1 {
    WriteV1 {
        version: 1,
        status,
        kind: kind_name(what),
        fid,
        hash: hash.to_string(),
    }
}

fn report(status: Status, what: &What, fid: u64, hash: MessageHash, json: bool) {
    let out = match json {
        true => serde_json::to_string(&entry(status, what, fid, hash))
            .expect("a report always serialises"),
        false => match status {
            Status::Sent => format!("Sent and seen on the node: {} {hash}", kind_name(what)),
            Status::Valid => format!("Dry run: the node would accept {hash}. Nothing was sent."),
            Status::Invalid => format!("Dry run: the node would reject {hash}. Nothing was sent."),
        },
    };
    println!("{out}");
}

#[cfg(test)]
mod tests {
    use selfcast_core::build::{Action, AppKey, Envelope, NewCast, SignedMessage, sign};
    use selfcast_core::cast::{CastRef, MessageHash, Target};
    use selfcast_core::proto::{CastType, FarcasterNetwork, MessageData, ReactionType};
    use selfcast_core::time::Timestamp;

    use super::{What, cast_text, confirmation, decode, embed_arg, kind_name, shows, what};

    fn hash(byte: u8) -> MessageHash {
        MessageHash::from_bytes(&[byte; 20]).expect("20 bytes")
    }

    fn signed(action: &Action) -> (SignedMessage, MessageData) {
        let envelope = Envelope {
            fid: 7,
            network: FarcasterNetwork::Mainnet,
            timestamp: Timestamp::from_unix(1_792_281_600),
        };
        let signed = sign(&AppKey::from_secret(&[0x11; 32]), &envelope, action).expect("signs");
        let data = decode(&signed).expect("decodes");
        (signed, data)
    }

    fn cast(text: &str) -> Action {
        Action::CastAdd(NewCast {
            text: text.to_owned(),
            mentions: Vec::new(),
            embeds: vec![
                Target::Url("https://example.com".to_owned()),
                Target::Cast(CastRef {
                    fid: 9,
                    hash: hash(0xbb),
                }),
            ],
            parent: Some(Target::Cast(CastRef {
                fid: 3,
                hash: hash(0xaa),
            })),
            kind: CastType::Cast,
        })
    }

    #[test]
    fn every_message_decodes_back_to_what_was_asked() {
        let to = CastRef {
            fid: 3,
            hash: hash(0xaa),
        };
        let cases = [
            (
                Action::CastRemove(hash(0xcc)),
                What::Delete(hash(0xcc)),
                "delete",
            ),
            (
                Action::ReactionAdd(ReactionType::Like, Target::Cast(to)),
                What::React {
                    undo: false,
                    kind: ReactionType::Like,
                    cast: to,
                },
                "like",
            ),
            (
                Action::ReactionRemove(ReactionType::Recast, Target::Cast(to)),
                What::React {
                    undo: true,
                    kind: ReactionType::Recast,
                    cast: to,
                },
                "unrecast",
            ),
            (
                Action::FollowAdd(3),
                What::Follow {
                    undo: false,
                    account: 3,
                },
                "follow",
            ),
            (
                Action::FollowRemove(3),
                What::Follow {
                    undo: true,
                    account: 3,
                },
                "unfollow",
            ),
        ];
        for (action, expected, kind) in cases {
            let decoded = what(&signed(&action).1).expect("known");
            assert_eq!(decoded, expected);
            assert_eq!(kind_name(&decoded), kind);
        }
        let decoded = what(&signed(&cast("hi")).1).expect("known");
        assert!(
            matches!(&decoded, What::Cast { text, embeds, parent: Some(_) }
            if text == "hi" && embeds.len() == 2)
        );
    }

    #[test]
    fn the_confirmation_shows_every_part_of_the_cast() {
        let (signed, data) = signed(&cast("first line\nsecond line"));
        let shown = confirmation(
            &data,
            &what(&data).expect("known"),
            &signed,
            Some("alice".into()),
        );
        for part in [
            "@alice (FID 7)",
            &signed.signer().to_string(),
            "2026-10-18T00:00:00Z",
            "    | first line\n    | second line",
            &format!("Reply to: cast 3:{}", hash(0xaa)),
            "Embed:    https://example.com",
            &format!("Embed:    cast 9:{}", hash(0xbb)),
        ] {
            assert!(shown.contains(part), "missing {part:?} in:\n{shown}");
        }
    }

    #[test]
    fn control_characters_in_the_text_are_shown_escaped() {
        let (signed, data) = signed(&cast("ok\u{1b}[2K\rhidden"));
        let shown = confirmation(&data, &what(&data).expect("known"), &signed, None);
        assert!(
            !shown.contains('\u{1b}') && !shown.contains('\r'),
            "{shown:?}"
        );
        assert!(shown.contains(r"ok\u{1b}[2K\rhidden"), "{shown}");
    }

    #[test]
    fn an_add_lands_as_itself_and_an_undo_as_nothing_or_itself() {
        let (ours, other) = (hash(1), hash(2));
        assert!(shows(Some(ours), false, ours));
        assert!(!shows(Some(other), false, ours));
        assert!(!shows(None, false, ours));
        assert!(shows(None, true, ours));
        assert!(shows(Some(ours), true, ours));
        assert!(!shows(Some(other), true, ours));
    }

    #[test]
    fn an_embed_is_a_url_or_a_cast() {
        assert_eq!(
            embed_arg("https://example.com/a").expect("url"),
            Target::Url("https://example.com/a".to_owned())
        );
        let cast = format!("9:{}", hash(0xbb));
        assert!(matches!(
            embed_arg(&cast),
            Ok(Target::Cast(CastRef { fid: 9, .. }))
        ));
        assert!(embed_arg("example.com").is_err());
    }

    #[test]
    fn cast_text_sends_long_text_as_a_long_cast_and_refuses_past_1024_bytes() {
        let long = cast_text(&"a".repeat(619)).expect("fits a long cast");
        assert_eq!(long.kind, CastType::LongCast);
        assert_eq!(cast_text("gm").expect("fits").kind, CastType::Cast);
        let err = cast_text(&"a".repeat(1025)).expect_err("too long");
        assert!(err.starts_with("1025 bytes"), "{err}");
    }
}
