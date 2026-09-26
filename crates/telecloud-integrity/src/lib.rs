use sha2::{Digest, Sha256};
use std::path::Path;
use telecloud_core::{Result, TeleCloudError};
use telecloud_manifest::FileManifest;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

pub struct IntegrityVerifier;

impl IntegrityVerifier {
    pub async fn verify_file_sha256(path: &Path, expected_hex: &str) -> Result<bool> {
        let mut file = File::open(path).await.map_err(|e| TeleCloudError::Io {
            operation: "open_file_for_hash".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];

        loop {
            let n = file.read(&mut buffer).await.map_err(|e| TeleCloudError::Io {
                operation: "stream_read_for_hash".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        let computed = format!("{:x}", hasher.finalize());
        Ok(computed.eq_ignore_ascii_case(expected_hex))
    }

    pub fn verify_chunk_sha256(data: &[u8], expected_hex: &str) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let computed = format!("{:x}", hasher.finalize());
        computed.eq_ignore_ascii_case(expected_hex)
    }

    pub fn verify_manifest_structure(manifest: &FileManifest) -> Result<()> {
        manifest.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_hash_verification() {
        let data = b"TeleCloud chunk verification payload";
        let mut hasher = Sha256::new();
        hasher.update(data);
        let expected = format!("{:x}", hasher.finalize());

        assert!(IntegrityVerifier::verify_chunk_sha256(data, &expected));
        assert!(!IntegrityVerifier::verify_chunk_sha256(b"Tampered", &expected));
    }
}
