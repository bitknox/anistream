//! The Cast v2 wire format: a 4-byte big-endian length, then one `CastMessage` protobuf.
//!
//! The message has seven fields and every control message rides in its string payload as JSON,
//! so it is encoded by hand here rather than through a protobuf crate and a build step:
//!
//! ```text
//! message CastMessage {
//!   required ProtocolVersion protocol_version = 1;  // CASTV2_1_0 = 0
//!   required string source_id = 2;
//!   required string destination_id = 3;
//!   required string namespace = 4;
//!   required PayloadType payload_type = 5;          // STRING = 0, BINARY = 1
//!   optional string payload_utf8 = 6;
//!   optional bytes payload_binary = 7;
//! }
//! ```

/// The largest message accepted from a device. Media status with a long track list is a few
/// kilobytes; the protocol's own ceiling is 64 KiB.
pub const MAX_MESSAGE: usize = 64 * 1024;

/// One Cast message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastMessage {
    pub source: String,
    pub destination: String,
    pub namespace: String,
    pub payload: Payload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Payload {
    Text(String),
    Binary(Vec<u8>),
}

impl CastMessage {
    pub fn text(
        source: impl Into<String>,
        destination: impl Into<String>,
        namespace: impl Into<String>,
        payload: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            destination: destination.into(),
            namespace: namespace.into(),
            payload: Payload::Text(payload.into()),
        }
    }

    /// The protobuf bytes, without the length prefix.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(64 + self.namespace.len());
        varint_field(&mut out, 1, 0);
        bytes_field(&mut out, 2, self.source.as_bytes());
        bytes_field(&mut out, 3, self.destination.as_bytes());
        bytes_field(&mut out, 4, self.namespace.as_bytes());
        match &self.payload {
            Payload::Text(text) => {
                varint_field(&mut out, 5, 0);
                bytes_field(&mut out, 6, text.as_bytes());
            }
            Payload::Binary(data) => {
                varint_field(&mut out, 5, 1);
                bytes_field(&mut out, 7, data);
            }
        }
        out
    }

    /// The length-prefixed frame that goes on the wire.
    pub fn frame(&self) -> Vec<u8> {
        let body = self.encode();
        let mut out = Vec::with_capacity(body.len() + 4);
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        out.extend_from_slice(&body);
        out
    }

    /// Parse the protobuf bytes of one message. Unknown fields are skipped, as protobuf says.
    pub fn decode(mut data: &[u8]) -> Result<Self, DecodeError> {
        let mut message = CastMessage {
            source: String::new(),
            destination: String::new(),
            namespace: String::new(),
            payload: Payload::Text(String::new()),
        };
        let mut binary = false;
        let mut text = None;
        let mut bytes = None;
        while !data.is_empty() {
            let key = read_varint(&mut data)?;
            let (field, wire) = (key >> 3, key & 7);
            match wire {
                0 => {
                    let value = read_varint(&mut data)?;
                    if field == 5 {
                        binary = value == 1;
                    }
                }
                2 => {
                    let len = read_varint(&mut data)? as usize;
                    if len > data.len() {
                        return Err(DecodeError("a field runs past the message"));
                    }
                    let (value, rest) = data.split_at(len);
                    data = rest;
                    let string = || {
                        String::from_utf8(value.to_vec())
                            .map_err(|_| DecodeError("a string field is not UTF-8"))
                    };
                    match field {
                        2 => message.source = string()?,
                        3 => message.destination = string()?,
                        4 => message.namespace = string()?,
                        6 => text = Some(string()?),
                        7 => bytes = Some(value.to_vec()),
                        _ => {}
                    }
                }
                5 => data = data.get(4..).ok_or(DecodeError("a short fixed32"))?,
                1 => data = data.get(8..).ok_or(DecodeError("a short fixed64"))?,
                _ => return Err(DecodeError("an unknown wire type")),
            }
        }
        message.payload = if binary {
            Payload::Binary(bytes.unwrap_or_default())
        } else {
            Payload::Text(text.unwrap_or_default())
        };
        Ok(message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a malformed Cast message: {0}")]
pub struct DecodeError(&'static str);

fn varint(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn varint_field(out: &mut Vec<u8>, field: u64, value: u64) {
    varint(out, field << 3);
    varint(out, value);
}

fn bytes_field(out: &mut Vec<u8>, field: u64, value: &[u8]) {
    varint(out, (field << 3) | 2);
    varint(out, value.len() as u64);
    out.extend_from_slice(value);
}

fn read_varint(data: &mut &[u8]) -> Result<u64, DecodeError> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let (&byte, rest) = data.split_first().ok_or(DecodeError("a short varint"))?;
        *data = rest;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(DecodeError("a varint longer than 64 bits"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_survives_the_round_trip() {
        let message = CastMessage::text(
            "sender-0",
            "receiver-0",
            "urn:x-cast:com.google.cast.tp.connection",
            r#"{"type":"CONNECT"}"#,
        );
        assert_eq!(CastMessage::decode(&message.encode()).unwrap(), message);
        let frame = message.frame();
        assert_eq!(u32::from_be_bytes(frame[..4].try_into().unwrap()) as usize, frame.len() - 4);
    }

    #[test]
    fn the_bytes_are_what_protobuf_writes() {
        // protoc --encode of {protocol_version: 0, source_id: "a", destination_id: "b",
        // namespace: "n", payload_type: STRING, payload_utf8: "{}"}.
        let message = CastMessage::text("a", "b", "n", "{}");
        assert_eq!(
            message.encode(),
            [0x08, 0, 0x12, 1, b'a', 0x1a, 1, b'b', 0x22, 1, b'n', 0x28, 0, 0x32, 2, b'{', b'}']
        );
    }

    #[test]
    fn unknown_fields_are_skipped() {
        let mut data = CastMessage::text("a", "b", "n", "{}").encode();
        // Field 8 (`continued`, a bool) and field 9 (`remaining_length`), as newer devices send.
        data.extend([0x40, 0x00, 0x48, 0x05]);
        assert_eq!(CastMessage::decode(&data).unwrap().payload, Payload::Text("{}".into()));
    }
}
