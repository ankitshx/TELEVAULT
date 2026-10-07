//! Manifest schemas, serialization, and encrypted metadata contracts for TELEVAULT.

#![deny(missing_docs)]

pub mod chunk;
pub mod error;
pub mod manifest;
pub mod types;
pub mod version;

pub use chunk::{
    calculate_expected_chunk_count, ChunkManifest, CHUNK_THRESHOLD_BYTES, TARGET_CHUNK_SIZE_BYTES,
};
pub use error::{ManifestError, Result};
pub use manifest::{LogicalFileMetadata, ManifestV1};
pub use types::{
    CompressionMetadata, EncryptionMetadata, HashAlgorithm, IntegrityMetadata, KdfInfo,
    StorageReference,
};
pub use version::ManifestVersion;

/// Returns the numerical specification version (1 for v1).
pub fn manifest_version() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_version() {
        assert_eq!(manifest_version(), 1);
    }
}
