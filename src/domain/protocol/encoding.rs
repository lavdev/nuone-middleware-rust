/// Encodes text using Java-compatible ISO-8859-1 replacement behavior.
///
/// Characters outside the Latin-1 range become `?`, matching
/// `String.getBytes(StandardCharsets.ISO_8859_1)`.
pub fn encode_proto_text(value: &str) -> Vec<u8> {
    value
        .chars()
        .map(|character| {
            let codepoint = character as u32;
            if codepoint <= u8::MAX as u32 {
                codepoint as u8
            } else {
                b'?'
            }
        })
        .collect()
}

/// Decodes ISO-8859-1 bytes by mapping each byte to the same Unicode codepoint.
pub fn decode_proto_text(value: &[u8]) -> String {
    value.iter().copied().map(char::from).collect()
}

#[cfg(test)]
mod tests {
    use super::{decode_proto_text, encode_proto_text};

    #[test]
    fn round_trips_latin1_boundary_bytes() {
        let bytes = [0x00, 0x7f, 0x80, 0xa4, 0xf7, 0xff];
        let text = decode_proto_text(&bytes);

        assert_eq!(encode_proto_text(&text), bytes);
    }

    #[test]
    fn replaces_characters_outside_latin1_with_question_mark() {
        assert_eq!(encode_proto_text("A€漢"), [b'A', b'?', b'?']);
    }
}
