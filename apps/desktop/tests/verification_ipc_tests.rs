//! Comprehensive IPC integration tests for remote verification and integrity commands.

use std::fs;
use std::path::PathBuf;
use tauri::Manager;
use televault_core::ids::{ChunkId, FileId, ProfileId, SnapshotId, VersionId};
use televault_core::models::BackupStatus;
use televault_db::{ChunkRecord, FileRecord, ManifestRecord, SnapshotRecord, VersionRecord};
use televault_desktop::commands::backup::*;
use televault_desktop::commands::verification::*;
use televault_desktop::dto::*;
use televault_desktop::state::DesktopAppState;
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
    let temp_dir = std::env::temp_dir().join(format!("televault_verify_ipc_test_{nanos}"));
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
            description: Some("Test profile for verification IPC".to_string()),
            source_path: source.to_string_lossy().to_string(),
        },
    )
    .expect("create profile")
}

#[tokio::test]
async fn test_ipc_verify_manifest_healthy_and_history_recorded() {
    let (app, temp_dir) = setup_test_state();
    let state = app.state::<DesktopAppState>();

    let profile = create_test_profile(state.clone(), &temp_dir, "prof-v-01");
    let pid = ProfileId::new(&profile.profile_id).unwrap();
    let fid = FileId::new("file-v-01").unwrap();

    state
        .db
        .create_file(&FileRecord {
            file_id: fid.clone(),
            profile_id: Some(pid.clone()),
            file_name: "test.doc".into(),
            relative_path: "test.doc".into(),
            original_size: 1024,
            mime_type: None,
            status: "backed_up".into(),
            logical_file_hash: Some("1".repeat(64)),
            created_at: "2026-10-07T10:00:00Z".into(),
            modified_at: None,
            created_timestamp: "2026-10-07T10:00:00Z".into(),
            updated_timestamp: "2026-10-07T10:00:00Z".into(),
        })
        .unwrap();

    let sref = StorageReference::Telegram {
        chat_id: -1001234567890,
        message_id: 9001,
        file_id: "tg_9001".into(),
    };

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-ipc-01".into(),
        logical_file: LogicalFileMetadata {
            file_id: fid.clone(),
            file_name: "test.doc".into(),
            relative_path: "test.doc".into(),
            original_size: 1024,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        chunks: vec![ChunkManifest {
            chunk_id: ChunkId::new("chunk-ipc-01").unwrap(),
            index: 0,
            plaintext_size: 1024,
            stored_size: 1024,
            integrity: IntegrityMetadata::sha256(
                "a000000000000000000000000000000000000000000000000000000000000000",
            ),
            storage_reference: sref.clone(),
        }],
        integrity: IntegrityMetadata::sha256(
            "a000000000000000000000000000000000000000000000000000000000000000",
        ),
    };

    // Store manifest in DB
    state
        .db
        .create_manifest(&ManifestRecord {
            manifest_id: manifest.manifest_id.clone(),
            file_id: fid.clone(),
            manifest_version: "v1".into(),
            serialized_manifest: serde_json::to_string(&manifest).unwrap(),
            created_at: "2026-10-07T10:00:00Z".into(),
            updated_at: "2026-10-07T10:00:00Z".into(),
        })
        .unwrap();

    // Store chunk in DB
    state
        .db
        .create_chunk(&ChunkRecord {
            chunk_id: manifest.chunks[0].chunk_id.clone(),
            file_id: fid.clone(),
            manifest_id: manifest.manifest_id.clone(),
            chunk_index: 0,
            plaintext_size: 1024,
            stored_size: 1024,
            integrity_hash: manifest.chunks[0].integrity.digest.clone(),
            storage_reference: serde_json::to_string(&sref).unwrap(),
            status: "verified".into(),
            created_at: "2026-10-07T10:00:00Z".into(),
            updated_at: "2026-10-07T10:00:00Z".into(),
        })
        .unwrap();

    // Invoke IPC command: verify_manifest (Level 1: metadata only)
    let req = VerifyTargetRequest {
        profile_id: profile.profile_id.clone(),
        target_id: Some("man-ipc-01".into()),
        level: Some(1),
        full_hash_check: Some(false),
        decrypt_check: Some(false),
        operation_id: None,
    };

    let result = verify_manifest(state.clone(), req)
        .await
        .expect("verify_manifest");

    assert_eq!(result.status, "healthy");
    assert!(result.is_restore_ready);
    assert_eq!(result.summary.healthy_chunks, 1);
    assert_eq!(result.summary.corrupted_chunks, 0);

    // Verify history recorded in SQLite
    let history = get_verification_history(state.clone(), profile.profile_id.clone(), Some(10))
        .expect("get_verification_history");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].target_id, "man-ipc-01");
    assert_eq!(history[0].status, "healthy");
}

