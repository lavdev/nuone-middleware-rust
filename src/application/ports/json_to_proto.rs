//! Application boundary for translating client JSON commands into legacy PROTO.

#[derive(Debug, thiserror::Error)]
pub enum JsonToProtoError {
    #[error("invalid command JSON")]
    InvalidJson,
    #[error("unsupported command")]
    UnsupportedCommand,
    #[error("missing command field: {0}")]
    MissingField(String),
    #[error("ClientData must be an array")]
    ClientDataMustBeArray,
}

pub trait JsonToProtoPort: Send + Sync {
    fn translate(&self, command_json: &str) -> Result<String, JsonToProtoError>;
}
