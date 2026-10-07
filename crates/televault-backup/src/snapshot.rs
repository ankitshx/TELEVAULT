//! Snapshot lifecycle management and comparison engine.

use crate::change_detector::{ChangeDetector, ChangeSet};
use crate::error::{BackupError, Result};
use crate::scanner::ScannedFile;
use std::sync::Arc;
use televault_core::ids::{ProfileId, SnapshotId};
use televault_core::models::BackupStatus;
use televault_db::{Database, FileRecord, SnapshotRecord, VersionRecord};

/// Snapshot coordinator managing persistent snapshots and version bindings.
pub struct SnapshotManager {
    db: Arc<Database>,
}

impl SnapshotManager {
    /// Creates a new `SnapshotManager` backed by embedded SQLite database.
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Initializes a new snapshot record in the database.
    pub fn create_snapshot(
        &self,
        profile_id: &ProfileId,
        snapshot_id: &SnapshotId,
        status: BackupStatus,
    ) -> Result<SnapshotRecord> {
        let record = SnapshotRecord {
            snapshot_id: snapshot_id.clone(),
            profile_id: profile_id.clone(),
            status,
            metadata: None,
            created_at: "2026-10-07T12:00:00Z".into(),
        };

        self.db
            .create_snapshot(&record)
            .map_err(BackupError::from)?;

        Ok(record)
    }

    /// Updates the status and metadata of an existing snapshot.
    pub fn update_snapshot_status(
        &self,
        snapshot_id: &SnapshotId,
        status: BackupStatus,
        metadata: Option<String>,
    ) -> Result<()> {
        self.db
            .update_snapshot_status_and_metadata(snapshot_id, status, metadata.as_deref())
            .map_err(BackupError::from)?;

        Ok(())
    }

    /// Retrieves the most recent snapshot for a profile.
    pub fn get_latest_snapshot(&self, profile_id: &ProfileId) -> Result<Option<SnapshotRecord>> {
        self.db
            .get_latest_snapshot(profile_id)
            .map_err(BackupError::from)
    }

    /// Retrieves all files and version records associated with a snapshot.
    pub fn list_snapshot_files(
        &self,
        snapshot_id: &SnapshotId,
    ) -> Result<Vec<(FileRecord, VersionRecord)>> {
        self.db
            .list_files_by_snapshot(snapshot_id)
            .map_err(BackupError::from)
    }

    /// Compares a newly scanned filesystem state against the latest snapshot of a profile.
    pub fn diff_against_latest_snapshot(
        &self,
        profile_id: &ProfileId,
        scanned_files: &[ScannedFile],
    ) -> Result<ChangeSet> {
        let latest_snap = self.get_latest_snapshot(profile_id)?;

        let previous_records = match latest_snap {
            Some(snap) => self.list_snapshot_files(&snap.snapshot_id)?,
            None => Vec::new(),
        };

        Ok(ChangeDetector::detect_changes(
            scanned_files,
            &previous_records,
        ))
    }

    /// Compares two historical snapshots to compute the differential change set between them.
    pub fn compare_snapshots(
        &self,
        earlier_snapshot_id: &SnapshotId,
        later_snapshot_id: &SnapshotId,
    ) -> Result<ChangeSet> {
        let earlier_records = self.list_snapshot_files(earlier_snapshot_id)?;
        let later_records = self.list_snapshot_files(later_snapshot_id)?;

        // Reconstruct ScannedFiles from later snapshot records
        let later_scanned: Vec<ScannedFile> = later_records
            .into_iter()
            .map(|(f, _v)| ScannedFile {
                relative_path: f.relative_path,
                absolute_path: std::path::PathBuf::from(""),
                file_name: f.file_name,
                size_bytes: f.original_size,
                modified_at_epoch_ms: 0,
                modified_at: f.modified_at.unwrap_or_default(),
                is_accessible: true,
            })
            .collect();

        Ok(ChangeDetector::detect_changes(
            &later_scanned,
            &earlier_records,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use televault_core::ids::ProfileId;

    #[test]
    fn test_snapshot_manager_lifecycle() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let profile = televault_db::models::ProfileRecord {
            profile_id: ProfileId::new("p-snap").unwrap(),
            name: "Snap Test".into(),
            description: None,
            source_path: "/data".into(),
            enabled: true,
            created_at: "2026-10-07T12:00:00Z".into(),
            updated_at: "2026-10-07T12:00:00Z".into(),
        };
        db.create_profile(&profile).unwrap();

        let manager = SnapshotManager::new(db);
        let s_id = SnapshotId::new("snap-001").unwrap();

        let created = manager
            .create_snapshot(&profile.profile_id, &s_id, BackupStatus::Scanning)
            .unwrap();
        assert_eq!(created.snapshot_id, s_id);
        assert_eq!(created.status, BackupStatus::Scanning);

        manager
            .update_snapshot_status(&s_id, BackupStatus::Completed, Some("{\"ok\":true}".into()))
            .unwrap();

        let latest = manager.get_latest_snapshot(&profile.profile_id).unwrap();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().status, BackupStatus::Completed);
    }
}
