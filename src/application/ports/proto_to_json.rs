//! Application boundary for turning legacy ERP PROTO replies into client JSON.

#[derive(Debug, thiserror::Error)]
pub enum ProtoToJsonError {
    #[error("failed to translate ERP PROTO response")]
    Translation,
}

pub trait ProtoToJsonPort: Send + Sync {
    fn translate(&self, response: &str) -> Result<String, ProtoToJsonError>;
}