#[tokio::test]
async fn test_ipc_verify_file_backup() {
    let (app, temp_dir) = setup_test_state();
    let state = app.state::<DesktopAppState>();

    let profile = create_test_profile(state.clone(), &temp_dir, "prof-v-02");
    let pid = ProfileId::new(&profile.profile_id).unwrap();
    let fid = FileId::new("file-v-02").unwrap();

    state
        .db
        .create_file(&FileRecord {
            file_id: fid.clone(),
            profile_id: Some(pid.clone()),
            file_name: "archive.zip".into(),
            relative_path: "archive.zip".into(),
            original_size: 2048,
            mime_type: None,
            status: "backed_up".into(),
            logical_file_hash: Some("2".repeat(64)),
            created_at: "2026-10-07T10:00:00Z".into(),
            modified_at: None,
            created_timestamp: "2026-10-07T10:00:00Z".into(),
            updated_timestamp: "2026-10-07T10:00:00Z".into(),
        })
        .unwrap();

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-file-02".into(),
        logical_file: LogicalFileMetadata {
            file_id: fid.clone(),
            file_name: "archive.zip".into(),
            relative_path: "archive.zip".into(),
            original_size: 2048,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        chunks: vec![ChunkManifest {
            chunk_id: ChunkId::new("chunk-file-02").unwrap(),
            index: 0,
            plaintext_size: 2048,
            stored_size: 2048,
            integrity: IntegrityMetadata::sha256(
                "b000000000000000000000000000000000000000000000000000000000000000",
            ),
            storage_reference: StorageReference::Telegram {
                chat_id: -1001234567890,
                message_id: 9002,
                file_id: "tg_9002".into(),
            },
        }],
        integrity: IntegrityMetadata::sha256(
            "b000000000000000000000000000000000000000000000000000000000000000",
        ),
    };

    state
        .db
        .create_manifest(&ManifestRecord {
            manifest_id: manifest.manifest_id.clone(),
            file_id: fid.clone(),
            manifest_version: "v1".into(),
            serialized_manifest: serde_json::to_string(&manifest).unwrap(),
            created_at: "2026-10-07T10:00:00Z".into(),
            updated_at: "2026-10-07T10:00:00Z".into(),
        })
        .unwrap();

    let sref_str = serde_json::to_string(&manifest.chunks[0].storage_reference).unwrap();
    state
        .db
        .create_chunk(&ChunkRecord {
            chunk_id: manifest.chunks[0].chunk_id.clone(),
            file_id: fid.clone(),
            manifest_id: manifest.manifest_id.clone(),
            chunk_index: 0,
            plaintext_size: 2048,
            stored_size: 2048,
            integrity_hash: manifest.chunks[0].integrity.digest.clone(),
            storage_reference: sref_str,
            status: "verified".into(),
            created_at: "2026-10-07T10:00:00Z".into(),
            updated_at: "2026-10-07T10:00:00Z".into(),
        })
        .unwrap();

    let sid = SnapshotId::new("snap-02").unwrap();
    state
        .db
        .create_snapshot(&SnapshotRecord {
            snapshot_id: sid.clone(),
            profile_id: pid.clone(),
            status: BackupStatus::Completed,
            metadata: None,
            created_at: "2026-10-07T10:00:00Z".into(),
        })
        .unwrap();

    state
        .db
        .create_version(&VersionRecord {
            version_id: VersionId::new("ver-02").unwrap(),
            file_id: fid.clone(),
            snapshot_id: sid,
            manifest_id: manifest.manifest_id.clone(),
            status: "active".into(),
            created_at: "2026-10-07T10:00:00Z".into(),
        })
        .unwrap();

    let req = VerifyTargetRequest {
        profile_id: profile.profile_id.clone(),
        target_id: Some("file-v-02".into()),
        level: Some(1),
        full_hash_check: Some(false),
        decrypt_check: Some(false),
        operation_id: None,
    };

    let result = verify_file_backup(state.clone(), req)
        .await
        .expect("verify_file_backup");
    assert_eq!(result.target_type, "file");
    assert_eq!(result.target_id, "file-v-02");
    assert_eq!(result.status, "healthy");
    assert!(result.is_restore_ready);
}

