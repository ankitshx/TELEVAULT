//! Authenticated AES-256-GCM encryption and decryption primitives.

use crate::aad::ChunkAad;
use crate::error::{CryptoError, Result};
use crate::key::{Nonce, SecretKey};
use crate::payload::{EncryptedPayload, KdfMetadata};
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::Aes256Gcm;

/// Encrypts raw plaintext using AES-256-GCM with a freshly generated nonce.
///
/// An optional AAD byte slice may be provided to bind context.
pub fn encrypt_raw(
    key: &SecretKey,
    plaintext: &[u8],
    aad: Option<&[u8]>,
    kdf_metadata: Option<KdfMetadata>,
) -> Result<EncryptedPayload> {
    let nonce = Nonce::generate();
    let cipher_nonce = aes_gcm::Nonce::from_slice(nonce.as_bytes());

    let cipher =
        Aes256Gcm::new_from_slice(key.as_bytes()).map_err(|_| CryptoError::InvalidKeyLength {
            expected: 32,
            actual: key.as_bytes().len(),
        })?;

    let payload = Payload {
        msg: plaintext,
        aad: aad.unwrap_or(b""),
    };

    let ciphertext = cipher
        .encrypt(cipher_nonce, payload)
        .map_err(|_| CryptoError::AuthenticationFailed)?;

    Ok(EncryptedPayload::new(nonce, ciphertext, kdf_metadata))
}

/// Decrypts an [`EncryptedPayload`] using AES-256-GCM with optional AAD validation.
pub fn decrypt_raw(
    key: &SecretKey,
    payload: &EncryptedPayload,
    aad: Option<&[u8]>,
) -> Result<Vec<u8>> {
    payload.validate_header()?;

    let cipher_nonce = aes_gcm::Nonce::from_slice(payload.nonce.as_bytes());
    let cipher =
        Aes256Gcm::new_from_slice(key.as_bytes()).map_err(|_| CryptoError::InvalidKeyLength {
            expected: 32,
            actual: key.as_bytes().len(),
        })?;

    let aead_payload = Payload {
        msg: &payload.ciphertext,
        aad: aad.unwrap_or(b""),
    };

    cipher
        .decrypt(cipher_nonce, aead_payload)
        .map_err(|_| CryptoError::AuthenticationFailed)
}

/// Encrypts a chunk of a file, cryptographically binding the [`ChunkAad`] context.
pub fn encrypt_chunk(
    key: &SecretKey,
    chunk_bytes: &[u8],
    aad: &ChunkAad,
    kdf_metadata: Option<KdfMetadata>,
) -> Result<EncryptedPayload> {
    let aad_bytes = aad.to_aad_bytes();
    encrypt_raw(key, chunk_bytes, Some(&aad_bytes), kdf_metadata)
}

