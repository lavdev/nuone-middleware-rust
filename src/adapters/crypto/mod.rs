//! Adapters for the HTTP AES contract and legacy TCP XOR transform.

mod aes;

use base64::{Engine, engine::general_purpose::STANDARD};

use crate::{
    application::ports::crypto::{CryptoError, CryptoPort},
    domain::protocol::{XOR_KEY, decode_proto_text, encode_proto_text},
};

/// Implements AES/ECB/PKCS5Padding + standard Base64 for HTTP and XOR for TCP.
pub struct CryptoAdapter {
    key: Vec<u8>,
}

impl CryptoAdapter {
    /// The key follows Java's default UTF-8 `String.getBytes()` behavior.
    pub fn new(key: &str) -> Self {
        Self {
            key: key.as_bytes().to_vec(),
        }
    }
}

impl CryptoPort for CryptoAdapter {
    fn encrypt_http_body(&self, plain_text: &str) -> Result<String, CryptoError> {
        let encrypted = aes::encrypt(&self.key, &encode_proto_text(plain_text))?;
        Ok(STANDARD.encode(encrypted))
    }

    fn decrypt_http_body(&self, encrypted_body: &str) -> Result<String, CryptoError> {
        let decoded = STANDARD
            .decode(encrypted_body)
            .map_err(|_| CryptoError::InvalidBase64)?;
        let plain_text = aes::decrypt(&self.key, &decoded)?;
        Ok(decode_proto_text(&plain_text))
    }

    fn apply_tcp_xor(&self, message: &[u8]) -> Vec<u8> {
        message.iter().map(|byte| byte ^ XOR_KEY).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_java_aes_base64_vector() {
        let crypto = CryptoAdapter::new("1234567890123456");
        let plaintext = r#"{"command":"PING","acentos":"acao"}"#;
        let encrypted = crypto.encrypt_http_body(plaintext).expect("valid AES key");
        assert_eq!(
            encrypted,
            "4DJRDPh8hmf0NWWUBd9x2tz5rgDh/pwmhvT284nTvY1P+A0sHua4B4FxAeUGfN8t"
        );
        assert_eq!(
            crypto
                .decrypt_http_body(&encrypted)
                .expect("valid ciphertext"),
            plaintext
        );
    }

    #[test]
    fn applies_legacy_xor_to_bytes() {
        let crypto = CryptoAdapter::new("1234567890123456");
        let encoded = encode_proto_text("PROTO");
        let encrypted = crypto.apply_tcp_xor(&encoded);
        assert_eq!(encrypted, [21, 23, 10, 17, 10]);
        assert_eq!(crypto.apply_tcp_xor(&encrypted), encoded);
    }

    #[test]
    fn rejects_invalid_keys_and_ciphertexts() {
        let bad_key = CryptoAdapter::new("short");
        assert!(matches!(
            bad_key.encrypt_http_body("x"),
            Err(CryptoError::InvalidKeyLength)
        ));

        let crypto = CryptoAdapter::new("1234567890123456");
        assert!(matches!(
            crypto.decrypt_http_body("not-base64!"),
            Err(CryptoError::InvalidBase64)
        ));
    }
}
