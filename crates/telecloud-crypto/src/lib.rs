use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use argon2::{
    Algorithm, Argon2, Params, Version,
};
use rand::RngCore;
use sha2::{Digest, Sha256};
use telecloud_core::{Result, TeleCloudError};
use zeroize::Zeroize;

pub const SALT_LEN: usize = 16;
pub const KEY_LEN: usize = 32;
pub const NONCE_LEN: usize = 12;

/// Secure key derived using Argon2id with zeroization on drop.
pub struct DerivedKey(pub [u8; KEY_LEN]);

impl Drop for DerivedKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

pub fn generate_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    salt
}

pub fn generate_nonce() -> [u8; NONCE_LEN] {
    let mut nonce = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce);
    nonce
}

pub fn calculate_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex_encode(hasher.finalize())
}

pub fn derive_key(passphrase: &str, salt: &[u8; SALT_LEN]) -> Result<DerivedKey> {
    let params = Params::new(65536, 3, 2, Some(KEY_LEN)).map_err(|e| TeleCloudError::Crypto {
        operation: "argon2_params".to_string(),
        message: e.to_string(),
        recoverable: false,
    })?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; KEY_LEN];
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| TeleCloudError::Crypto {
            operation: "argon2_kdf".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

    Ok(DerivedKey(key))
}

pub fn encrypt_chunk(
    key: &DerivedKey,
    chunk_index: u32,
    plaintext: &[u8],
) -> Result<(Vec<u8>, [u8; NONCE_LEN])> {
    let cipher = Aes256Gcm::new_from_slice(&key.0).map_err(|e| TeleCloudError::Crypto {
        operation: "init_aes256gcm".to_string(),
        message: e.to_string(),
        recoverable: false,
    })?;

    let nonce_bytes = generate_nonce();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ad = chunk_index.to_be_bytes();
    let payload = Payload {
        msg: plaintext,
        aad: &ad,
    };

    let ciphertext = cipher.encrypt(nonce, payload).map_err(|e| TeleCloudError::Crypto {
        operation: "encrypt_chunk".to_string(),
        message: format!("Chunk {} encryption failed: {}", chunk_index, e),
        recoverable: false,
    })?;

    Ok((ciphertext, nonce_bytes))
}

pub fn decrypt_chunk(
    key: &DerivedKey,
    chunk_index: u32,
    nonce: &[u8; NONCE_LEN],
    ciphertext: &[u8],
) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(&key.0).map_err(|e| TeleCloudError::Crypto {
        operation: "init_aes256gcm".to_string(),
        message: e.to_string(),
        recoverable: false,
    })?;

    let nonce_ref = Nonce::from_slice(nonce);
    let ad = chunk_index.to_be_bytes();
    let payload = Payload {
        msg: ciphertext,
        aad: &ad,
    };

    let plaintext = cipher.decrypt(nonce_ref, payload).map_err(|e| TeleCloudError::Crypto {
        operation: "decrypt_chunk".to_string(),
        message: format!("Chunk {} decryption failed (corrupted or wrong key): {}", chunk_index, e),
        recoverable: false,
    })?;

    Ok(plaintext)
}

pub fn hex_encode(data: impl AsRef<[u8]>) -> String {
    data.as_ref()
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}

pub fn hex_decode(hex_str: &str) -> Result<Vec<u8>> {
    if hex_str.len() % 2 != 0 {
        return Err(TeleCloudError::Crypto {
            operation: "hex_decode".to_string(),
            message: "Odd length hex string".to_string(),
            recoverable: false,
        });
    }

    (0..hex_str.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex_str[i..i + 2], 16).map_err(|e| TeleCloudError::Crypto {
                operation: "hex_decode".to_string(),
                message: format!("Invalid hex byte: {}", e),
                recoverable: false,
            })
        })
        .collect()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crypto_round_trip() {
        let salt = generate_salt();
        let key = derive_key("master_password_123", &salt).unwrap();
        let chunk_index = 0;
        let data = b"Hello, TeleCloud secure storage!";

        let (ciphertext, nonce) = encrypt_chunk(&key, chunk_index, data).unwrap();
        assert_ne!(ciphertext, data);

        let decrypted = decrypt_chunk(&key, chunk_index, &nonce, &ciphertext).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_tamper_detection() {
        let salt = generate_salt();
        let key = derive_key("master_password_123", &salt).unwrap();
        let chunk_index = 0;
        let data = b"Sensitive Payload";

        let (mut ciphertext, nonce) = encrypt_chunk(&key, chunk_index, data).unwrap();
        // Tamper with one byte
        ciphertext[0] ^= 0xFF;

        let result = decrypt_chunk(&key, chunk_index, &nonce, &ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_chunk_index_associated_data_rejection() {
        let salt = generate_salt();
        let key = derive_key("master_password_123", &salt).unwrap();
        let (ciphertext, nonce) = encrypt_chunk(&key, 1, b"Payload for chunk 1").unwrap();

        // Attempting to decrypt with chunk index 2 should fail due to AD mismatch
        let result = decrypt_chunk(&key, 2, &nonce, &ciphertext);
        assert!(result.is_err());
    }
}
