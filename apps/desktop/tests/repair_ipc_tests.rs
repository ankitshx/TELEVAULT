//! Comprehensive IPC integration tests for remote repair, preview, history, and concurrency guards.

use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tauri::Manager;
use televault_core::ids::{ChunkId, FileId, ProfileId, SnapshotId, VersionId};
use televault_core::models::BackupStatus;
use televault_db::{ChunkRecord, FileRecord, SnapshotRecord, VersionRecord};
use televault_desktop::commands::backup::*;
use televault_desktop::commands::repair::*;
use televault_desktop::dto::*;
use televault_desktop::state::DesktopAppState;
use televault_integrity::hasher::StreamHasher;
use televault_manifest::chunk::ChunkManifest;
use televault_manifest::{
    CompressionMetadata, IntegrityMetadata, LogicalFileMetadata, ManifestV1, ManifestVersion,
    StorageReference,
};

fn setup_test_state() -> (tauri::App<tauri::test::MockRuntime>, PathBuf) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("televault_repair_ipc_test_{nanos}"));
    fs::create_dir_all(&temp_dir).unwrap();

    let state = DesktopAppState::new_in_memory();
    let app = tauri::test::mock_app();
    app.manage(state);
    (app, temp_dir)
}

fn create_test_profile(
    state: tauri::State<'_, DesktopAppState>,
    temp_dir: &std::path::Path,
    id: &str,
) -> BackupProfileDto {
    let source = temp_dir.join(format!("src_{id}"));
    fs::create_dir_all(&source).unwrap();

    create_backup_profile(
        state,
        CreateProfileRequest {
            profile_id: id.to_string(),
            name: format!("Profile {id}"),
            description: Some("Test profile for repair IPC".to_string()),
            source_path: source.to_string_lossy().to_string(),
        },
    )
    .expect("create profile")
}

#[tokio::test]
async fn test_ipc_repair_file_preview_and_execution() {
    let (app, temp_dir) = setup_test_state();
    let state = app.state::<DesktopAppState>();

    let profile = create_test_profile(state.clone(), &temp_dir, "prof-r-01");
    let pid = ProfileId::new(&profile.profile_id).unwrap();
    let fid = FileId::new("file-r-01").unwrap();

    let source_dir = PathBuf::from(&profile.source_path);
    let src_file = source_dir.join("document.pdf");
    let content = b"Authoritative content for repair IPC test file!";
    {
        let mut f = File::create(&src_file).unwrap();
        f.write_all(content).unwrap();
    }

    let file_hash = StreamHasher::hash_bytes(content);
    state
        .db
        .create_file(&FileRecord {
            file_id: fid.clone(),
            profile_id: Some(pid.clone()),
            file_name: "document.pdf".into(),
            relative_path: "document.pdf".into(),
            original_size: content.len() as u64,
            mime_type: None,
            status: "backed_up".into(),
            logical_file_hash: Some(file_hash.clone()),
            created_at: "2026-10-08T10:00:00Z".into(),
            modified_at: None,
            created_timestamp: "2026-10-08T10:00:00Z".into(),
            updated_timestamp: "2026-10-08T10:00:00Z".into(),
        })
        .unwrap();

    let cid = ChunkId::new("chk-r-01").unwrap();
    let old_sref = StorageReference::Telegram {
        chat_id: -1001234567890,
        message_id: 8888,
        file_id: "tg_file_8888".into(),
    };

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-r-01".into(),
        logical_file: LogicalFileMetadata {
            file_id: fid.clone(),
            file_name: "document.pdf".into(),
            relative_path: "document.pdf".into(),
            original_size: content.len() as u64,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        integrity: IntegrityMetadata::sha256(&file_hash),
        chunks: vec![ChunkManifest {
            chunk_id: cid.clone(),
            index: 0,
            plaintext_size: content.len() as u64,
            stored_size: content.len() as u64,
            integrity: IntegrityMetadata::sha256(&file_hash),
            storage_reference: old_sref.clone(),
        }],
    };
    state.db.save_manifest(&manifest).unwrap();

    state
        .db
        .create_chunk(&ChunkRecord {
            chunk_id: cid.clone(),
            file_id: fid.clone(),
            manifest_id: "man-r-01".into(),
            chunk_index: 0,
            plaintext_size: content.len() as u64,
            stored_size: content.len() as u64,
            integrity_hash: file_hash.clone(),
            storage_reference: serde_json::to_string(&old_sref).unwrap(),
            status: "active".into(),
            created_at: "2026-10-08T10:00:00Z".into(),
            updated_at: "2026-10-08T10:00:00Z".into(),
        })
        .unwrap();

    let snap_id = SnapshotId::new("snap-r-01").unwrap();
    state
        .db
        .create_snapshot(&SnapshotRecord {
            snapshot_id: snap_id.clone(),
            profile_id: pid.clone(),
            status: BackupStatus::Completed,
            metadata: None,
            created_at: "2026-10-08T10:00:00Z".into(),
        })
        .unwrap();

    state
        .db
        .create_version(&VersionRecord {
            version_id: VersionId::new("ver-r-01").unwrap(),
            file_id: fid.clone(),
            snapshot_id: snap_id,
            manifest_id: "man-r-01".into(),
            status: "active".into(),
            created_at: "2026-10-08T10:00:00Z".into(),
        })
        .unwrap();

    // The remote chunk is MISSING in mock storage (never inserted)
    // 1. Preview repair via IPC
    let preview_req = PreviewRepairRequest {
        profile_id: pid.to_string(),
        target_type: "file".into(),
        target_id: fid.to_string(),
        operation_id: None,
        passphrase: None,
    };
    let preview = preview_repair(state.clone(), preview_req).await.unwrap();
    assert!(preview.is_repairable);
    assert_eq!(preview.total_affected_chunks, 1);
    assert_eq!(preview.eligible_candidates.len(), 1);

    // 2. Execute dry run via repair_file IPC
    let dry_req = RepairFileRequest {
        profile_id: pid.to_string(),
        file_id: fid.to_string(),
        dry_run: Some(true),
        operation_id: None,
        passphrase: None,
    };
    let dry_res = repair_file(state.clone(), dry_req).await.unwrap();
    assert_eq!(dry_res.repaired_chunks, 0);
    assert_eq!(dry_res.chunk_results[0].status, "dry_run");

    // 3. Execute live repair via repair_file IPC
    let live_req = RepairFileRequest {
        profile_id: pid.to_string(),
        file_id: fid.to_string(),
        dry_run: Some(false),
        operation_id: None,
        passphrase: None,
    };
    let live_res = repair_file(state.clone(), live_req).await.unwrap();
    assert_eq!(live_res.repaired_chunks, 1);
    assert_eq!(live_res.failed_chunks, 0);
    assert_eq!(live_res.chunk_results[0].status, "success");

    // 4. Retrieve repair history via IPC
    let history = get_repair_history(state.clone(), pid.to_string(), Some(10)).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].status, "success");
    assert_eq!(history[0].bytes_processed, content.len() as u64);
}

