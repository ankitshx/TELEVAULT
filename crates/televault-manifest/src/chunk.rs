//! Physical storage chunk metadata and chunking calculation contracts.

use crate::error::Result;
use crate::types::{IntegrityMetadata, StorageReference};
use serde::{Deserialize, Serialize};
use televault_core::ids::ChunkId;

/// Maximum size for a single-chunk file before chunking is required (2 GB = 2,147,483,648 bytes).
pub const CHUNK_THRESHOLD_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Target size for split chunks when a file is >= 2 GB (1.8 GB = 1,887,436,800 bytes).
pub const TARGET_CHUNK_SIZE_BYTES: u64 = 1_800 * 1024 * 1024;

/// Calculates the expected number of chunks for a given logical file size.
///
/// Rules:
/// - Empty file (0 bytes) -> 1 chunk (0 bytes).
/// - Below 2 GB (< 2,147,483,648 bytes) -> 1 chunk.
/// - At or above 2 GB (>= 2,147,483,648 bytes) -> split into ~1.8 GB chunks.
pub fn calculate_expected_chunk_count(file_size: u64) -> u32 {
    if file_size < CHUNK_THRESHOLD_BYTES {
        1
    } else {
        let count = file_size.div_ceil(TARGET_CHUNK_SIZE_BYTES);
        count as u32
    }
}

/// Metadata for an individual physical storage chunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkManifest {
    /// Unique identifier for this physical chunk.
    pub chunk_id: ChunkId,
    /// Zero-based sequential index (0, 1, ..., N-1).
    pub index: u32,
    /// Plaintext uncompressed, unencrypted size of this chunk in bytes.
    pub plaintext_size: u64,
    /// Final stored size in bytes after optional compression and optional encryption.
    pub stored_size: u64,
    /// Cryptographic integrity hash of this physical chunk payload.
    pub integrity: IntegrityMetadata,
    /// Authoritative storage reference indicating where this chunk resides.
    pub storage_reference: StorageReference,
}

impl ChunkManifest {
    /// Validates chunk metadata invariants.
    pub fn validate(&self) -> Result<()> {
        self.integrity.validate()?;
        self.storage_reference.validate()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_count_calculation() {
        // Empty file -> 1 chunk
        assert_eq!(calculate_expected_chunk_count(0), 1);

        // 100 MB -> 1 chunk
        assert_eq!(calculate_expected_chunk_count(100 * 1024 * 1024), 1);

        // 1.99 GB (< 2 GB) -> 1 chunk
        assert_eq!(calculate_expected_chunk_count(CHUNK_THRESHOLD_BYTES - 1), 1);

        // Exactly 2 GB -> split into 1.8 GB chunks: 2 GB / 1.8 GB = 2 chunks
        assert_eq!(calculate_expected_chunk_count(CHUNK_THRESHOLD_BYTES), 2);

        // 5.2 GB (5,583,457,484 bytes):
        // 5.2 GB / 1.8 GB = 3 chunks (1.8 GB, 1.8 GB, 1.6 GB)
        let five_point_two_gb = (5.2 * 1024.0 * 1024.0 * 1024.0) as u64;
        assert_eq!(calculate_expected_chunk_count(five_point_two_gb), 3);
    }
}