/// Decrypts a chunk of a file, verifying ciphertext integrity and [`ChunkAad`] context.
///
/// If the chunk index, file ID, or total chunks do not match what was used during
/// encryption, authentication fails deterministically.
pub fn decrypt_chunk(
    key: &SecretKey,
    payload: &EncryptedPayload,
    aad: &ChunkAad,
) -> Result<Vec<u8>> {
    let aad_bytes = aad.to_aad_bytes();
    decrypt_raw(key, payload, Some(&aad_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use televault_core::ids::FileId;

    #[test]
    fn test_raw_encryption_roundtrip() {
        let key = SecretKey::generate();
        let message = b"Hello, TELEVAULT encrypted world!";

        let encrypted = encrypt_raw(&key, message, None, None).unwrap();
        assert_ne!(&encrypted.ciphertext, message);

        let decrypted = decrypt_raw(&key, &encrypted, None).unwrap();
        assert_eq!(&decrypted, message);
    }

    #[test]
    fn test_empty_payload_roundtrip() {
        let key = SecretKey::generate();
        let empty = b"";

        let encrypted = encrypt_raw(&key, empty, None, None).unwrap();
        let decrypted = decrypt_raw(&key, &encrypted, None).unwrap();
        assert_eq!(&decrypted, empty);
    }

    #[test]
    fn test_large_buffer_roundtrip() {
        let key = SecretKey::generate();
        let large_data = vec![0xAB; 256 * 1024]; // 256 KiB buffer

        let encrypted = encrypt_raw(&key, &large_data, None, None).unwrap();
        let decrypted = decrypt_raw(&key, &encrypted, None).unwrap();
        assert_eq!(decrypted, large_data);
    }

    #[test]
    fn test_chunk_encryption_roundtrip() {
        let key = SecretKey::generate();
        let fid = FileId::new("video-file-01").unwrap();
        let aad = ChunkAad::new(fid, 2, 10);
        let chunk_data = b"Chunk 2 data contents representing 1.8 GB slice";

        let encrypted = encrypt_chunk(&key, chunk_data, &aad, None).unwrap();
        let decrypted = decrypt_chunk(&key, &encrypted, &aad).unwrap();
        assert_eq!(&decrypted, chunk_data);
    }

    #[test]
    fn test_wrong_key_fails_authentication() {
        let key1 = SecretKey::generate();
        let key2 = SecretKey::generate();
        let data = b"Confidential document";

        let encrypted = encrypt_raw(&key1, data, None, None).unwrap();
        let result = decrypt_raw(&key2, &encrypted, None);
        assert_eq!(result, Err(CryptoError::AuthenticationFailed));
    }

    #[test]
    fn test_tampered_ciphertext_fails_authentication() {
        let key = SecretKey::generate();
        let data = b"Tamper test message";

        let mut encrypted = encrypt_raw(&key, data, None, None).unwrap();
        // Flip a bit in ciphertext
        if let Some(byte) = encrypted.ciphertext.first_mut() {
            *byte ^= 0x01;
        }

        let result = decrypt_raw(&key, &encrypted, None);
        assert_eq!(result, Err(CryptoError::AuthenticationFailed));
    }

    #[test]
    fn test_tampered_nonce_fails_authentication() {
        let key = SecretKey::generate();
        let data = b"Nonce tamper test message";

        let mut encrypted = encrypt_raw(&key, data, None, None).unwrap();
        let mut nonce_bytes = *encrypted.nonce.as_bytes();
        nonce_bytes[0] ^= 0xFF;
        encrypted.nonce = Nonce::from_bytes(nonce_bytes);

        let result = decrypt_raw(&key, &encrypted, None);
        assert_eq!(result, Err(CryptoError::AuthenticationFailed));
    }

    #[test]
    fn test_chunk_aad_tamper_fails_authentication() {
        let key = SecretKey::generate();
        let fid = FileId::new("file-safe-01").unwrap();
        let aad_orig = ChunkAad::new(fid.clone(), 0, 3);
        let chunk_data = b"First chunk data";

        let encrypted = encrypt_chunk(&key, chunk_data, &aad_orig, None).unwrap();

        // 1. Changing chunk index must fail
        let aad_swapped_index = ChunkAad::new(fid.clone(), 1, 3);
        assert_eq!(
            decrypt_chunk(&key, &encrypted, &aad_swapped_index),
            Err(CryptoError::AuthenticationFailed)
        );

        // 2. Changing file ID must fail
        let other_fid = FileId::new("file-safe-02").unwrap();
        let aad_wrong_file = ChunkAad::new(other_fid, 0, 3);
        assert_eq!(
            decrypt_chunk(&key, &encrypted, &aad_wrong_file),
            Err(CryptoError::AuthenticationFailed)
        );

        // 3. Changing total chunks must fail
        let aad_wrong_total = ChunkAad::new(fid, 0, 4);
        assert_eq!(
            decrypt_chunk(&key, &encrypted, &aad_wrong_total),
            Err(CryptoError::AuthenticationFailed)
        );
    }
}
