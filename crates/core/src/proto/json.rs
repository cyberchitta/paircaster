//! Converts a message in a Snapchain node's HTTP JSON into [`Message`].
//!
//! The JSON shapes follow snapchain v0.14.3 `src/network/http_server.rs`
//! (`Message` at line 113, the bodies after it, and the proto-to-JSON mapping
//! from line 2377). Byte fields come in two encodings there: the envelope's
//! `hash` and `signer` and every `CastId.hash` are `0x` hex, while
//! `signature` and `castRemoveBody.targetHash` are standard base64.
//! Unknown fields are allowed, since the format will grow. Enum values are
//! read as text first, so a value a newer node adds is an
//! [`Error::Unknown`], not a malformed response.

use base64::prelude::*;
use serde::de::DeserializeOwned;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use super::{
    Body, CastAddBody, CastId, CastParent, CastRemoveBody, CastType, Embed, EmbedTarget,
    FarcasterNetwork, HashScheme, LinkBody, LinkTarget, Message, MessageData, MessageType,
    ReactionBody, ReactionTarget, ReactionType, SignatureScheme, UserDataBody, UserDataType,
};
use crate::Error;

/// Parses one message as the node's HTTP API returns it, e.g. an element of
/// a `castsByFid` response's `messages`.
///
/// `data_bytes` is `None` in the result: the node's JSON does not carry it.
pub fn message_from_json(value: serde_json::Value) -> Result<Message, Error> {
    let json: JsonMessage =
        serde_json::from_value(value).map_err(|e| Error::Malformed(e.to_string()))?;
    json.try_into()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonMessage {
    data: JsonData,
    #[serde(deserialize_with = "hex_bytes")]
    hash: Vec<u8>,
    hash_scheme: String,
    #[serde(deserialize_with = "base64_bytes")]
    signature: Vec<u8>,
    signature_scheme: String,
    #[serde(deserialize_with = "hex_bytes")]
    signer: Vec<u8>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonData {
    #[serde(rename = "type")]
    message_type: String,
    fid: u64,
    timestamp: u32,
    network: String,
    cast_add_body: Option<JsonCastAdd>,
    cast_remove_body: Option<JsonCastRemove>,
    reaction_body: Option<JsonReaction>,
    link_body: Option<JsonLink>,
    user_data_body: Option<JsonUserData>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonCastAdd {
    embeds_deprecated: Vec<String>,
    mentions: Vec<u64>,
    parent_cast_id: Option<JsonCastId>,
    parent_url: Option<String>,
    text: String,
    embeds: Vec<JsonEmbed>,
    mentions_positions: Vec<u32>,
    #[serde(rename = "type")]
    cast_type: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonCastRemove {
    #[serde(deserialize_with = "base64_bytes")]
    target_hash: Vec<u8>,
}

#[derive(Deserialize)]
struct JsonCastId {
    fid: u64,
    #[serde(deserialize_with = "hex_bytes")]
    hash: Vec<u8>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonEmbed {
    url: Option<String>,
    cast_id: Option<JsonCastId>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonReaction {
    #[serde(rename = "type")]
    reaction_type: String,
    target_cast_id: Option<JsonCastId>,
    target_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonLink {
    #[serde(rename = "type")]
    link_type: String,
    display_timestamp: Option<u32>,
    target_fid: u64,
}

#[derive(Deserialize)]
struct JsonUserData {
    #[serde(rename = "type")]
    user_data_type: String,
    value: String,
}

impl TryFrom<JsonMessage> for Message {
    type Error = Error;

    fn try_from(json: JsonMessage) -> Result<Self, Error> {
        let data = json.data.try_into()?;
        Ok(Message {
            data: Some(data),
            hash: json.hash,
            hash_scheme: known::<HashScheme>(&json.hash_scheme)?.into(),
            signature: json.signature,
            signature_scheme: known::<SignatureScheme>(&json.signature_scheme)?.into(),
            signer: json.signer,
            data_bytes: None,
        })
    }
}

impl TryFrom<JsonData> for MessageData {
    type Error = Error;

    fn try_from(json: JsonData) -> Result<Self, Error> {
        let message_type = known::<MessageType>(&json.message_type)?;
        let body = body_for(&json, message_type)?;
        Ok(MessageData {
            r#type: message_type.into(),
            fid: json.fid,
            timestamp: json.timestamp,
            network: known::<FarcasterNetwork>(&json.network)?.into(),
            body: Some(body),
        })
    }
}

fn one_body(json: &JsonData) -> Result<(), Error> {
    let present = [
        json.cast_add_body.is_some(),
        json.cast_remove_body.is_some(),
        json.reaction_body.is_some(),
        json.link_body.is_some(),
        json.user_data_body.is_some(),
    ];
    if present.iter().filter(|p| **p).count() != 1 {
        return Err(bad("message must have exactly one body"));
    }
    Ok(())
}

fn body_for(json: &JsonData, message_type: MessageType) -> Result<Body, Error> {
    one_body(json)?;
    let missing = || bad(&format!("{message_type:?} message without its body"));
    match message_type {
        MessageType::CastAdd => json
            .cast_add_body
            .as_ref()
            .map(cast_add)
            .ok_or_else(missing)?,
        MessageType::CastRemove => json
            .cast_remove_body
            .as_ref()
            .map(cast_remove)
            .ok_or_else(missing),
        MessageType::ReactionAdd | MessageType::ReactionRemove => json
            .reaction_body
            .as_ref()
            .map(reaction)
            .ok_or_else(missing)?,
        MessageType::LinkAdd | MessageType::LinkRemove => {
            json.link_body.as_ref().map(link).ok_or_else(missing)
        }
        MessageType::UserDataAdd => json
            .user_data_body
            .as_ref()
            .map(user_data)
            .ok_or_else(missing)?,
        MessageType::KeyAdd => Err(bad("KEY_ADD messages are not read")),
        MessageType::None => Err(bad("message type is NONE")),
    }
}

fn cast_add(json: &JsonCastAdd) -> Result<Body, Error> {
    let parent = match (&json.parent_cast_id, &json.parent_url) {
        (Some(id), None) => Some(CastParent::CastId(cast_id(id))),
        (None, Some(url)) => Some(CastParent::Url(url.clone())),
        (None, None) => None,
        (Some(_), Some(_)) => return Err(bad("cast has both a parent cast and a parent URL")),
    };
    let embeds = json.embeds.iter().map(embed).collect::<Result<_, _>>()?;
    Ok(Body::CastAdd(CastAddBody {
        embeds_deprecated: json.embeds_deprecated.clone(),
        mentions: json.mentions.clone(),
        parent,
        text: json.text.clone(),
        mentions_positions: json.mentions_positions.clone(),
        embeds,
        r#type: known::<CastType>(&json.cast_type)?.into(),
    }))
}

fn cast_remove(json: &JsonCastRemove) -> Body {
    Body::CastRemove(CastRemoveBody {
        target_hash: json.target_hash.clone(),
    })
}

fn embed(json: &JsonEmbed) -> Result<Embed, Error> {
    let target = match (&json.url, &json.cast_id) {
        (Some(url), None) => EmbedTarget::Url(url.clone()),
        (None, Some(id)) => EmbedTarget::CastId(cast_id(id)),
        _ => return Err(bad("embed must have exactly one of url and castId")),
    };
    Ok(Embed {
        embed: Some(target),
    })
}

fn reaction(json: &JsonReaction) -> Result<Body, Error> {
    let target = match (&json.target_cast_id, &json.target_url) {
        (Some(id), None) => ReactionTarget::CastId(cast_id(id)),
        (None, Some(url)) => ReactionTarget::Url(url.clone()),
        _ => {
            return Err(bad(
                "reaction must have exactly one of targetCastId and targetUrl",
            ));
        }
    };
    Ok(Body::Reaction(ReactionBody {
        r#type: known::<ReactionType>(&json.reaction_type)?.into(),
        target: Some(target),
    }))
}

fn link(json: &JsonLink) -> Body {
    Body::Link(LinkBody {
        r#type: json.link_type.clone(),
        display_timestamp: json.display_timestamp,
        target: Some(LinkTarget::Fid(json.target_fid)),
    })
}

fn user_data(json: &JsonUserData) -> Result<Body, Error> {
    Ok(Body::UserData(UserDataBody {
        r#type: known::<UserDataType>(&json.user_data_type)?.into(),
        value: json.value.clone(),
    }))
}

fn cast_id(json: &JsonCastId) -> CastId {
    CastId {
        fid: json.fid,
        hash: json.hash.clone(),
    }
}

/// One of the node's enum values, by its name: `CAST_TYPE_CAST` and the like.
/// A name not in the enum is [`Error::Unknown`].
fn known<T: DeserializeOwned>(name: &str) -> Result<T, Error> {
    serde_json::from_value(serde_json::Value::String(name.to_owned()))
        .map_err(|_| Error::Unknown(format!("the value {name}")))
}

fn bad(reason: &str) -> Error {
    Error::Malformed(reason.to_owned())
}

fn hex_bytes<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
    let s = String::deserialize(d)?;
    let digits = s
        .strip_prefix("0x")
        .ok_or_else(|| D::Error::custom(format!("{s:?} is not 0x-prefixed hex")))?;
    hex::decode(digits).map_err(D::Error::custom)
}

fn base64_bytes<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
    let s = String::deserialize(d)?;
    BASE64_STANDARD.decode(&s).map_err(D::Error::custom)
}