#[tokio::test]
async fn test_ipc_repair_cross_profile_isolation_rejected() {
    let (app, temp_dir) = setup_test_state();
    let state = app.state::<DesktopAppState>();

    let profile1 = create_test_profile(state.clone(), &temp_dir, "prof-r-02a");
    let profile2 = create_test_profile(state.clone(), &temp_dir, "prof-r-02b");

    let pid1 = ProfileId::new(&profile1.profile_id).unwrap();
    let pid2 = ProfileId::new(&profile2.profile_id).unwrap();
    let fid1 = FileId::new("file-r-02a").unwrap();

    let source_dir = PathBuf::from(&profile1.source_path);
    let src_file = source_dir.join("test.txt");
    fs::write(&src_file, b"isolation test").unwrap();

    state
        .db
        .create_file(&FileRecord {
            file_id: fid1.clone(),
            profile_id: Some(pid1.clone()),
            file_name: "test.txt".into(),
            relative_path: "test.txt".into(),
            original_size: 14,
            mime_type: None,
            status: "backed_up".into(),
            logical_file_hash: Some("c".repeat(64)),
            created_at: "2026-10-08T10:00:00Z".into(),
            modified_at: None,
            created_timestamp: "2026-10-08T10:00:00Z".into(),
            updated_timestamp: "2026-10-08T10:00:00Z".into(),
        })
        .unwrap();

    // Attempting to repair Profile 1's file under Profile 2 MUST be rejected
    let preview_req = PreviewRepairRequest {
        profile_id: pid2.to_string(),
        target_type: "file".into(),
        target_id: fid1.to_string(),
        operation_id: None,
        passphrase: None,
    };
    let err = preview_repair(state.clone(), preview_req).await;
    assert!(err.is_err(), "Cross-profile repair must be rejected");
}

#[tokio::test]
async fn test_ipc_repair_execution_guard_concurrency_conflict() {
    let (app, temp_dir) = setup_test_state();
    let state = app.state::<DesktopAppState>();

    let profile = create_test_profile(state.clone(), &temp_dir, "prof-r-03");
    let pid = ProfileId::new(&profile.profile_id).unwrap();

    // Acquire execution guard lock externally to simulate concurrent running backup
    let guard = state.scheduler_service.execution_guard();
    let _active_lock = guard.try_acquire(&pid).expect("acquire lock");

    // Now trying to execute repair on the locked profile must fail immediately
    let req = RepairFileRequest {
        profile_id: pid.to_string(),
        file_id: "any-file".into(),
        dry_run: Some(false),
        operation_id: None,
        passphrase: None,
    };
    let err = repair_file(state.clone(), req).await;
    assert!(
        err.is_err(),
        "Concurrent repair during active operation must be rejected"
    );
    let err_msg = err.unwrap_err().to_string();
    assert!(
        err_msg.contains("busy")
            || err_msg.contains("active operation")
            || err_msg.contains("conflict")
    );
}

