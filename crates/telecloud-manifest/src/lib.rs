use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use telecloud_core::{Result, TeleCloudError};
use uuid::Uuid;

pub const CURRENT_MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestEncryptionConfig {
    pub algorithm: String,
    pub kdf: String,
    pub salt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestStorageObject {
    pub provider: String,
    pub chat_id: i64,
    pub object_id: i64,
    #[serde(default = "default_true")]
    pub is_primary: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestChunk {
    pub index: u32,
    pub offset: u64,
    pub size: u64,
    pub sha256: String,
    pub storage_objects: Vec<ManifestStorageObject>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileManifest {
    pub manifest_version: u32,
    pub file_id: Uuid,
    pub name: String,
    pub size: u64,
    pub full_sha256: String,
    pub mime_type: Option<String>,
    pub chunk_size: u64,
    pub total_chunks: u32,
    pub encryption: Option<ManifestEncryptionConfig>,
    pub chunks: Vec<ManifestChunk>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl FileManifest {
    pub fn new(
        file_id: Uuid,
        name: String,
        size: u64,
        full_sha256: String,
        chunk_size: u64,
        encryption: Option<ManifestEncryptionConfig>,
    ) -> Self {
        let total_chunks = if size == 0 {
            1
        } else {
            ((size + chunk_size - 1) / chunk_size) as u32
        };

        let mut chunks = Vec::with_capacity(total_chunks as usize);
        for idx in 0..total_chunks {
            let offset = (idx as u64) * chunk_size;
            let chunk_len = if idx == total_chunks - 1 {
                size - offset
            } else {
                chunk_size
            };
            chunks.push(ManifestChunk {
                index: idx,
                offset,
                size: chunk_len,
                sha256: String::new(),
                storage_objects: Vec::new(),
                status: "pending".to_string(),
            });
        }

        Self {
            manifest_version: CURRENT_MANIFEST_VERSION,
            file_id,
            name,
            size,
            full_sha256,
            mime_type: None,
            chunk_size,
            total_chunks,
            encryption,
            chunks,
            created_at: Utc::now(),
            updated_at: None,
        }
    }

    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| TeleCloudError::Manifest {
            operation: "serialize_manifest".to_string(),
            message: e.to_string(),
            recoverable: false,
        })
    }

    pub fn from_json(json: &str) -> Result<Self> {
        let manifest: Self = serde_json::from_str(json).map_err(|e| TeleCloudError::Manifest {
            operation: "deserialize_manifest".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        if manifest.manifest_version != CURRENT_MANIFEST_VERSION {
            return Err(TeleCloudError::Manifest {
                operation: "validate_manifest".to_string(),
                message: format!("Unsupported manifest version: {}", manifest.manifest_version),
                recoverable: false,
            });
        }

        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<()> {
        if self.chunks.len() != self.total_chunks as usize {
            return Err(TeleCloudError::Manifest {
                operation: "validate_chunks_count".to_string(),
                message: format!(
                    "Mismatch: expected {} chunks, got {}",
                    self.total_chunks,
                    self.chunks.len()
                ),
                recoverable: false,
            });
        }

        let computed_size: u64 = self.chunks.iter().map(|c| c.size).sum();
        if computed_size != self.size {
            return Err(TeleCloudError::Manifest {
                operation: "validate_total_size".to_string(),
                message: format!(
                    "Chunk sizes sum ({}) does not match total size ({})",
                    computed_size, self.size
                ),
                recoverable: false,
            });
        }

        Ok(())
    }
}
