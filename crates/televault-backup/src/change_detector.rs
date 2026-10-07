//! Fast metadata-first incremental change detection engine.

use crate::scanner::ScannedFile;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use televault_core::ids::FileId;
use televault_db::{FileRecord, VersionRecord};

/// Specific classification of file state transition between snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// Newly added file not present in previous snapshot.
    New,
    /// Existing file whose size or timestamp has changed.
    Modified,
    /// Existing file identical to previous snapshot; requires zero transfer.
    Unchanged,
    /// File previously backed up but missing on local filesystem.
    Deleted,
}

/// Fully described file change entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    /// Change category.
    pub kind: ChangeKind,
    /// Authoritative logical file identifier.
    pub file_id: FileId,
    /// Relative path within the backup profile root.
    pub relative_path: String,
    /// Scanned file metadata (present for New, Modified, and Unchanged).
    pub scanned: Option<ScannedFile>,
    /// Previous persistent file record, if known.
    pub previous_file: Option<FileRecord>,
    /// Previous persistent version record, if known.
    pub previous_version: Option<VersionRecord>,
}

/// Comprehensive change set resulting from snapshot comparison.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChangeSet {
    /// All detected file changes, ordered deterministically.
    pub changes: Vec<FileChange>,
}

impl ChangeSet {
    /// Returns files classified as newly added.
    pub fn new_files(&self) -> Vec<&FileChange> {
        self.changes
            .iter()
            .filter(|c| c.kind == ChangeKind::New)
            .collect()
    }

    /// Returns files classified as modified.
    pub fn modified_files(&self) -> Vec<&FileChange> {
        self.changes
            .iter()
            .filter(|c| c.kind == ChangeKind::Modified)
            .collect()
    }

    /// Returns files classified as unchanged.
    pub fn unchanged_files(&self) -> Vec<&FileChange> {
        self.changes
            .iter()
            .filter(|c| c.kind == ChangeKind::Unchanged)
            .collect()
    }

    /// Returns files classified as deleted.
    pub fn deleted_files(&self) -> Vec<&FileChange> {
        self.changes
            .iter()
            .filter(|c| c.kind == ChangeKind::Deleted)
            .collect()
    }

    /// Returns whether any mutations (new, modified, or deleted) exist.
    pub fn has_mutations(&self) -> bool {
        self.changes.iter().any(|c| c.kind != ChangeKind::Unchanged)
    }

    /// Returns total count of files that require payload processing and transfer.
    pub fn count_transfers_needed(&self) -> usize {
        self.changes
            .iter()
            .filter(|c| c.kind == ChangeKind::New || c.kind == ChangeKind::Modified)
            .count()
    }

    /// Aggregate byte size of all files requiring upload.
    pub fn total_transfer_bytes(&self) -> u64 {
        self.changes
            .iter()
            .filter(|c| c.kind == ChangeKind::New || c.kind == ChangeKind::Modified)
            .filter_map(|c| c.scanned.as_ref().map(|s| s.size_bytes))
            .sum()
    }

    /// Aggregate byte size of all unchanged files safely reused without transfer.
    pub fn total_reused_bytes(&self) -> u64 {
        self.changes
            .iter()
            .filter(|c| c.kind == ChangeKind::Unchanged)
            .filter_map(|c| c.scanned.as_ref().map(|s| s.size_bytes))
            .sum()
    }
}

/// Change detection engine comparing scanned files against previous catalog records.
pub struct ChangeDetector;

impl ChangeDetector {
    /// Compares scanned filesystem entries against previous file and version records.
    pub fn detect_changes(
        scanned_files: &[ScannedFile],
        previous_records: &[(FileRecord, VersionRecord)],
    ) -> ChangeSet {
        let mut prev_by_path: HashMap<String, (&FileRecord, &VersionRecord)> = HashMap::new();
        for (file, ver) in previous_records {
            prev_by_path.insert(file.relative_path.clone(), (file, ver));
        }

        let mut changes = Vec::new();
        let mut seen_paths = std::collections::HashSet::new();

        // 1. Process current scanned files (New, Modified, Unchanged)
        for scanned in scanned_files {
            seen_paths.insert(scanned.relative_path.clone());

            if let Some(&(prev_file, prev_ver)) = prev_by_path.get(&scanned.relative_path) {
                // Check if unchanged using size and modification timestamp
                let size_matches = scanned.size_bytes == prev_file.original_size;
                let mtime_matches = prev_file
                    .modified_at
                    .as_deref()
                    .map(|prev_m| prev_m == scanned.modified_at)
                    .unwrap_or(false);

                let kind = if size_matches && mtime_matches {
                    ChangeKind::Unchanged
                } else {
                    ChangeKind::Modified
                };

                changes.push(FileChange {
                    kind,
                    file_id: prev_file.file_id.clone(),
                    relative_path: scanned.relative_path.clone(),
                    scanned: Some(scanned.clone()),
                    previous_file: Some(prev_file.clone()),
                    previous_version: Some(prev_ver.clone()),
                });
            } else {
                // Newly added file -> generate deterministic FileId based on path hash
                let file_id = Self::deterministic_file_id(&scanned.relative_path);
                changes.push(FileChange {
                    kind: ChangeKind::New,
                    file_id,
                    relative_path: scanned.relative_path.clone(),
                    scanned: Some(scanned.clone()),
                    previous_file: None,
                    previous_version: None,
                });
            }
        }

        // 2. Identify deleted files (present in previous snapshot, missing now)
        for (prev_file, prev_ver) in previous_records {
            if !seen_paths.contains(&prev_file.relative_path) {
                changes.push(FileChange {
                    kind: ChangeKind::Deleted,
                    file_id: prev_file.file_id.clone(),
                    relative_path: prev_file.relative_path.clone(),
                    scanned: None,
                    previous_file: Some(prev_file.clone()),
                    previous_version: Some(prev_ver.clone()),
                });
            }
        }

        // Deterministic ordering by relative path
        changes.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

        ChangeSet { changes }
    }

