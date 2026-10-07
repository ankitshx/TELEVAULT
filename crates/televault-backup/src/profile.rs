//! Backup profile definition and configuration models.

use crate::error::{BackupError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use televault_core::ids::ProfileId;
use televault_core::models::CompressionAlgorithm;
use televault_crypto::policy::EncryptionPolicy;
use televault_db::ProfileRecord;
use televault_manifest::chunk::TARGET_CHUNK_SIZE_BYTES;

/// Extended profile configuration stored in profile metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProfileConfig {
    /// Include glob/filter patterns (e.g. `*.pdf`, `docs/*`).
    pub include_patterns: Vec<String>,
    /// Exclude glob/filter patterns (e.g. `*.tmp`, `node_modules/*`, `.git/*`).
    pub exclude_patterns: Vec<String>,
    /// Compression algorithm to apply.
    pub compression_algorithm: CompressionAlgorithm,
    /// Maximum chunk size in bytes (defaults to 1.8 GB).
    pub max_chunk_size_bytes: u64,
}

/// Fully configured backup profile specifying what and how to back up.
#[derive(Debug, Clone)]
pub struct BackupProfile {
    /// Unique profile identifier.
    pub profile_id: ProfileId,
    /// Human-readable profile name.
    pub name: String,
    /// Optional profile description.
    pub description: Option<String>,
    /// Root directory on the local filesystem to back up.
    pub source_path: PathBuf,
    /// Whether this profile is active for execution.
    pub enabled: bool,
    /// Include filter patterns.
    pub include_patterns: Vec<String>,
    /// Exclude filter patterns.
    pub exclude_patterns: Vec<String>,
    /// Application-level encryption policy (Enabled or Disabled).
    pub encryption_policy: EncryptionPolicy,
    /// Payload compression algorithm (None or Zstd).
    pub compression_algorithm: CompressionAlgorithm,
    /// Maximum physical chunk size in bytes.
    pub max_chunk_size_bytes: u64,
    /// Profile creation timestamp (ISO-8601).
    pub created_at: String,
    /// Profile update timestamp (ISO-8601).
    pub updated_at: String,
}

impl PartialEq for BackupProfile {
    fn eq(&self, other: &Self) -> bool {
        self.profile_id == other.profile_id
            && self.name == other.name
            && self.description == other.description
            && self.source_path == other.source_path
            && self.enabled == other.enabled
            && self.include_patterns == other.include_patterns
            && self.exclude_patterns == other.exclude_patterns
            && self.encryption_policy.is_enabled() == other.encryption_policy.is_enabled()
            && self.compression_algorithm == other.compression_algorithm
            && self.max_chunk_size_bytes == other.max_chunk_size_bytes
            && self.created_at == other.created_at
            && self.updated_at == other.updated_at
    }
}

impl BackupProfile {
    /// Creates a new backup profile with sensible defaults.
    pub fn new(profile_id: ProfileId, name: impl Into<String>, source_path: PathBuf) -> Self {
        Self {
            profile_id,
            name: name.into(),
            description: None,
            source_path,
            enabled: true,
            include_patterns: Vec::new(),
            exclude_patterns: vec![
                "*.tmp".into(),
                "*.temp".into(),
                "node_modules/*".into(),
                ".git/*".into(),
            ],
            encryption_policy: EncryptionPolicy::Disabled,
            compression_algorithm: CompressionAlgorithm::None,
            max_chunk_size_bytes: TARGET_CHUNK_SIZE_BYTES,
            created_at: "2026-10-07T12:00:00Z".into(),
            updated_at: "2026-10-07T12:00:00Z".into(),
        }
    }

    /// Sets the encryption policy for this profile.
    pub fn with_encryption_policy(mut self, policy: EncryptionPolicy) -> Self {
        self.encryption_policy = policy;
        self
    }

    /// Sets the compression algorithm for this profile.
    pub fn with_compression(mut self, algo: CompressionAlgorithm) -> Self {
        self.compression_algorithm = algo;
        self
    }

    /// Sets include patterns.
    pub fn with_include_patterns(mut self, patterns: Vec<String>) -> Self {
        self.include_patterns = patterns;
        self
    }

