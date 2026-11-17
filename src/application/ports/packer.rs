//! Binary packing contract used by the legacy protocol adapter.

#[derive(Debug, thiserror::Error)]
pub enum PackerError {
    #[error("failed to compress protocol message")]
    Compression,
    #[error("packed protocol message is malformed")]
    Malformed,
}

pub trait PackerPort: Send + Sync {
    fn pack(&self, message: &[u8]) -> Result<Vec<u8>, PackerError>;
    fn unpack(&self, packed_message: &[u8]) -> Result<Vec<u8>, PackerError>;
    fn is_packed(&self, message: &[u8]) -> bool;
}