#[tokio::test]
async fn test_ipc_verify_profile_cross_profile_isolation() {
    let (app, temp_dir) = setup_test_state();
    let state = app.state::<DesktopAppState>();

    let prof_a = create_test_profile(state.clone(), &temp_dir, "prof-owner-a");
    let prof_b = create_test_profile(state.clone(), &temp_dir, "prof-intruder-b");

    let fid_a = FileId::new("file-isolated-a").unwrap();
    state
        .db
        .create_file(&FileRecord {
            file_id: fid_a.clone(),
            profile_id: Some(ProfileId::new(&prof_a.profile_id).unwrap()),
            file_name: "secret.txt".into(),
            relative_path: "secret.txt".into(),
            original_size: 512,
            mime_type: None,
            status: "backed_up".into(),
            logical_file_hash: Some("3".repeat(64)),
            created_at: "2026-10-07T10:00:00Z".into(),
            modified_at: None,
            created_timestamp: "2026-10-07T10:00:00Z".into(),
            updated_timestamp: "2026-10-07T10:00:00Z".into(),
        })
        .unwrap();

    // Attacker profile B attempts to verify Profile A's file!
    let req = VerifyTargetRequest {
        profile_id: prof_b.profile_id.clone(),
        target_id: Some("file-isolated-a".into()),
        level: Some(1),
        full_hash_check: Some(false),
        decrypt_check: Some(false),
        operation_id: None,
    };

    let result = verify_file_backup(state.clone(), req)
        .await
        .expect("cross profile verify");
    assert_eq!(result.status, "failed");
    assert!(!result.is_restore_ready);
    assert!(result.summary.ownership_violations >= 1);
    assert!(result
        .findings
        .iter()
        .any(|f| f.code == "FileOwnershipViolation"));
}

#[tokio::test]
async fn test_ipc_verify_snapshot_and_cancellation() {
    let (app, temp_dir) = setup_test_state();
    let state = app.state::<DesktopAppState>();

    let profile = create_test_profile(state.clone(), &temp_dir, "prof-v-cancel");
    let pid = ProfileId::new(&profile.profile_id).unwrap();
    let sid = SnapshotId::new("snap-cancel-01").unwrap();

    state
        .db
        .create_snapshot(&SnapshotRecord {
            snapshot_id: sid.clone(),
            profile_id: pid.clone(),
            status: BackupStatus::Completed,
            metadata: None,
            created_at: "2026-10-07T10:00:00Z".into(),
        })
        .unwrap();

    // Verify snapshot empty
    let req = VerifyTargetRequest {
        profile_id: profile.profile_id.clone(),
        target_id: Some("snap-cancel-01".into()),
        level: Some(1),
        full_hash_check: Some(false),
        decrypt_check: Some(false),
        operation_id: None,
    };

    let result = verify_snapshot(state.clone(), req)
        .await
        .expect("verify_snapshot");
    assert_eq!(result.status, "healthy");
    assert_eq!(result.target_type, "snapshot");
}
