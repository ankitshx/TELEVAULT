//! High-level contracts for Telegram cloud storage operations and configuration.

use crate::error::{Result, TelegramError};
use serde::{Deserialize, Serialize};
use televault_core::ids::{ChunkId, FileId};

/// Non-sensitive operational configuration for Telegram remote storage.
///
/// Adheres strictly to AD-007: API hashes, session tokens, passwords, and private keys
/// are FORBIDDEN in application configuration structs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelegramStorageConfig {
    /// Target chat or channel identifier where backups are stored (e.g. Saved Messages or private channel).
    pub target_chat_id: i64,
    /// Timeout in seconds for an individual 1.8 GB chunk streaming upload.
    pub chunk_upload_timeout_secs: u64,
    /// Maximum retry attempts before marking a chunk transfer as failed.
    pub max_retries: u32,
}

impl Default for TelegramStorageConfig {
    fn default() -> Self {
        Self {
            target_chat_id: 0,
            chunk_upload_timeout_secs: 300,
            max_retries: 3,
        }
    }
}

impl TelegramStorageConfig {
    /// Validates configuration invariants.
    pub fn validate(&self) -> Result<()> {
        if self.target_chat_id == 0 {
            return Err(TelegramError::InvalidReference(
                "target_chat_id cannot be 0".into(),
            ));
        }
        if self.chunk_upload_timeout_secs == 0 {
            return Err(TelegramError::Transport(
                "chunk_upload_timeout_secs must be positive".into(),
            ));
        }
        Ok(())
    }
}

/// Metadata payload extracted from an uploaded Telegram document message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelegramDocumentMessage {
    /// Telegram chat identifier.
    pub chat_id: i64,
    /// Message identifier.
    pub message_id: i64,
    /// Telegram file identifier.
    pub file_id: String,
    /// Confirmed document byte size.
    pub size_bytes: u64,
    /// Document MIME type string.
    pub mime_type: Option<String>,
    /// Document file name as transmitted to Telegram.
    pub file_name: Option<String>,
    /// Message text or caption attached to the document.
    pub caption: Option<String>,
}

/// Structured caption header tagging uploaded Telegram document chunks.
///
/// Enables disaster recovery scanning and remote verification directly from
/// Telegram channel messages without needing an external catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelegramChunkHeader {
    /// Specification version (1).
    pub format_version: u32,
    /// Associated logical file identifier.
    pub file_id: FileId,
    /// Physical chunk identifier.
    pub chunk_id: ChunkId,
    /// Zero-based chunk index.
    pub chunk_index: u32,
    /// Total chunks in logical file.
    pub total_chunks: u32,
    /// Exact byte size.
    pub size_bytes: u64,
}

impl TelegramChunkHeader {
    /// Encodes this header into a clean Telegram message caption.
    pub fn to_caption(&self) -> String {
        format!(
            "TELEVAULT:v={}:fid={}:cid={}:cidx={}:tot={}:sz={}",
            self.format_version,
            self.file_id.as_str(),
            self.chunk_id.as_str(),
            self.chunk_index,
            self.total_chunks,
            self.size_bytes
        )
    }

    /// Parses a Telegram message caption into structured chunk metadata.
    pub fn parse_caption(caption: &str) -> Option<Self> {
        let trimmed = caption.trim();
        if !trimmed.starts_with("TELEVAULT:") {
            return None;
        }

        let parts: Vec<&str> = trimmed.split(':').collect();
        if parts.len() != 7 {
            return None;
        }

        let mut v = 1;
        let mut fid = None;
        let mut cid = None;
        let mut cidx = None;
        let mut tot = None;
        let mut sz = None;

        for part in &parts[1..] {
            if let Some(val) = part.strip_prefix("v=") {
                v = val.parse().ok()?;
            } else if let Some(val) = part.strip_prefix("fid=") {
                fid = Some(FileId::new(val).ok()?);
            } else if let Some(val) = part.strip_prefix("cid=") {
                cid = Some(ChunkId::new(val).ok()?);
            } else if let Some(val) = part.strip_prefix("cidx=") {
                cidx = Some(val.parse::<u32>().ok()?);
            } else if let Some(val) = part.strip_prefix("tot=") {
                tot = Some(val.parse::<u32>().ok()?);
            } else if let Some(val) = part.strip_prefix("sz=") {
                sz = Some(val.parse::<u64>().ok()?);
            }
        }

        Some(Self {
            format_version: v,
            file_id: fid?,
            chunk_id: cid?,
            chunk_index: cidx?,
            total_chunks: tot?,
            size_bytes: sz?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_header_caption_roundtrip() {
        let header = TelegramChunkHeader {
            format_version: 1,
            file_id: FileId::new("file-video-01").unwrap(),
            chunk_id: ChunkId::new("chunk-01").unwrap(),
            chunk_index: 0,
            total_chunks: 3,
            size_bytes: 1_887_436_800,
        };

        let caption = header.to_caption();
        assert!(caption.starts_with("TELEVAULT:"));

        let parsed = TelegramChunkHeader::parse_caption(&caption).expect("parse caption");
        assert_eq!(parsed, header);
    }
}