    /// Computes a stable, deterministic `FileId` from relative path for newly discovered files.
    pub fn deterministic_file_id(relative_path: &str) -> FileId {
        let mut hasher = Sha256::new();
        hasher.update(relative_path.as_bytes());
        let digest = format!("{:x}", hasher.finalize());
        let id_str = format!("file-{}", &digest[..16]);
        FileId::new(id_str).unwrap_or_else(|_| FileId::new("file-default").unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use televault_core::ids::{SnapshotId, VersionId};

    #[test]
    fn test_change_detection_all_states() {
        let prev_f1 = FileRecord {
            file_id: FileId::new("file-1").unwrap(),
            profile_id: None,
            file_name: "unchanged.txt".into(),
            relative_path: "unchanged.txt".into(),
            original_size: 100,
            mime_type: None,
            status: "active".into(),
            logical_file_hash: None,
            created_at: "2026-10-07T12:00:00Z".into(),
            modified_at: Some("epoch-ms:1000".into()),
            created_timestamp: "2026-10-07T12:00:00Z".into(),
            updated_timestamp: "2026-10-07T12:00:00Z".into(),
        };
        let prev_v1 = VersionRecord {
            version_id: VersionId::new("ver-1").unwrap(),
            file_id: prev_f1.file_id.clone(),
            snapshot_id: SnapshotId::new("snap-prev").unwrap(),
            manifest_id: "man-1".into(),
            status: "active".into(),
            created_at: "2026-10-07T12:00:00Z".into(),
        };

        let prev_f2 = FileRecord {
            file_id: FileId::new("file-2").unwrap(),
            profile_id: None,
            file_name: "modified.txt".into(),
            relative_path: "modified.txt".into(),
            original_size: 200,
            mime_type: None,
            status: "active".into(),
            logical_file_hash: None,
            created_at: "2026-10-07T12:00:00Z".into(),
            modified_at: Some("epoch-ms:2000".into()),
            created_timestamp: "2026-10-07T12:00:00Z".into(),
            updated_timestamp: "2026-10-07T12:00:00Z".into(),
        };
        let prev_v2 = VersionRecord {
            version_id: VersionId::new("ver-2").unwrap(),
            file_id: prev_f2.file_id.clone(),
            snapshot_id: SnapshotId::new("snap-prev").unwrap(),
            manifest_id: "man-2".into(),
            status: "active".into(),
            created_at: "2026-10-07T12:00:00Z".into(),
        };

        let prev_f3 = FileRecord {
            file_id: FileId::new("file-3").unwrap(),
            profile_id: None,
            file_name: "deleted.txt".into(),
            relative_path: "deleted.txt".into(),
            original_size: 300,
            mime_type: None,
            status: "active".into(),
            logical_file_hash: None,
            created_at: "2026-10-07T12:00:00Z".into(),
            modified_at: Some("epoch-ms:3000".into()),
            created_timestamp: "2026-10-07T12:00:00Z".into(),
            updated_timestamp: "2026-10-07T12:00:00Z".into(),
        };
        let prev_v3 = VersionRecord {
            version_id: VersionId::new("ver-3").unwrap(),
            file_id: prev_f3.file_id.clone(),
            snapshot_id: SnapshotId::new("snap-prev").unwrap(),
            manifest_id: "man-3".into(),
            status: "active".into(),
            created_at: "2026-10-07T12:00:00Z".into(),
        };

        let previous = vec![(prev_f1, prev_v1), (prev_f2, prev_v2), (prev_f3, prev_v3)];

        let scanned = vec![
            // Unchanged: same size and mtime
            ScannedFile {
                relative_path: "unchanged.txt".into(),
                absolute_path: PathBuf::from("/data/unchanged.txt"),
                file_name: "unchanged.txt".into(),
                size_bytes: 100,
                modified_at_epoch_ms: 1000,
                modified_at: "epoch-ms:1000".into(),
                is_accessible: true,
            },
            // Modified: size increased to 250
            ScannedFile {
                relative_path: "modified.txt".into(),
                absolute_path: PathBuf::from("/data/modified.txt"),
                file_name: "modified.txt".into(),
                size_bytes: 250,
                modified_at_epoch_ms: 2500,
                modified_at: "epoch-ms:2500".into(),
                is_accessible: true,
            },
            // New file
            ScannedFile {
                relative_path: "new_file.txt".into(),
                absolute_path: PathBuf::from("/data/new_file.txt"),
                file_name: "new_file.txt".into(),
                size_bytes: 500,
                modified_at_epoch_ms: 5000,
                modified_at: "epoch-ms:5000".into(),
                is_accessible: true,
            },
        ];

        let cset = ChangeDetector::detect_changes(&scanned, &previous);

        assert_eq!(cset.unchanged_files().len(), 1);
        assert_eq!(cset.modified_files().len(), 1);
        assert_eq!(cset.new_files().len(), 1);
        assert_eq!(cset.deleted_files().len(), 1);

        assert_eq!(cset.unchanged_files()[0].file_id.as_str(), "file-1");
        assert_eq!(cset.modified_files()[0].file_id.as_str(), "file-2");
        assert_eq!(cset.deleted_files()[0].file_id.as_str(), "file-3");

        assert_eq!(cset.total_transfer_bytes(), 250 + 500);
        assert_eq!(cset.total_reused_bytes(), 100);
    }
}
