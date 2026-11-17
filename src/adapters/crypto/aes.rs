use aes::{
    Aes128, Aes192, Aes256,
    cipher::{Block, BlockDecrypt, BlockEncrypt, KeyInit, consts::U16},
};

use crate::application::ports::crypto::CryptoError;

pub(super) fn encrypt(key: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let padding = 16 - (plaintext.len() % 16);
    let mut padded = plaintext.to_vec();
    padded.resize(plaintext.len() + padding, padding as u8);
    with_cipher(key, |cipher| encrypt_blocks(cipher, &mut padded))?;
    Ok(padded)
}

pub(super) fn decrypt(key: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if ciphertext.is_empty() || ciphertext.len() % 16 != 0 {
        return Err(CryptoError::InvalidCiphertext);
    }
    let mut plaintext = ciphertext.to_vec();
    with_cipher(key, |cipher| decrypt_blocks(cipher, &mut plaintext))?;
    let padding = usize::from(*plaintext.last().ok_or(CryptoError::InvalidCiphertext)?);
    if !(1..=16).contains(&padding)
        || plaintext[plaintext.len() - padding..]
            .iter()
            .any(|byte| usize::from(*byte) != padding)
    {
        return Err(CryptoError::InvalidCiphertext);
    }
    plaintext.truncate(plaintext.len() - padding);
    Ok(plaintext)
}

fn with_cipher<T>(
    key: &[u8],
    operation: impl FnOnce(&dyn CipherBlocks) -> T,
) -> Result<T, CryptoError> {
    match key.len() {
        16 => Ok(operation(
            &Aes128::new_from_slice(key).map_err(|_| CryptoError::InvalidKeyLength)?,
        )),
        24 => Ok(operation(
            &Aes192::new_from_slice(key).map_err(|_| CryptoError::InvalidKeyLength)?,
        )),
        32 => Ok(operation(
            &Aes256::new_from_slice(key).map_err(|_| CryptoError::InvalidKeyLength)?,
        )),
        _ => Err(CryptoError::InvalidKeyLength),
    }
}

trait CipherBlocks {
    fn process(&self, bytes: &mut [u8], encrypt: bool);
}

impl<T> CipherBlocks for T
where
    T: BlockEncrypt<BlockSize = U16> + BlockDecrypt<BlockSize = U16>,
{
    fn process(&self, bytes: &mut [u8], encrypt: bool) {
        for chunk in bytes.chunks_exact_mut(16) {
            let mut block = Block::<T>::default();
            block.copy_from_slice(chunk);
            if encrypt {
                self.encrypt_block(&mut block);
            } else {
                self.decrypt_block(&mut block);
            }
            chunk.copy_from_slice(&block);
        }
    }
}

fn encrypt_blocks(cipher: &dyn CipherBlocks, bytes: &mut [u8]) {
    cipher.process(bytes, true);
}
fn decrypt_blocks(cipher: &dyn CipherBlocks, bytes: &mut [u8]) {
    cipher.process(bytes, false);
}
