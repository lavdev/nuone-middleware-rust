//! Outbound encryption contracts used by HTTP and legacy TCP flows.

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("AES key must encode to 16, 24, or 32 bytes")]
    InvalidKeyLength,
    #[error("encrypted body is not valid Base64")]
    InvalidBase64,
    #[error("encrypted body has invalid padding or ciphertext")]
    InvalidCiphertext,
}

pub trait CryptoPort: Send + Sync {
    fn encrypt_http_body(&self, plain_text: &str) -> Result<String, CryptoError>;
    fn decrypt_http_body(&self, encrypted_body: &str) -> Result<String, CryptoError>;
    fn apply_tcp_xor(&self, message: &[u8]) -> Vec<u8>;
}
