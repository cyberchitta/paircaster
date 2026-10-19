//! `CastAddBody`'s protobuf encoding, matching ts-proto's byte for byte.
//!
//! ts-proto writes fields in `.proto` declaration order (`message.proto:143`:
//! 1, 2, the `parent` oneof's 3 or 7, then 4, 5, 6, 8), skips proto3 defaults
//! for scalars, and always writes the two packed repeated fields, 2 and 5.
//! Decoding accepts both packed and unpacked repeated fields, as protobuf
//! requires.

use prost::DecodeError;
use prost::bytes::{Buf, BufMut};
use prost::encoding::{
    DecodeContext, WireType, encode_key, encode_varint, encoded_len_varint, int32, key_len,
    message, skip_field, string, uint32, uint64,
};

use super::{CastAddBody, CastParent};

impl prost::Message for CastAddBody {
    fn encode_raw(&self, buf: &mut impl BufMut) {
        string::encode_repeated(1, &self.embeds_deprecated, buf);
        encode_packed_always(2, &self.mentions, buf);
        if let Some(parent) = &self.parent {
            parent.encode(buf);
        }
        if !self.text.is_empty() {
            string::encode(4, &self.text, buf);
        }
        encode_packed_always(5, &self.varint_positions(), buf);
        message::encode_repeated(6, &self.embeds, buf);
        if self.r#type != 0 {
            int32::encode(8, &self.r#type, buf);
        }
    }

    fn merge_field(
        &mut self,
        tag: u32,
        wire_type: WireType,
        buf: &mut impl Buf,
        ctx: DecodeContext,
    ) -> Result<(), DecodeError> {
        match tag {
            1 => string::merge_repeated(wire_type, &mut self.embeds_deprecated, buf, ctx),
            2 => uint64::merge_repeated(wire_type, &mut self.mentions, buf, ctx),
            3 | 7 => CastParent::merge(&mut self.parent, tag, wire_type, buf, ctx),
            4 => string::merge(wire_type, &mut self.text, buf, ctx),
            5 => uint32::merge_repeated(wire_type, &mut self.mentions_positions, buf, ctx),
            6 => message::merge_repeated(wire_type, &mut self.embeds, buf, ctx),
            8 => int32::merge(wire_type, &mut self.r#type, buf, ctx),
            _ => skip_field(wire_type, tag, buf, ctx),
        }
    }

    fn encoded_len(&self) -> usize {
        string::encoded_len_repeated(1, &self.embeds_deprecated)
            + packed_always_len(2, &self.mentions)
            + self.parent.as_ref().map_or(0, CastParent::encoded_len)
            + if self.text.is_empty() {
                0
            } else {
                string::encoded_len(4, &self.text)
            }
            + packed_always_len(5, &self.varint_positions())
            + message::encoded_len_repeated(6, &self.embeds)
            + if self.r#type == 0 {
                0
            } else {
                int32::encoded_len(8, &self.r#type)
            }
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

impl CastAddBody {
    fn varint_positions(&self) -> Vec<u64> {
        self.mentions_positions
            .iter()
            .map(|p| u64::from(*p))
            .collect()
    }
}

fn encode_packed_always(tag: u32, values: &[u64], buf: &mut impl BufMut) {
    encode_key(tag, WireType::LengthDelimited, buf);
    encode_varint(packed_body_len(values) as u64, buf);
    values.iter().for_each(|v| encode_varint(*v, buf));
}

fn packed_always_len(tag: u32, values: &[u64]) -> usize {
    let body = packed_body_len(values);
    key_len(tag) + encoded_len_varint(body as u64) + body
}

fn packed_body_len(values: &[u64]) -> usize {
    values.iter().map(|v| encoded_len_varint(*v)).sum()
}
