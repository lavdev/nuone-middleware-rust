//! Outbound adapters for legacy protocol transformations.

use std::io::{Read, Write};

use flate2::{Compression, read::DeflateDecoder, write::DeflateEncoder};

use crate::{
    application::ports::packer::{PackerError, PackerPort},
    application::ports::proto_to_json::{ProtoToJsonError, ProtoToJsonPort},
    domain::protocol::{
        CLIENT_ACK_OK, LENGTH_END, PACK_END, PACK_START, PROTO_ALERT, PROTO_ASK, PROTO_ENDBLOCK,
        PROTO_ERRO, PROTO_FINALIZER, PROTO_MENS, PROTO_SEPARATOR, PROTO_W_BUTTON, PROTO_W_LABEL,
        PROTO_W_LABEL_ATIVO, PROTO_W_MDICHILD, PROTO_W_MDIHISTORICAL, PROTO_W_MODALWINDOW,
        PROTO_W_SERVER_VERSION, encode_proto_text,
    },
};

const MAX_UNIT_LENGTH: usize = 200;
const MAX_PACK_SIZE: usize = 10 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default)]
pub struct DeflatePackerAdapter;

#[derive(Clone, Copy, Debug, Default)]
pub struct ProtoToJsonAdapter;

impl ProtoToJsonPort for ProtoToJsonAdapter {
    fn translate(&self, response: &str) -> Result<String, ProtoToJsonError> {
        let mut components = Vec::new();
        let events: Vec<serde_json::Value> = Vec::new();
        let normalized = response
            .replace(PROTO_FINALIZER, "")
            .replace("\u{fffd}\u{fffd}\u{fffd}", "");

        for block in normalized
            .split(PROTO_ENDBLOCK)
            .filter(|block| !block.trim().is_empty())
        {
            let fields: Vec<&str> = block.split(PROTO_SEPARATOR).collect();
            if fields.len() < 2 {
                continue;
            }
            let unit = ProtoUnit {
                message_id: fields[0],
                header: fields[1],
                values: &fields[2..],
            };
            match unit.header {
                CLIENT_ACK_OK | PROTO_W_SERVER_VERSION => {}
                PROTO_MENS | PROTO_ASK | PROTO_ERRO => components.push(json_component(
                    "MessageBox",
                    serde_json::json!({
                        "text": unit.get(0),
                        "title": unit.get(1).replace(PROTO_FINALIZER, ""),
                        "error": unit.header == PROTO_ERRO,
                        "question": unit.header == PROTO_ASK,
                        "messageID": unit.message_id,
                    }),
                )),
                PROTO_ALERT => {
                    let mut options = format!("{},", unit.get(2));
                    for value in unit.values.iter().skip(3).filter(|value| !value.is_empty()) {
                        options.push_str(value);
                        options.push(',');
                    }
                    components.push(json_component(
                        "Alert",
                        serde_json::json!({
                            "id": unit.message_id,
                            "text": unit.get(0),
                            "title": unit.get(1),
                            "options": options,
                        }),
                    ));
                }
                PROTO_W_MDICHILD | PROTO_W_MDIHISTORICAL => components.push(json_component(
                    "Window",
                    serde_json::json!({
                        "id": unit.message_id,
                        "title": unit.get(0),
                        "top": unit.get(2),
                        "left": unit.get(1),
                        "width": unit.get(3),
                        "height": unit.get(4),
                        "modal": "false",
                        "historic": unit.header == PROTO_W_MDIHISTORICAL,
                    }),
                )),
                PROTO_W_MODALWINDOW => components.push(json_component(
                    "Window",
                    serde_json::json!({
                        "id": unit.message_id,
                        "title": unit.get(0),
                        "width": unit.get(1),
                        "height": unit.get(2),
                        "modal": true,
                    }),
                )),
                PROTO_W_LABEL | PROTO_W_LABEL_ATIVO => {
                    let name = if unit.header == PROTO_W_LABEL_ATIVO {
                        "LabelActive"
                    } else {
                        "Label"
                    };
                    components.push(json_component(
                        name,
                        serde_json::json!({
                            "id": unit.message_id,
                            "parent": unit.get(0),
                            "text": unit.get(1),
                            "left": unit.get(2),
                            "top": unit.get(3),
                            "width": unit.get(4),
                            "height": unit.get(5),
                        }),
                    ));
                }
                PROTO_W_BUTTON => {
                    let text = normalize_button_text(unit.get(1));
                    components.push(json_component(
                        "Button",
                        serde_json::json!({
                            "id": unit.message_id,
                            "parent": unit.get(0),
                            "text": text,
                            "left": unit.get(2),
                            "top": unit.get(3),
                            "width": unit.get(4),
                            "height": unit.get(5),
                        }),
                    ));
                }
                _ => {}
            }
        }

        serde_json::to_string(&serde_json::json!({
            "content": { "Component": components, "Event": events }
        }))
        .map_err(|_| ProtoToJsonError::Translation)
    }
}

struct ProtoUnit<'a> {
    message_id: &'a str,
    header: &'a str,
    values: &'a [&'a str],
}

