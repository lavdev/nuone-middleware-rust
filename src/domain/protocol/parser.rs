#[derive(Clone, Copy, Debug, Default)]
pub struct ProtoParser;

impl ProtoParser {
    pub fn convert_proto_boolean<'a>(&self, value: &'a str) -> &'a str {
        convert_proto_boolean(value)
    }
}

pub fn convert_proto_boolean(value: &str) -> &str {
    match value {
        "S" | ".T." => "true",
        "N" | ".F." => "false",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::{ProtoParser, convert_proto_boolean};

    #[test]
    fn converts_legacy_true_and_false_tokens() {
        for value in ["S", ".T."] {
            assert_eq!(convert_proto_boolean(value), "true");
        }
        for value in ["N", ".F."] {
            assert_eq!(convert_proto_boolean(value), "false");
        }
    }

    #[test]
    fn preserves_unrecognized_values() {
        let value = "s";
        assert_eq!(convert_proto_boolean(value), value);
        assert_eq!(ProtoParser.convert_proto_boolean(value), value);
    }
}