    /// Sets exclude patterns.
    pub fn with_exclude_patterns(mut self, patterns: Vec<String>) -> Self {
        self.exclude_patterns = patterns;
        self
    }

    /// Validates the profile invariants.
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(BackupError::InvalidProfile(
                "Profile name cannot be empty".into(),
            ));
        }
        if self.source_path.as_os_str().is_empty() {
            return Err(BackupError::InvalidProfile(
                "Source path cannot be empty".into(),
            ));
        }
        if self.max_chunk_size_bytes == 0 {
            return Err(BackupError::InvalidProfile(
                "max_chunk_size_bytes must be greater than zero".into(),
            ));
        }
        Ok(())
    }

    /// Determines whether a relative file path matches the profile's include/exclude filter rules.
    pub fn is_path_included(&self, relative_path: &str) -> bool {
        let normalized = relative_path.replace('\\', "/");

        // 1. Check exclude patterns first
        for pat in &self.exclude_patterns {
            if matches_pattern(&normalized, pat) {
                return false;
            }
        }

        // 2. Check include patterns if specified
        if !self.include_patterns.is_empty() {
            return self
                .include_patterns
                .iter()
                .any(|pat| matches_pattern(&normalized, pat));
        }

        true
    }

    /// Converts this profile into a database `ProfileRecord`.
    pub fn to_db_record(&self) -> ProfileRecord {
        ProfileRecord {
            profile_id: self.profile_id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            source_path: self.source_path.to_string_lossy().to_string(),
            enabled: self.enabled,
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }

    /// Reconstructs a `BackupProfile` from a database record and optional extended configuration.
    pub fn from_db_record(
        record: &ProfileRecord,
        config: Option<ProfileConfig>,
        encryption: EncryptionPolicy,
    ) -> Self {
        let cfg = config.unwrap_or_default();
        Self {
            profile_id: record.profile_id.clone(),
            name: record.name.clone(),
            description: record.description.clone(),
            source_path: PathBuf::from(&record.source_path),
            enabled: record.enabled,
            include_patterns: cfg.include_patterns,
            exclude_patterns: cfg.exclude_patterns,
            encryption_policy: encryption,
            compression_algorithm: cfg.compression_algorithm,
            max_chunk_size_bytes: if cfg.max_chunk_size_bytes > 0 {
                cfg.max_chunk_size_bytes
            } else {
                TARGET_CHUNK_SIZE_BYTES
            },
            created_at: record.created_at.clone(),
            updated_at: record.updated_at.clone(),
        }
    }
}

/// Evaluates whether a relative path matches a simple wildcard filter pattern.
///
/// Supports:
/// - `*.ext` (extension match)
/// - `prefix*` (prefix match)
/// - `dir/*` (directory contents match)
/// - Exact matches
fn matches_pattern(path: &str, pattern: &str) -> bool {
    let pat = pattern.replace('\\', "/");

    if pat == "*" {
        return true;
    }

    if let Some(suffix) = pat.strip_prefix('*') {
        return path.ends_with(suffix);
    }

    if let Some(prefix) = pat.strip_suffix('*') {
        return path.starts_with(prefix);
    }

    path == pat
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_filtering() {
        let profile = BackupProfile::new(
            ProfileId::new("p1").unwrap(),
            "Docs",
            PathBuf::from("C:\\Data"),
        )
        .with_include_patterns(vec!["*.pdf".into(), "reports/*".into()])
        .with_exclude_patterns(vec!["*.tmp".into(), "reports/drafts/*".into()]);

        assert!(profile.is_path_included("financial.pdf"));
        assert!(profile.is_path_included("reports/q1.docx"));
        assert!(!profile.is_path_included("financial.tmp"));
        assert!(!profile.is_path_included("reports/drafts/notes.docx"));
        assert!(!profile.is_path_included("random.png")); // Not included in include list
    }

    #[test]
    fn test_profile_validation() {
        let valid = BackupProfile::new(
            ProfileId::new("p-ok").unwrap(),
            "Valid",
            PathBuf::from("/home/user/docs"),
        );
        assert!(valid.validate().is_ok());

        let empty_name = BackupProfile::new(
            ProfileId::new("p-empty").unwrap(),
            "   ",
            PathBuf::from("/home/user/docs"),
        );
        assert!(empty_name.validate().is_err());
    }
}