#[tokio::test]
async fn test_ipc_repair_snapshot_preview_and_execution() {
    let (app, temp_dir) = setup_test_state();
    let state = app.state::<DesktopAppState>();

    let profile = create_test_profile(state.clone(), &temp_dir, "prof-r-04");
    let pid = ProfileId::new(&profile.profile_id).unwrap();
    let fid = FileId::new("file-r-04").unwrap();

    let source_dir = PathBuf::from(&profile.source_path);
    let src_file = source_dir.join("snap_doc.txt");
    let content = b"Snapshot repair content!";
    {
        let mut f = File::create(&src_file).unwrap();
        f.write_all(content).unwrap();
    }

    let file_hash = StreamHasher::hash_bytes(content);
    state
        .db
        .create_file(&FileRecord {
            file_id: fid.clone(),
            profile_id: Some(pid.clone()),
            file_name: "snap_doc.txt".into(),
            relative_path: "snap_doc.txt".into(),
            original_size: content.len() as u64,
            mime_type: None,
            status: "backed_up".into(),
            logical_file_hash: Some(file_hash.clone()),
            created_at: "2026-10-08T10:00:00Z".into(),
            modified_at: None,
            created_timestamp: "2026-10-08T10:00:00Z".into(),
            updated_timestamp: "2026-10-08T10:00:00Z".into(),
        })
        .unwrap();

    let cid = ChunkId::new("chk-r-04").unwrap();
    let old_sref = StorageReference::Telegram {
        chat_id: -1001234567890,
        message_id: 9999,
        file_id: "tg_file_9999".into(),
    };

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-r-04".into(),
        logical_file: LogicalFileMetadata {
            file_id: fid.clone(),
            file_name: "snap_doc.txt".into(),
            relative_path: "snap_doc.txt".into(),
            original_size: content.len() as u64,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        integrity: IntegrityMetadata::sha256(&file_hash),
        chunks: vec![ChunkManifest {
            chunk_id: cid.clone(),
            index: 0,
            plaintext_size: content.len() as u64,
            stored_size: content.len() as u64,
            integrity: IntegrityMetadata::sha256(&file_hash),
            storage_reference: old_sref.clone(),
        }],
    };
    state.db.save_manifest(&manifest).unwrap();

    state
        .db
        .create_chunk(&ChunkRecord {
            chunk_id: cid.clone(),
            file_id: fid.clone(),
            manifest_id: "man-r-04".into(),
            chunk_index: 0,
            plaintext_size: content.len() as u64,
            stored_size: content.len() as u64,
            integrity_hash: file_hash.clone(),
            storage_reference: serde_json::to_string(&old_sref).unwrap(),
            status: "active".into(),
            created_at: "2026-10-08T10:00:00Z".into(),
            updated_at: "2026-10-08T10:00:00Z".into(),
        })
        .unwrap();

    let snap_id = SnapshotId::new("snap-r-04").unwrap();
    state
        .db
        .create_snapshot(&SnapshotRecord {
            snapshot_id: snap_id.clone(),
            profile_id: pid.clone(),
            status: BackupStatus::Completed,
            metadata: None,
            created_at: "2026-10-08T10:00:00Z".into(),
        })
        .unwrap();

    state
        .db
        .create_version(&VersionRecord {
            version_id: VersionId::new("ver-r-04").unwrap(),
            file_id: fid.clone(),
            snapshot_id: snap_id.clone(),
            manifest_id: "man-r-04".into(),
            status: "active".into(),
            created_at: "2026-10-08T10:00:00Z".into(),
        })
        .unwrap();

    // 1. Snapshot preview via IPC
    let prev_req = PreviewRepairRequest {
        profile_id: pid.to_string(),
        target_type: "snapshot".into(),
        target_id: snap_id.to_string(),
        operation_id: None,
        passphrase: None,
    };
    let preview = preview_repair(state.clone(), prev_req).await.unwrap();
    assert_eq!(preview.total_affected_chunks, 1);
    assert!(preview.is_repairable);

    // 2. Snapshot repair execution via IPC
    let snap_req = RepairSnapshotRequest {
        profile_id: pid.to_string(),
        snapshot_id: snap_id.to_string(),
        dry_run: Some(false),
        operation_id: None,
        passphrase: None,
    };
    let exec = repair_snapshot(state.clone(), snap_req).await.unwrap();
    assert_eq!(exec.repaired_chunks, 1);
    assert_eq!(exec.failed_chunks, 0);
}