impl ProtoUnit<'_> {
    fn get(&self, index: usize) -> &str {
        self.values.get(index).copied().unwrap_or_default()
    }
}

fn json_component(name: &str, data: serde_json::Value) -> serde_json::Value {
    let mut component = serde_json::Map::new();
    component.insert(name.to_owned(), data);
    serde_json::Value::Object(component)
}

fn normalize_button_text(text: &str) -> String {
    let replacements = [
        ("[+]", "[plus]"),
        ("[V]", "[check]"),
        ("[v]", "[check]"),
        ("[...]", "[searchengin]"),
    ];
    let Some(end) = text.find(']') else {
        return text.to_owned();
    };
    if !text.starts_with('[') {
        return text.to_owned();
    }
    let token = &text[..=end];
    let Some((_, replacement)) = replacements.iter().find(|(legacy, _)| *legacy == token) else {
        return text.to_owned();
    };
    let label = &text[end + 1..];
    if !label.is_empty() && !label.chars().next().is_some_and(char::is_whitespace) {
        format!("{replacement} {label}")
    } else {
        format!("{replacement}{label}")
    }
}

impl PackerPort for DeflatePackerAdapter {
    fn pack(&self, message: &[u8]) -> Result<Vec<u8>, PackerError> {
        if message.len() <= MAX_UNIT_LENGTH || message.len() > MAX_PACK_SIZE {
            return Ok(message.to_vec());
        }

        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
        encoder
            .write_all(message)
            .map_err(|_| PackerError::Compression)?;
        let compressed = encoder.finish().map_err(|_| PackerError::Compression)?;

        let mut packed =
            Vec::with_capacity(PACK_START.len() + compressed.len() + PACK_END.len() + 24);
        packed.extend_from_slice(&encode_proto_text(PACK_START));
        packed.extend_from_slice(&compressed);
        packed.extend_from_slice(&encode_proto_text(PACK_END));
        packed.extend_from_slice(message.len().to_string().as_bytes());
        packed.extend_from_slice(&encode_proto_text(LENGTH_END));
        Ok(packed)
    }

    fn unpack(&self, packed_message: &[u8]) -> Result<Vec<u8>, PackerError> {
        if !self.is_packed(packed_message) {
            return Ok(packed_message.to_vec());
        }
        let start = encode_proto_text(PACK_START);
        let end = encode_proto_text(PACK_END);
        let payload_start = start.len();
        let payload_end =
            find_subslice(packed_message, &end, payload_start).ok_or(PackerError::Malformed)?;

        let mut decoder = DeflateDecoder::new(&packed_message[payload_start..payload_end]);
        let mut unpacked = Vec::new();
        decoder
            .read_to_end(&mut unpacked)
            .map_err(|_| PackerError::Malformed)?;
        Ok(unpacked)
    }

    fn is_packed(&self, message: &[u8]) -> bool {
        let start = encode_proto_text(PACK_START);
        let end = encode_proto_text(PACK_END);
        message.starts_with(&start) && find_subslice(message, &end, start.len()).is_some()
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from > haystack.len() {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|position| from + position)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};

    #[test]
    fn leaves_short_messages_as_copies() {
        let packer = DeflatePackerAdapter;
        let message = b"short-proto-message";
        let packed = packer.pack(message).expect("small message");
        assert_eq!(packed, message);
        assert!(!packer.is_packed(&packed));
    }

    #[test]
    fn preserves_legacy_envelope_and_round_trips() {
        let packer = DeflatePackerAdapter;
        let message = b"PROTO_FIELD;".repeat(30);
        let packed = packer.pack(&message).expect("pack succeeds");
        assert!(packer.is_packed(&packed));
        let envelope = String::from_utf8_lossy(&packed);
        assert!(envelope.starts_with(PACK_START));
        assert!(envelope.contains(PACK_END));
        assert!(envelope.ends_with(&format!("{}{}", message.len(), LENGTH_END)));
        assert_eq!(packer.unpack(&packed).expect("unpack succeeds"), message);
    }

    #[test]
    fn unpacks_java_raw_deflate_fixture() {
        let message = b"PROTO_FIELD;".repeat(30);
        let java_envelope = [
            encode_proto_text(PACK_START),
            STANDARD
                .decode("CwjyD/GPd/N09XGxDhhl04wNAA==")
                .expect("fixture base64"),
            encode_proto_text(PACK_END),
            message.len().to_string().into_bytes(),
            encode_proto_text(LENGTH_END),
        ]
        .concat();
        assert_eq!(
            DeflatePackerAdapter
                .unpack(&java_envelope)
                .expect("Java deflate fixture unpacks"),
            message
        );
    }

    #[test]
    fn rejects_malformed_compressed_payload() {
        let packed = [
            encode_proto_text(PACK_START),
            b"bad".to_vec(),
            encode_proto_text(PACK_END),
        ]
        .concat();
        assert!(matches!(
            DeflatePackerAdapter.unpack(&packed),
            Err(PackerError::Malformed)
        ));
    }
}
