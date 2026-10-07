//! Chunk-aware Authenticated Associated Data (AAD) binding context.

use televault_core::ids::FileId;

/// Contextual metadata bound cryptographically to chunk ciphertext via AAD.
///
/// Prevents:
/// - Cross-file chunk replay (moving a chunk from file A to file B)
/// - Chunk index reordering (swapping chunk 1 and chunk 2)
/// - Truncation / splicing attacks (manipulating total chunk count)
/// - Format version confusion
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkAad {
    /// Identifier of the logical file this chunk belongs to.
    pub file_id: FileId,
    /// 0-indexed position of this chunk within the file.
    pub chunk_index: u32,
    /// Total number of chunks comprising the file.
    pub total_chunks: u32,
    /// Cryptographic specification version.
    pub format_version: u32,
}

impl ChunkAad {
    /// Creates a new [`ChunkAad`] binding.
    pub fn new(file_id: FileId, chunk_index: u32, total_chunks: u32) -> Self {
        Self {
            file_id,
            chunk_index,
            total_chunks,
            format_version: 1,
        }
    }

    /// Serializes this context into canonical, unambiguous byte format for AAD binding.
    ///
    /// Layout:
    /// - Magic prefix: `b"TELEVAULT-CHUNK-v1\0"`
    /// - file_id length (u16 little-endian)
    /// - file_id bytes (UTF-8)
    /// - chunk_index (u32 little-endian)
    /// - total_chunks (u32 little-endian)
    /// - format_version (u32 little-endian)
    pub fn to_aad_bytes(&self) -> Vec<u8> {
        let magic = b"TELEVAULT-CHUNK-v1\0";
        let file_id_str = self.file_id.as_str();
        let file_id_bytes = file_id_str.as_bytes();
        let file_id_len = file_id_bytes.len() as u16;

        let mut bytes = Vec::with_capacity(magic.len() + 2 + file_id_bytes.len() + 12);
        bytes.extend_from_slice(magic);
        bytes.extend_from_slice(&file_id_len.to_le_bytes());
        bytes.extend_from_slice(file_id_bytes);
        bytes.extend_from_slice(&self.chunk_index.to_le_bytes());
        bytes.extend_from_slice(&self.total_chunks.to_le_bytes());
        bytes.extend_from_slice(&self.format_version.to_le_bytes());

        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_aad_determinism() {
        let fid = FileId::new("file-abc-123").unwrap();
        let aad1 = ChunkAad::new(fid.clone(), 0, 5);
        let aad2 = ChunkAad::new(fid.clone(), 0, 5);

        assert_eq!(aad1.to_aad_bytes(), aad2.to_aad_bytes());
    }

    #[test]
    fn test_chunk_aad_distinguishes_different_indices() {
        let fid = FileId::new("file-abc-123").unwrap();
        let aad0 = ChunkAad::new(fid.clone(), 0, 5);
        let aad1 = ChunkAad::new(fid.clone(), 1, 5);

        assert_ne!(aad0.to_aad_bytes(), aad1.to_aad_bytes());
    }

    #[test]
    fn test_chunk_aad_distinguishes_different_files() {
        let fid1 = FileId::new("file-01").unwrap();
        let fid2 = FileId::new("file-02").unwrap();
        let aad1 = ChunkAad::new(fid1, 0, 5);
        let aad2 = ChunkAad::new(fid2, 0, 5);

        assert_ne!(aad1.to_aad_bytes(), aad2.to_aad_bytes());
    }

    #[test]
    fn test_chunk_aad_distinguishes_different_totals() {
        let fid = FileId::new("file-01").unwrap();
        let aad1 = ChunkAad::new(fid.clone(), 0, 5);
        let aad2 = ChunkAad::new(fid, 0, 6);

        assert_ne!(aad1.to_aad_bytes(), aad2.to_aad_bytes());
    }
}
