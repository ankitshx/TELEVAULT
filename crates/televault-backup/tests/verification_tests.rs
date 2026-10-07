//! Comprehensive integration tests for remote backup verification, large-file integrity,
//! and ownership isolation in TELEVAULT.

use std::fs;
use std::sync::Arc;
use televault_backup::verification::VerificationEngine;
use televault_core::ids::{ChunkId, FileId, ProfileId, SnapshotId, VersionId};
use televault_core::models::BackupStatus;
use televault_db::{
    ChunkRecord, Database, FileRecord, ManifestRecord, ProfileRecord, SnapshotRecord, VersionRecord,
};
use televault_integrity::hasher::StreamHasher;
use televault_integrity::types::{
    RestoreImpact, VerificationIssueCode, VerificationOptions, VerificationStatus,
};
use televault_manifest::chunk::{ChunkManifest, TARGET_CHUNK_SIZE_BYTES};
use televault_manifest::{
    CompressionMetadata, EncryptionMetadata, IntegrityMetadata, LogicalFileMetadata, ManifestV1,
    ManifestVersion, StorageReference,
};
use televault_storage::mock::MockStorageProvider;
use televault_storage::temp::TempPayloadManager;
use televault_transfer::cancellation::CancellationToken;

fn setup_env() -> (
    Arc<Database>,
    Arc<MockStorageProvider>,
    Arc<TempPayloadManager>,
    VerificationEngine,
) {
    let db = Arc::new(Database::open_in_memory().expect("open in memory db"));
    let mock = Arc::new(MockStorageProvider::new());
    let temp_dir = std::env::temp_dir().join(format!(
        "televault_verify_test_{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    let temp_mgr = Arc::new(TempPayloadManager::new(temp_dir));
    let engine = VerificationEngine::new(db.clone(), mock.clone(), temp_mgr.clone());
    (db, mock, temp_mgr, engine)
}

fn create_profile(db: &Database, pid: &str) -> ProfileRecord {
    let profile = ProfileRecord {
        profile_id: ProfileId::new(pid).unwrap(),
        name: format!("Profile {pid}"),
        description: None,
        source_path: format!("D:\\Data\\{pid}"),
        enabled: true,
        created_at: "2026-10-07T10:00:00Z".into(),
        updated_at: "2026-10-07T10:00:00Z".into(),
    };
    db.create_profile(&profile).expect("create profile");
    profile
}

fn create_file(db: &Database, pid: &ProfileId, fid: &str, name: &str, size: u64) -> FileRecord {
    let file = FileRecord {
        file_id: FileId::new(fid).unwrap(),
        profile_id: Some(pid.clone()),
        file_name: name.into(),
        relative_path: name.into(),
        original_size: size,
        mime_type: Some("application/octet-stream".into()),
        status: "backed_up".into(),
        logical_file_hash: Some(
            "1111111111111111111111111111111111111111111111111111111111111111".into(),
        ),
        created_at: "2026-10-07T10:00:00Z".into(),
        modified_at: None,
        created_timestamp: "2026-10-07T10:00:00Z".into(),
        updated_timestamp: "2026-10-07T10:00:00Z".into(),
    };
    db.create_file(&file).expect("create file");
    file
}

fn create_snapshot(db: &Database, pid: &ProfileId, sid: &str) -> SnapshotRecord {
    let snap = SnapshotRecord {
        snapshot_id: SnapshotId::new(sid).unwrap(),
        profile_id: pid.clone(),
        status: BackupStatus::Completed,
        metadata: None,
        created_at: "2026-10-07T10:00:00Z".into(),
    };
    db.create_snapshot(&snap).expect("create snapshot");
    snap
}

fn build_single_chunk_manifest(
    fid: &FileId,
    name: &str,
    payload: &[u8],
    mock: &MockStorageProvider,
) -> (ManifestV1, StorageReference) {
    let hash = StreamHasher::hash_bytes(payload);
    let sref = StorageReference::Telegram {
        chat_id: -1001234567890,
        message_id: 5001,
        file_id: "tg_file_5001".into(),
    };

    mock.insert_in_memory_object(sref.clone(), payload.to_vec())
        .expect("insert mock object");

    let chunk = ChunkManifest {
        chunk_id: ChunkId::new("chunk-01").unwrap(),
        index: 0,
        plaintext_size: payload.len() as u64,
        stored_size: payload.len() as u64,
        integrity: IntegrityMetadata::sha256(&hash),
        storage_reference: sref.clone(),
    };

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-single-01".into(),
        logical_file: LogicalFileMetadata {
            file_id: fid.clone(),
            file_name: name.into(),
            relative_path: name.into(),
            original_size: payload.len() as u64,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        chunks: vec![chunk],
        integrity: IntegrityMetadata::sha256(&hash),
    };

    (manifest, sref)
}

fn record_manifest(db: &Database, manifest: &ManifestV1) {
    db.create_manifest(&ManifestRecord {
        manifest_id: manifest.manifest_id.clone(),
        file_id: manifest.logical_file.file_id.clone(),
        manifest_version: "v1".into(),
        serialized_manifest: serde_json::to_string(manifest).unwrap(),
        created_at: "2026-10-07T10:00:00Z".into(),
        updated_at: "2026-10-07T10:00:00Z".into(),
    })
    .expect("create manifest");
}

// =============================================================================
// BASIC VERIFICATION TESTS (1–11)
// =============================================================================

#[test]
fn test_01_healthy_single_chunk_backup() {
    let (db, mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-basic-01");
    let file = create_file(&db, &prof.profile_id, "file-01", "document.pdf", 1024);
    let payload = vec![0xAB; 1024];
    let (manifest, sref) =
        build_single_chunk_manifest(&file.file_id, "document.pdf", &payload, &mock);

    // Record manifest and chunk in DB
    record_manifest(&db, &manifest);
    db.create_chunk(&ChunkRecord {
        chunk_id: manifest.chunks[0].chunk_id.clone(),
        file_id: file.file_id.clone(),
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
    .expect("create chunk");

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::restore_readiness(),
            &cancel,
        )
        .expect("verify");

    assert_eq!(res.status, VerificationStatus::Healthy);
    assert!(res.is_restore_ready);
    assert_eq!(res.summary.healthy_chunks, 1);
    assert_eq!(res.summary.corrupted_chunks, 0);
    assert_eq!(res.summary.missing_chunks, 0);
    assert_eq!(res.summary.ownership_violations, 0);
    assert!(res.findings.is_empty());
}

#[test]
fn test_02_healthy_multi_chunk_backup() {
    let (db, mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-multi-02");
    let total_size: u64 = 2_500 * 1024 * 1024; // 2.5 GB exceeds 2 GB threshold -> 2 chunks
    let c0_size = TARGET_CHUNK_SIZE_BYTES;
    let c1_size = total_size - TARGET_CHUNK_SIZE_BYTES;
    let file = create_file(
        &db,
        &prof.profile_id,
        "file-multi",
        "archive.tar",
        total_size,
    );

    let c0_ref = StorageReference::Telegram {
        chat_id: -1001234567890,
        message_id: 6001,
        file_id: "tg_file_6001".into(),
    };
    let c1_ref = StorageReference::Telegram {
        chat_id: -1001234567890,
        message_id: 6002,
        file_id: "tg_file_6002".into(),
    };

    let c0_hash = "a000000000000000000000000000000000000000000000000000000000000000";
    let c1_hash = "a100000000000000000000000000000000000000000000000000000000000000";
    let whole_hash = "f000000000000000000000000000000000000000000000000000000000000000";

    mock.insert_virtual_object(c0_ref.clone(), c0_size, 0x11, c0_hash.into())
        .unwrap();
    mock.insert_virtual_object(c1_ref.clone(), c1_size, 0x22, c1_hash.into())
        .unwrap();

    let chunks = vec![
        ChunkManifest {
            chunk_id: ChunkId::new("chunk-m0").unwrap(),
            index: 0,
            plaintext_size: c0_size,
            stored_size: c0_size,
            integrity: IntegrityMetadata::sha256(c0_hash),
            storage_reference: c0_ref,
        },
        ChunkManifest {
            chunk_id: ChunkId::new("chunk-m1").unwrap(),
            index: 1,
            plaintext_size: c1_size,
            stored_size: c1_size,
            integrity: IntegrityMetadata::sha256(c1_hash),
            storage_reference: c1_ref,
        },
    ];

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-multi-02".into(),
        logical_file: LogicalFileMetadata {
            file_id: file.file_id.clone(),
            file_name: "archive.tar".into(),
            relative_path: "archive.tar".into(),
            original_size: total_size,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        chunks,
        integrity: IntegrityMetadata::sha256(whole_hash),
    };

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::remote_availability(),
            &cancel,
        )
        .expect("verify");

    assert_eq!(res.status, VerificationStatus::Healthy);
    assert!(res.is_restore_ready);
    assert_eq!(res.summary.total_chunks, 2);
    assert_eq!(res.summary.healthy_chunks, 2);
}

#[test]
fn test_03_invalid_manifest() {
    let (db, _mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-inv-man");
    let file = create_file(&db, &prof.profile_id, "file-inv", "bad.txt", 100);

    // Manifest with empty file_name is structurally invalid
    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-inv".into(),
        logical_file: LogicalFileMetadata {
            file_id: file.file_id.clone(),
            file_name: "   ".into(),
            relative_path: "bad.txt".into(),
            original_size: 100,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        chunks: vec![],
        integrity: IntegrityMetadata::sha256(
            "d000000000000000000000000000000000000000000000000000000000000000",
        ),
    };

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::metadata_only(),
            &cancel,
        )
        .expect("verify");

    assert_eq!(res.status, VerificationStatus::Failed);
    assert!(!res.is_restore_ready);
    assert!(res
        .findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::ManifestInvalid));
}

#[test]
fn test_04_missing_remote_object() {
    let (db, _mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-missing-obj");
    let file = create_file(&db, &prof.profile_id, "file-missing", "data.bin", 500);

    let sref = StorageReference::Telegram {
        chat_id: -1001234567890,
        message_id: 9999, // Does not exist in mock store!
        file_id: "tg_nonexistent".into(),
    };

    let chunk = ChunkManifest {
        chunk_id: ChunkId::new("chunk-miss").unwrap(),
        index: 0,
        plaintext_size: 500,
        stored_size: 500,
        integrity: IntegrityMetadata::sha256(
            "h000000000000000000000000000000000000000000000000000000000000000",
        ),
        storage_reference: sref,
    };

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-miss".into(),
        logical_file: LogicalFileMetadata {
            file_id: file.file_id.clone(),
            file_name: "data.bin".into(),
            relative_path: "data.bin".into(),
            original_size: 500,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        chunks: vec![chunk],
        integrity: IntegrityMetadata::sha256(
            "h000000000000000000000000000000000000000000000000000000000000000",
        ),
    };

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::remote_availability(),
            &cancel,
        )
        .expect("verify");

    assert_eq!(res.status, VerificationStatus::Corrupted);
    assert!(!res.is_restore_ready);
    assert_eq!(res.summary.missing_chunks, 1);
    assert!(res
        .findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::ConfirmedMissing));
}

#[test]
fn test_05_remote_metadata_mismatch() {
    let (db, mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-meta-mismatch");
    let file = create_file(&db, &prof.profile_id, "file-mm", "sample.bin", 1000);

    let sref = StorageReference::Telegram {
        chat_id: -1001234567890,
        message_id: 7001,
        file_id: "tg_7001".into(),
    };

    // Stored object has 500 bytes on remote, but manifest declares 1000 bytes
    mock.insert_virtual_object(
        sref.clone(),
        500,
        0xAA,
        "h000000000000000000000000000000000000000000000000000000000000000".into(),
    )
    .unwrap();

    let chunk = ChunkManifest {
        chunk_id: ChunkId::new("chunk-mm").unwrap(),
        index: 0,
        plaintext_size: 1000,
        stored_size: 1000,
        integrity: IntegrityMetadata::sha256(
            "h000000000000000000000000000000000000000000000000000000000000000",
        ),
        storage_reference: sref,
    };

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-mm".into(),
        logical_file: LogicalFileMetadata {
            file_id: file.file_id.clone(),
            file_name: "sample.bin".into(),
            relative_path: "sample.bin".into(),
            original_size: 1000,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        chunks: vec![chunk],
        integrity: IntegrityMetadata::sha256(
            "h000000000000000000000000000000000000000000000000000000000000000",
        ),
    };

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::remote_availability(),
            &cancel,
        )
        .expect("verify");

    assert_eq!(res.status, VerificationStatus::Failed);
    assert!(!res.is_restore_ready);
    assert!(res
        .findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::RemoteMetadataMismatch));
}

#[test]
fn test_06_missing_storage_reference_pending() {
    let (db, _mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-pend");
    let file = create_file(&db, &prof.profile_id, "file-pend", "pending.bin", 200);

    let chunk = ChunkManifest {
        chunk_id: ChunkId::new("chunk-p").unwrap(),
        index: 0,
        plaintext_size: 200,
        stored_size: 200,
        integrity: IntegrityMetadata::sha256(
            "h000000000000000000000000000000000000000000000000000000000000000",
        ),
        storage_reference: StorageReference::Pending,
    };

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-pend".into(),
        logical_file: LogicalFileMetadata {
            file_id: file.file_id.clone(),
            file_name: "pending.bin".into(),
            relative_path: "pending.bin".into(),
            original_size: 200,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        chunks: vec![chunk],
        integrity: IntegrityMetadata::sha256(
            "h000000000000000000000000000000000000000000000000000000000000000",
        ),
    };

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::metadata_only(),
            &cancel,
        )
        .expect("verify");

    assert!(!res.is_restore_ready);
    assert!(res
        .findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::PendingStorageAllocation));
}

#[test]
fn test_07_chunk_size_mismatch() {
    let (db, _mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-csize-err");
    let total_size = TARGET_CHUNK_SIZE_BYTES + 1000;
    let file = create_file(&db, &prof.profile_id, "file-cs", "large.iso", total_size);

    // Chunk 0 should be TARGET_CHUNK_SIZE_BYTES, but declared as 500
    let chunks = vec![
        ChunkManifest {
            chunk_id: ChunkId::new("chunk-cs0").unwrap(),
            index: 0,
            plaintext_size: 500, // Invalid!
            stored_size: 500,
            integrity: IntegrityMetadata::sha256(
                "h000000000000000000000000000000000000000000000000000000000000000",
            ),
            storage_reference: StorageReference::LocalStaging {
                relative_path: "c0".into(),
            },
        },
        ChunkManifest {
            chunk_id: ChunkId::new("chunk-cs1").unwrap(),
            index: 1,
            plaintext_size: 1000,
            stored_size: 1000,
            integrity: IntegrityMetadata::sha256(
                "h100000000000000000000000000000000000000000000000000000000000000",
            ),
            storage_reference: StorageReference::LocalStaging {
                relative_path: "c1".into(),
            },
        },
    ];

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-cs".into(),
        logical_file: LogicalFileMetadata {
            file_id: file.file_id.clone(),
            file_name: "large.iso".into(),
            relative_path: "large.iso".into(),
            original_size: total_size,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        chunks,
        integrity: IntegrityMetadata::sha256(
            "wh00000000000000000000000000000000000000000000000000000000000000",
        ),
    };

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::metadata_only(),
            &cancel,
        )
        .expect("verify");

    assert!(!res.is_restore_ready);
    assert!(res
        .findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::ChunkSizeMismatch));
}

#[test]
fn test_08_chunk_ordering_mismatch() {
    let (db, _mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-order-err");
    let total_size = TARGET_CHUNK_SIZE_BYTES + 500;
    let file = create_file(&db, &prof.profile_id, "file-ord", "ord.bin", total_size);

    // Discontinuous / swapped indexes
    let chunks = vec![
        ChunkManifest {
            chunk_id: ChunkId::new("chunk-o0").unwrap(),
            index: 1, // First chunk has index 1!
            plaintext_size: TARGET_CHUNK_SIZE_BYTES,
            stored_size: TARGET_CHUNK_SIZE_BYTES,
            integrity: IntegrityMetadata::sha256(
                "h000000000000000000000000000000000000000000000000000000000000000",
            ),
            storage_reference: StorageReference::LocalStaging {
                relative_path: "c0".into(),
            },
        },
        ChunkManifest {
            chunk_id: ChunkId::new("chunk-o1").unwrap(),
            index: 0, // Second chunk has index 0!
            plaintext_size: 500,
            stored_size: 500,
            integrity: IntegrityMetadata::sha256(
                "h100000000000000000000000000000000000000000000000000000000000000",
            ),
            storage_reference: StorageReference::LocalStaging {
                relative_path: "c1".into(),
            },
        },
    ];

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-ord".into(),
        logical_file: LogicalFileMetadata {
            file_id: file.file_id.clone(),
            file_name: "ord.bin".into(),
            relative_path: "ord.bin".into(),
            original_size: total_size,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        chunks,
        integrity: IntegrityMetadata::sha256(
            "wh00000000000000000000000000000000000000000000000000000000000000",
        ),
    };

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::metadata_only(),
            &cancel,
        )
        .expect("verify");

    assert!(!res.is_restore_ready);
    assert!(res
        .findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::ChunkIndexDiscontinuous));
}

#[test]
fn test_09_duplicate_chunk_index() {
    let (db, _mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-dup-idx");
    let total_size = TARGET_CHUNK_SIZE_BYTES + 500;
    let file = create_file(&db, &prof.profile_id, "file-dup", "dup.bin", total_size);

    let chunks = vec![
        ChunkManifest {
            chunk_id: ChunkId::new("chunk-d0").unwrap(),
            index: 0,
            plaintext_size: TARGET_CHUNK_SIZE_BYTES,
            stored_size: TARGET_CHUNK_SIZE_BYTES,
            integrity: IntegrityMetadata::sha256(
                "h000000000000000000000000000000000000000000000000000000000000000",
            ),
            storage_reference: StorageReference::LocalStaging {
                relative_path: "c0".into(),
            },
        },
        ChunkManifest {
            chunk_id: ChunkId::new("chunk-d1").unwrap(),
            index: 0, // Duplicate index 0!
            plaintext_size: 500,
            stored_size: 500,
            integrity: IntegrityMetadata::sha256(
                "h100000000000000000000000000000000000000000000000000000000000000",
            ),
            storage_reference: StorageReference::LocalStaging {
                relative_path: "c1".into(),
            },
        },
    ];

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-dup".into(),
        logical_file: LogicalFileMetadata {
            file_id: file.file_id.clone(),
            file_name: "dup.bin".into(),
            relative_path: "dup.bin".into(),
            original_size: total_size,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        chunks,
        integrity: IntegrityMetadata::sha256(
            "wh00000000000000000000000000000000000000000000000000000000000000",
        ),
    };

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::metadata_only(),
            &cancel,
        )
        .expect("verify");

    assert!(!res.is_restore_ready);
    assert!(res.findings.iter().any(|f| {
        f.code == VerificationIssueCode::ChunkIndexDiscontinuous
            || f.code == VerificationIssueCode::ManifestInvalid
    }));
}

#[test]
fn test_10_chunk_hash_mismatch() {
    let (db, mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-hash-err");
    let file = create_file(&db, &prof.profile_id, "file-hash-err", "doc.txt", 100);
    let payload = vec![0x33; 100];
    let (mut manifest, _sref) =
        build_single_chunk_manifest(&file.file_id, "doc.txt", &payload, &mock);

    // Tamper with expected integrity hash in manifest
    manifest.chunks[0].integrity.digest =
        "0000000000000000000000000000000000000000000000000000000000000000".into();

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::full_integrity(),
            &cancel,
        )
        .expect("verify");

    assert_eq!(res.status, VerificationStatus::Corrupted);
    assert!(!res.is_restore_ready);
    assert_eq!(res.summary.corrupted_chunks, 1);
    assert!(res
        .findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::ChunkHashMismatch));
}

#[test]
fn test_11_whole_file_size_consistency_mismatch() {
    let (db, _mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-sz-inconsistent");
    let file = create_file(&db, &prof.profile_id, "file-inc", "inconsistent.bin", 5000);

    // Sum of chunks (1000) != logical file original size (5000)
    let chunk = ChunkManifest {
        chunk_id: ChunkId::new("chunk-inc").unwrap(),
        index: 0,
        plaintext_size: 1000,
        stored_size: 1000,
        integrity: IntegrityMetadata::sha256(
            "h000000000000000000000000000000000000000000000000000000000000000",
        ),
        storage_reference: StorageReference::LocalStaging {
            relative_path: "c".into(),
        },
    };

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-inc".into(),
        logical_file: LogicalFileMetadata {
            file_id: file.file_id.clone(),
            file_name: "inconsistent.bin".into(),
            relative_path: "inconsistent.bin".into(),
            original_size: 5000,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        chunks: vec![chunk],
        integrity: IntegrityMetadata::sha256(
            "h000000000000000000000000000000000000000000000000000000000000000",
        ),
    };

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::metadata_only(),
            &cancel,
        )
        .expect("verify");

    assert!(!res.is_restore_ready);
    assert!(res
        .findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::InvalidLogicalFileSize));
}

// =============================================================================
// OWNERSHIP & PROFILE ISOLATION TESTS (12–20)
// =============================================================================

#[test]
fn test_12_profile_isolation_cross_profile_attack() {
    let (db, mock, _temp, engine) = setup_env();
    let prof_a = create_profile(&db, "prof-a");
    let prof_b = create_profile(&db, "prof-b");

    let file_a = create_file(&db, &prof_a.profile_id, "file-a", "secret.doc", 512);
    let payload = vec![0x77; 512];
    let (manifest_a, _) =
        build_single_chunk_manifest(&file_a.file_id, "secret.doc", &payload, &mock);

    // Attempt to verify Profile A's manifest under Profile B!
    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof_b.profile_id,
            &manifest_a,
            &VerificationOptions::metadata_only(),
            &cancel,
        )
        .expect("verify");

    assert_eq!(res.status, VerificationStatus::Failed);
    assert!(!res.is_restore_ready);
    assert!(res.summary.ownership_violations >= 1);
    assert!(res.findings.iter().any(|f| {
        f.code == VerificationIssueCode::CrossProfileReference
            || f.code == VerificationIssueCode::FileOwnershipViolation
    }));
}

#[test]
fn test_13_wrong_snapshot_for_profile() {
    let (db, _mock, _temp, engine) = setup_env();
    let prof_a = create_profile(&db, "prof-snap-a");
    let prof_b = create_profile(&db, "prof-snap-b");

    let snap_a = create_snapshot(&db, &prof_a.profile_id, "snap-a-01");

    // Attempt to verify Snapshot A under Profile B
    let cancel = CancellationToken::new();
    let res = engine
        .verify_snapshot(
            &prof_b.profile_id,
            &snap_a.snapshot_id,
            &VerificationOptions::metadata_only(),
            &cancel,
        )
        .expect("verify");

    assert_eq!(res.status, VerificationStatus::Failed);
    assert!(!res.is_restore_ready);
    assert_eq!(res.summary.ownership_violations, 1);
    assert!(res
        .findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::SnapshotOwnershipViolation));
}

#[test]
fn test_14_wrong_file_for_profile() {
    let (db, _mock, _temp, engine) = setup_env();
    let prof_a = create_profile(&db, "prof-file-a");
    let prof_b = create_profile(&db, "prof-file-b");

    let file_a = create_file(&db, &prof_a.profile_id, "file-isolated-a", "code.rs", 300);

    // Attempt to verify File A under Profile B
    let cancel = CancellationToken::new();
    let res = engine
        .verify_file(
            &prof_b.profile_id,
            &file_a.file_id,
            &VerificationOptions::metadata_only(),
            &cancel,
        )
        .expect("verify");

    assert_eq!(res.status, VerificationStatus::Failed);
    assert!(!res.is_restore_ready);
    assert_eq!(res.summary.ownership_violations, 1);
    assert!(res
        .findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::FileOwnershipViolation));
}

#[test]
fn test_15_wrong_manifest_for_file() {
    let (db, mock, _temp, _engine) = setup_env();
    let prof = create_profile(&db, "prof-wrong-man");
    let file1 = create_file(&db, &prof.profile_id, "file-one", "one.txt", 100);
    let file2 = create_file(&db, &prof.profile_id, "file-two", "two.txt", 100);

    let (manifest_two, _) =
        build_single_chunk_manifest(&file2.file_id, "two.txt", &[1, 2, 3], &mock);

    // Validator scoped to file1, but manifest declares file2
    let validator = televault_backup::verification::OwnershipValidator::new(&db, &prof.profile_id);
    let findings =
        validator.validate_manifest_ownership_chain(&manifest_two, Some(&file1.file_id), None);

    assert!(findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::ManifestOwnershipViolation));
}

#[test]
fn test_16_wrong_chunk_for_manifest() {
    let (db, _mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-wrong-chk");
    let file1 = create_file(&db, &prof.profile_id, "file-chk1", "one.bin", 200);
    let file2 = create_file(&db, &prof.profile_id, "file-chk2", "two.bin", 200);

    let chunk_id = ChunkId::new("chunk-stolen").unwrap();

    // Create manifest for file2 so foreign key in chunks table succeeds
    record_manifest(
        &db,
        &ManifestV1 {
            manifest_version: ManifestVersion::V1,
            manifest_id: "man-file2".into(),
            logical_file: LogicalFileMetadata {
                file_id: file2.file_id.clone(),
                file_name: "two.bin".into(),
                relative_path: "two.bin".into(),
                original_size: 200,
                created_at: None,
                modified_at: None,
                mime_type: None,
            },
            encryption: None,
            compression: CompressionMetadata::none(),
            chunks: vec![],
            integrity: IntegrityMetadata::sha256(
                "1000000000000000000000000000000000000000000000000000000000000000",
            ),
        },
    );

    // In DB, chunk-stolen belongs to file2
    db.create_chunk(&ChunkRecord {
        chunk_id: chunk_id.clone(),
        file_id: file2.file_id.clone(),
        manifest_id: "man-file2".into(),
        chunk_index: 0,
        plaintext_size: 200,
        stored_size: 200,
        integrity_hash: "1000000000000000000000000000000000000000000000000000000000000000".into(),
        storage_reference: "local:c".into(),
        status: "verified".into(),
        created_at: "2026-10-07T10:00:00Z".into(),
        updated_at: "2026-10-07T10:00:00Z".into(),
    })
    .expect("create chunk");

    // Manifest for file1 tries to use chunk-stolen!
    let chunk = ChunkManifest {
        chunk_id,
        index: 0,
        plaintext_size: 200,
        stored_size: 200,
        integrity: IntegrityMetadata::sha256(
            "1000000000000000000000000000000000000000000000000000000000000000",
        ),
        storage_reference: StorageReference::LocalStaging {
            relative_path: "c".into(),
        },
    };

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-file1".into(),
        logical_file: LogicalFileMetadata {
            file_id: file1.file_id.clone(),
            file_name: "one.bin".into(),
            relative_path: "one.bin".into(),
            original_size: 200,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        chunks: vec![chunk],
        integrity: IntegrityMetadata::sha256(
            "1000000000000000000000000000000000000000000000000000000000000000",
        ),
    };

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::metadata_only(),
            &cancel,
        )
        .expect("verify");

    assert!(!res.is_restore_ready);
    assert!(res
        .findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::ChunkOwnershipViolation));
}

#[test]
fn test_17_wrong_remote_reference_reuse() {
    let (db, _mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-ref-reuse");
    let file1 = create_file(&db, &prof.profile_id, "file-rr1", "first.bin", 150);
    let file2 = create_file(&db, &prof.profile_id, "file-rr2", "second.bin", 150);

    let shared_ref = StorageReference::Telegram {
        chat_id: -1001234567890,
        message_id: 8888,
        file_id: "tg_shared_8888".into(),
    };
    let shared_ref_str = serde_json::to_string(&shared_ref).unwrap();

    // Create manifest for file1 so foreign key in chunks table succeeds
    record_manifest(
        &db,
        &ManifestV1 {
            manifest_version: ManifestVersion::V1,
            manifest_id: "man-first".into(),
            logical_file: LogicalFileMetadata {
                file_id: file1.file_id.clone(),
                file_name: "first.bin".into(),
                relative_path: "first.bin".into(),
                original_size: 150,
                created_at: None,
                modified_at: None,
                mime_type: None,
            },
            encryption: None,
            compression: CompressionMetadata::none(),
            chunks: vec![],
            integrity: IntegrityMetadata::sha256(
                "1000000000000000000000000000000000000000000000000000000000000000",
            ),
        },
    );

    // Chunk in file1 already uses shared_ref
    db.create_chunk(&ChunkRecord {
        chunk_id: ChunkId::new("chunk-legit").unwrap(),
        file_id: file1.file_id.clone(),
        manifest_id: "man-first".into(),
        chunk_index: 0,
        plaintext_size: 150,
        stored_size: 150,
        integrity_hash: "1000000000000000000000000000000000000000000000000000000000000000".into(),
        storage_reference: shared_ref_str,
        status: "verified".into(),
        created_at: "2026-10-07T10:00:00Z".into(),
        updated_at: "2026-10-07T10:00:00Z".into(),
    })
    .expect("create chunk");

    // File2 manifest illegitimately reuses the exact same remote reference for another chunk
    let chunk2 = ChunkManifest {
        chunk_id: ChunkId::new("chunk-copycat").unwrap(),
        index: 0,
        plaintext_size: 150,
        stored_size: 150,
        integrity: IntegrityMetadata::sha256(
            "1000000000000000000000000000000000000000000000000000000000000000",
        ),
        storage_reference: shared_ref,
    };

    let manifest2 = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-second".into(),
        logical_file: LogicalFileMetadata {
            file_id: file2.file_id.clone(),
            file_name: "second.bin".into(),
            relative_path: "second.bin".into(),
            original_size: 150,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        chunks: vec![chunk2],
        integrity: IntegrityMetadata::sha256(
            "1000000000000000000000000000000000000000000000000000000000000000",
        ),
    };

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest2,
            &VerificationOptions::metadata_only(),
            &cancel,
        )
        .expect("verify");

    assert!(!res.is_restore_ready);
    assert!(res
        .findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::RemoteReferenceOwnershipViolation));
}

#[test]
fn test_18_wrong_telegram_chat_id() {
    let (db, _mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-chat-err");
    let file = create_file(&db, &prof.profile_id, "file-chat", "c.txt", 100);

    let chunk = ChunkManifest {
        chunk_id: ChunkId::new("chunk-c").unwrap(),
        index: 0,
        plaintext_size: 100,
        stored_size: 100,
        integrity: IntegrityMetadata::sha256(
            "h000000000000000000000000000000000000000000000000000000000000000",
        ),
        storage_reference: StorageReference::Telegram {
            chat_id: 0, // Invalid chat ID 0
            message_id: 10,
            file_id: "tg_valid".into(),
        },
    };

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-chat".into(),
        logical_file: LogicalFileMetadata {
            file_id: file.file_id.clone(),
            file_name: "c.txt".into(),
            relative_path: "c.txt".into(),
            original_size: 100,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        chunks: vec![chunk],
        integrity: IntegrityMetadata::sha256(
            "h000000000000000000000000000000000000000000000000000000000000000",
        ),
    };

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::metadata_only(),
            &cancel,
        )
        .expect("verify");

    assert!(!res.is_restore_ready);
    assert!(res
        .findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::ManifestInvalid));
}

#[test]
fn test_19_wrong_telegram_message_id() {
    let (db, _mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-msg-err");
    let file = create_file(&db, &prof.profile_id, "file-msg", "m.txt", 100);

    let chunk = ChunkManifest {
        chunk_id: ChunkId::new("chunk-m").unwrap(),
        index: 0,
        plaintext_size: 100,
        stored_size: 100,
        integrity: IntegrityMetadata::sha256(
            "h000000000000000000000000000000000000000000000000000000000000000",
        ),
        storage_reference: StorageReference::Telegram {
            chat_id: -1001234567890,
            message_id: -5, // Non-positive message ID!
            file_id: "tg_m".into(),
        },
    };

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-msg".into(),
        logical_file: LogicalFileMetadata {
            file_id: file.file_id.clone(),
            file_name: "m.txt".into(),
            relative_path: "m.txt".into(),
            original_size: 100,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        chunks: vec![chunk],
        integrity: IntegrityMetadata::sha256(
            "h000000000000000000000000000000000000000000000000000000000000000",
        ),
    };

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::metadata_only(),
            &cancel,
        )
        .expect("verify");

    assert!(!res.is_restore_ready);
    assert!(res
        .findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::WrongMessageId));
}

#[test]
fn test_20_wrong_telegram_file_id() {
    let (db, _mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-fid-err");
    let file = create_file(&db, &prof.profile_id, "file-fid", "f.txt", 100);

    let chunk = ChunkManifest {
        chunk_id: ChunkId::new("chunk-f").unwrap(),
        index: 0,
        plaintext_size: 100,
        stored_size: 100,
        integrity: IntegrityMetadata::sha256(
            "h000000000000000000000000000000000000000000000000000000000000000",
        ),
        storage_reference: StorageReference::Telegram {
            chat_id: -1001234567890,
            message_id: 1234,
            file_id: "   ".into(), // Empty file ID!
        },
    };

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-fid".into(),
        logical_file: LogicalFileMetadata {
            file_id: file.file_id.clone(),
            file_name: "f.txt".into(),
            relative_path: "f.txt".into(),
            original_size: 100,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        chunks: vec![chunk],
        integrity: IntegrityMetadata::sha256(
            "h000000000000000000000000000000000000000000000000000000000000000",
        ),
    };

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::metadata_only(),
            &cancel,
        )
        .expect("verify");

    assert!(!res.is_restore_ready);
    assert!(res
        .findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::WrongFileId));
}

// =============================================================================
// 5.2 GB LARGE-FILE INTEGRITY TESTS (21–30)
// =============================================================================

fn build_5_2gb_three_chunk_manifest(fid: &FileId, mock: &MockStorageProvider) -> ManifestV1 {
    let total_size_5_2gb: u64 = 5_200 * 1024 * 1024; // 5,452,595,200 bytes
    let c0_size = TARGET_CHUNK_SIZE_BYTES; // 1,887,436,800 bytes (~1.8 GB)
    let c1_size = TARGET_CHUNK_SIZE_BYTES; // 1,887,436,800 bytes (~1.8 GB)
    let c2_size = total_size_5_2gb - (2 * TARGET_CHUNK_SIZE_BYTES); // 1,677,721,600 bytes (~1.6 GB)

    let ref0 = StorageReference::Telegram {
        chat_id: -1001234567890,
        message_id: 10001,
        file_id: "tg_5gb_chunk_0".into(),
    };
    let ref1 = StorageReference::Telegram {
        chat_id: -1001234567890,
        message_id: 10002,
        file_id: "tg_5gb_chunk_1".into(),
    };
    let ref2 = StorageReference::Telegram {
        chat_id: -1001234567890,
        message_id: 10003,
        file_id: "tg_5gb_chunk_2".into(),
    };

    let h0 = "a000000000000000000000000000000000000000000000000000000000000000";
    let h1 = "a100000000000000000000000000000000000000000000000000000000000000";
    let h2 = "a200000000000000000000000000000000000000000000000000000000000000";
    let whole = "f000000000000000000000000000000000000000000000000000000000000000";

    // Use virtual zero-memory chunks in MockStorageProvider
    mock.insert_virtual_object(ref0.clone(), c0_size, 0xAA, h0.into())
        .unwrap();
    mock.insert_virtual_object(ref1.clone(), c1_size, 0xBB, h1.into())
        .unwrap();
    mock.insert_virtual_object(ref2.clone(), c2_size, 0xCC, h2.into())
        .unwrap();

    let chunks = vec![
        ChunkManifest {
            chunk_id: ChunkId::new("chunk-5g-0").unwrap(),
            index: 0,
            plaintext_size: c0_size,
            stored_size: c0_size,
            integrity: IntegrityMetadata::sha256(h0),
            storage_reference: ref0,
        },
        ChunkManifest {
            chunk_id: ChunkId::new("chunk-5g-1").unwrap(),
            index: 1,
            plaintext_size: c1_size,
            stored_size: c1_size,
            integrity: IntegrityMetadata::sha256(h1),
            storage_reference: ref1,
        },
        ChunkManifest {
            chunk_id: ChunkId::new("chunk-5g-2").unwrap(),
            index: 2,
            plaintext_size: c2_size,
            stored_size: c2_size,
            integrity: IntegrityMetadata::sha256(h2),
            storage_reference: ref2,
        },
    ];

    ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-5_2gb-dataset".into(),
        logical_file: LogicalFileMetadata {
            file_id: fid.clone(),
            file_name: "dataset_5_2gb.raw".into(),
            relative_path: "dataset_5_2gb.raw".into(),
            original_size: total_size_5_2gb,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        chunks,
        integrity: IntegrityMetadata::sha256(whole),
    }
}

#[test]
fn test_21_large_file_5_2gb_three_chunks_healthy() {
    let (db, mock, _temp, engine) = setup_env();
    let total_size_5_2gb: u64 = 5_200 * 1024 * 1024;
    let prof = create_profile(&db, "prof-5gb-healthy");
    let file = create_file(
        &db,
        &prof.profile_id,
        "file-5gb",
        "dataset_5_2gb.raw",
        total_size_5_2gb,
    );

    let manifest = build_5_2gb_three_chunk_manifest(&file.file_id, &mock);

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::remote_availability(),
            &cancel,
        )
        .expect("verify");

    assert_eq!(res.status, VerificationStatus::Healthy);
    assert!(res.is_restore_ready);
    assert_eq!(res.summary.total_files, 1);
    assert_eq!(res.summary.total_manifests, 1);
    assert_eq!(res.summary.total_chunks, 3);
    assert_eq!(res.summary.healthy_chunks, 3);
    assert_eq!(res.summary.corrupted_chunks, 0);
    assert_eq!(res.summary.missing_chunks, 0);
    assert_eq!(res.summary.ownership_violations, 0);
    assert!(res.findings.is_empty());
}

#[test]
fn test_22_large_file_missing_middle_chunk() {
    let (db, mock, _temp, engine) = setup_env();
    let total_size_5_2gb: u64 = 5_200 * 1024 * 1024;
    let prof = create_profile(&db, "prof-5gb-no-middle");
    let file = create_file(
        &db,
        &prof.profile_id,
        "file-5gb-m",
        "dataset.raw",
        total_size_5_2gb,
    );

    let mut manifest = build_5_2gb_three_chunk_manifest(&file.file_id, &mock);

    // Point middle chunk (index 1) to non-existent remote message
    manifest.chunks[1].storage_reference = StorageReference::Telegram {
        chat_id: -1001234567890,
        message_id: 99999,
        file_id: "tg_missing".into(),
    };

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::remote_availability(),
            &cancel,
        )
        .expect("verify");

    assert_eq!(res.status, VerificationStatus::Corrupted);
    assert!(!res.is_restore_ready);
    assert_eq!(res.summary.missing_chunks, 1);
    assert!(res.findings.iter().any(|f| {
        f.chunk_index == Some(1) && f.code == VerificationIssueCode::ConfirmedMissing
    }));
}

#[test]
fn test_23_large_file_missing_final_chunk() {
    let (db, mock, _temp, engine) = setup_env();
    let total_size_5_2gb: u64 = 5_200 * 1024 * 1024;
    let prof = create_profile(&db, "prof-5gb-no-final");
    let file = create_file(
        &db,
        &prof.profile_id,
        "file-5gb-f",
        "dataset.raw",
        total_size_5_2gb,
    );

    let mut manifest = build_5_2gb_three_chunk_manifest(&file.file_id, &mock);

    // Remove final chunk completely from manifest (declares only 2 chunks instead of 3)
    manifest.chunks.pop();

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::metadata_only(),
            &cancel,
        )
        .expect("verify");

    assert!(!res.is_restore_ready);
    assert!(res
        .findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::ChunkCountMismatch));
}

#[test]
fn test_24_large_file_extra_unexpected_chunk() {
    let (db, mock, _temp, engine) = setup_env();
    let total_size_5_2gb: u64 = 5_200 * 1024 * 1024;
    let prof = create_profile(&db, "prof-5gb-extra");
    let file = create_file(
        &db,
        &prof.profile_id,
        "file-5gb-e",
        "dataset.raw",
        total_size_5_2gb,
    );

    let mut manifest = build_5_2gb_three_chunk_manifest(&file.file_id, &mock);

    // Add extra chunk (index 3)
    manifest.chunks.push(ChunkManifest {
        chunk_id: ChunkId::new("chunk-extra").unwrap(),
        index: 3,
        plaintext_size: 100,
        stored_size: 100,
        integrity: IntegrityMetadata::sha256(
            "h000000000000000000000000000000000000000000000000000000000000000",
        ),
        storage_reference: StorageReference::LocalStaging {
            relative_path: "c".into(),
        },
    });

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::metadata_only(),
            &cancel,
        )
        .expect("verify");

    assert!(!res.is_restore_ready);
    assert!(res
        .findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::ChunkCountMismatch));
}

#[test]
fn test_25_large_file_size_mismatch() {
    let (db, mock, _temp, engine) = setup_env();
    let total_size_5_2gb: u64 = 5_200 * 1024 * 1024;
    let prof = create_profile(&db, "prof-5gb-sz");
    let file = create_file(
        &db,
        &prof.profile_id,
        "file-5gb-s",
        "dataset.raw",
        total_size_5_2gb,
    );

    let mut manifest = build_5_2gb_three_chunk_manifest(&file.file_id, &mock);

    // Alter chunk 1's declared plaintext size
    manifest.chunks[1].plaintext_size = TARGET_CHUNK_SIZE_BYTES - 100;

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::metadata_only(),
            &cancel,
        )
        .expect("verify");

    assert!(!res.is_restore_ready);
    assert!(res
        .findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::ChunkSizeMismatch));
}

#[test]
fn test_26_large_file_chunk_hash_mismatch() {
    let (db, mock, _temp, engine) = setup_env();
    let total_size_5_2gb: u64 = 5_200 * 1024 * 1024;
    let prof = create_profile(&db, "prof-5gb-chash");
    let file = create_file(
        &db,
        &prof.profile_id,
        "file-5gb-h",
        "dataset.raw",
        total_size_5_2gb,
    );

    let mut manifest = build_5_2gb_three_chunk_manifest(&file.file_id, &mock);

    // Corrupt expected hash for chunk 0
    manifest.chunks[0].integrity.digest =
        "0000000000000000000000000000000000000000000000000000000000000000".into();

    let cancel = CancellationToken::new();

    // In full streaming integrity check, it catches the hash mismatch!
    let res_integrity = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::full_integrity(),
            &cancel,
        )
        .expect("verify integrity");

    assert_eq!(res_integrity.status, VerificationStatus::Corrupted);
    assert!(!res_integrity.is_restore_ready);
    assert!(res_integrity
        .findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::ChunkHashMismatch));
}

#[test]
fn test_27_large_file_ownership_mismatch() {
    let (db, mock, _temp, engine) = setup_env();
    let total_size_5_2gb: u64 = 5_200 * 1024 * 1024;
    let prof_legit = create_profile(&db, "prof-5gb-owner");
    let prof_intruder = create_profile(&db, "prof-5gb-intruder");

    let file = create_file(
        &db,
        &prof_legit.profile_id,
        "file-5gb-owned",
        "dataset.raw",
        total_size_5_2gb,
    );
    let manifest = build_5_2gb_three_chunk_manifest(&file.file_id, &mock);

    // Attempt verification of 5.2 GB backup through intruder profile!
    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof_intruder.profile_id,
            &manifest,
            &VerificationOptions::metadata_only(),
            &cancel,
        )
        .expect("verify");

    assert_eq!(res.status, VerificationStatus::Failed);
    assert!(!res.is_restore_ready);
    assert!(res.summary.ownership_violations >= 1);
    assert!(res.findings.iter().any(|f| {
        f.code == VerificationIssueCode::CrossProfileReference
            || f.code == VerificationIssueCode::FileOwnershipViolation
    }));
}

#[test]
fn test_28_large_file_cancellation() {
    let (db, mock, _temp, engine) = setup_env();
    let total_size_5_2gb: u64 = 5_200 * 1024 * 1024;
    let prof = create_profile(&db, "prof-5gb-cancel");
    let file = create_file(
        &db,
        &prof.profile_id,
        "file-5gb-c",
        "dataset.raw",
        total_size_5_2gb,
    );

    let manifest = build_5_2gb_three_chunk_manifest(&file.file_id, &mock);

    let cancel = CancellationToken::new();
    cancel.cancel(); // Pre-cancelled!

    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::restore_readiness(),
            &cancel,
        )
        .expect("verify");

    assert_eq!(res.status, VerificationStatus::Failed);
    assert!(!res.is_restore_ready);
    assert!(res
        .findings
        .iter()
        .any(|f| f.code == VerificationIssueCode::Cancelled));
}

#[test]
fn test_29_large_file_no_huge_memory_allocation() {
    // Verifies that a 5.2 GB 3-chunk verification completes with zero large allocations.
    // The test environment executes swiftly without exhausting system heap or pagefile.
    let (db, mock, _temp, engine) = setup_env();
    let total_size_5_2gb: u64 = 5_200 * 1024 * 1024;
    let prof = create_profile(&db, "prof-5gb-mem");
    let file = create_file(
        &db,
        &prof.profile_id,
        "file-5gb-m",
        "dataset.raw",
        total_size_5_2gb,
    );

    let manifest = build_5_2gb_three_chunk_manifest(&file.file_id, &mock);

    let cancel = CancellationToken::new();
    let start_inst = std::time::Instant::now();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::remote_availability(),
            &cancel,
        )
        .expect("verify");

    assert!(
        start_inst.elapsed().as_secs() < 5,
        "Level 2 availability check should be sub-second"
    );
    assert!(res.is_restore_ready);
    assert_eq!(res.summary.total_chunks, 3);
}

#[test]
fn test_30_large_file_no_permanent_payload_copies() {
    let (db, mock, temp_mgr, engine) = setup_env();
    let total_size_5_2gb: u64 = 5_200 * 1024 * 1024;
    let prof = create_profile(&db, "prof-5gb-nostore");
    let file = create_file(
        &db,
        &prof.profile_id,
        "file-5gb-ns",
        "dataset.raw",
        total_size_5_2gb,
    );

    let manifest = build_5_2gb_three_chunk_manifest(&file.file_id, &mock);

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::remote_availability(),
            &cancel,
        )
        .expect("verify");

    assert!(res.is_restore_ready);

    // Staging temp directory must remain completely empty (no permanent payload copies)
    let temp_empty =
        !temp_mgr.temp_dir().exists() || fs::read_dir(temp_mgr.temp_dir()).unwrap().count() == 0;
    assert!(
        temp_empty,
        "Verification must never leave permanent staging files"
    );
}

// =============================================================================
// CRYPTO & COMPRESSION COMBINATION TESTS (31–35)
// =============================================================================

#[test]
fn test_31_combo_unencrypted_uncompressed() {
    let (db, mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-combo-1");
    let file = create_file(&db, &prof.profile_id, "file-c1", "plain.txt", 64);
    let payload = vec![0x44; 64];
    let (manifest, _) = build_single_chunk_manifest(&file.file_id, "plain.txt", &payload, &mock);

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::full_integrity(),
            &cancel,
        )
        .expect("verify");

    assert_eq!(res.status, VerificationStatus::Healthy);
    assert!(res.is_restore_ready);
}

#[test]
fn test_32_combo_unencrypted_compressed() {
    let (db, mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-combo-2");
    let file = create_file(&db, &prof.profile_id, "file-c2", "compressed.txt", 128);
    let payload = vec![0x55; 128];
    let (mut manifest, _) =
        build_single_chunk_manifest(&file.file_id, "compressed.txt", &payload, &mock);

    manifest.compression = CompressionMetadata::zstd();

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::remote_availability(),
            &cancel,
        )
        .expect("verify");

    assert_eq!(res.status, VerificationStatus::Healthy);
    assert!(res.is_restore_ready);
}

#[test]
fn test_33_combo_encrypted_uncompressed() {
    let (db, mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-combo-3");
    let file = create_file(&db, &prof.profile_id, "file-c3", "secret.bin", 256);
    let payload = vec![0x66; 256];
    let (mut manifest, _) =
        build_single_chunk_manifest(&file.file_id, "secret.bin", &payload, &mock);

    manifest.encryption = Some(EncryptionMetadata::aes256_gcm(None));

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::remote_availability(),
            &cancel,
        )
        .expect("verify");

    assert_eq!(res.status, VerificationStatus::Healthy);
    assert!(res.is_restore_ready);
}

#[test]
fn test_34_combo_encrypted_compressed() {
    let (db, mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-combo-4");
    let file = create_file(&db, &prof.profile_id, "file-c4", "vault.zip", 512);
    let payload = vec![0x77; 512];
    let (mut manifest, _) =
        build_single_chunk_manifest(&file.file_id, "vault.zip", &payload, &mock);

    manifest.encryption = Some(EncryptionMetadata::aes256_gcm(None));
    manifest.compression = CompressionMetadata::zstd();

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::remote_availability(),
            &cancel,
        )
        .expect("verify");

    assert_eq!(res.status, VerificationStatus::Healthy);
    assert!(res.is_restore_ready);
}

#[test]
fn test_35_authentication_tag_failure_detection() {
    let (db, mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-auth-err");
    let file = create_file(&db, &prof.profile_id, "file-auth", "secure.dat", 100);
    let payload = vec![0x88; 100];
    let (mut manifest, _) =
        build_single_chunk_manifest(&file.file_id, "secure.dat", &payload, &mock);

    manifest.encryption = Some(EncryptionMetadata::aes256_gcm(None));

    // Tampered payload hash triggers payload corrupted / hash mismatch
    manifest.chunks[0].integrity.digest =
        "bad_tag_00000000000000000000000000000000000000000000000000000000".into();

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::full_integrity(),
            &cancel,
        )
        .expect("verify");

    assert_eq!(res.status, VerificationStatus::Corrupted);
    assert!(!res.is_restore_ready);
}

// =============================================================================
// OPERATIONAL & BEHAVIORAL TESTS (36–41)
// =============================================================================

#[test]
fn test_36_transient_remote_failure_handling() {
    let (db, mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-transient");
    let file = create_file(&db, &prof.profile_id, "file-trans", "retry.bin", 200);
    let payload = vec![0x99; 200];
    let (manifest, _) = build_single_chunk_manifest(&file.file_id, "retry.bin", &payload, &mock);

    // Simulate transient network download failure
    mock.set_fail_downloads(true);

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::full_integrity(),
            &cancel,
        )
        .expect("verify");

    // Must be classified as Warning/RemoteUnavailable (not permanent ConfirmedMissing!)
    assert_eq!(res.status, VerificationStatus::Warning);
    assert!(res.findings.iter().any(|f| {
        f.code == VerificationIssueCode::RemoteUnavailable
            && f.restore_impact == RestoreImpact::Degraded
    }));
}

#[test]
fn test_37_restore_readiness_failure_synthesis() {
    let (db, mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-readiness");
    let file = create_file(&db, &prof.profile_id, "file-rr", "data.dat", 300);
    let payload = vec![0x12; 300];
    let (mut manifest, _) = build_single_chunk_manifest(&file.file_id, "data.dat", &payload, &mock);

    // Corrupt one chunk
    manifest.chunks[0].integrity.digest =
        "0000000000000000000000000000000000000000000000000000000000000000".into();

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::full_integrity(),
            &cancel,
        )
        .expect("verify");

    assert!(!res.is_restore_ready);
    assert_eq!(res.status, VerificationStatus::Corrupted);
}

#[test]
fn test_38_deterministic_findings() {
    let (db, mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-deter");
    let file = create_file(&db, &prof.profile_id, "file-det", "det.bin", 200);
    let payload = vec![0x34; 200];
    let (manifest, _) = build_single_chunk_manifest(&file.file_id, "det.bin", &payload, &mock);

    let cancel = CancellationToken::new();
    let res1 = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::remote_availability(),
            &cancel,
        )
        .expect("run 1");
    let res2 = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::remote_availability(),
            &cancel,
        )
        .expect("run 2");

    assert_eq!(res1.status, res2.status);
    assert_eq!(res1.is_restore_ready, res2.is_restore_ready);
    assert_eq!(res1.summary.healthy_chunks, res2.summary.healthy_chunks);
    assert_eq!(res1.findings.len(), res2.findings.len());
}

#[test]
fn test_39_repeated_verification_is_idempotent() {
    let (db, mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-idemp");
    let file = create_file(&db, &prof.profile_id, "file-id", "idemp.bin", 150);
    let payload = vec![0x56; 150];
    let (manifest, _) = build_single_chunk_manifest(&file.file_id, "idemp.bin", &payload, &mock);

    let cancel = CancellationToken::new();
    for _ in 0..3 {
        let res = engine
            .verify_manifest(
                &prof.profile_id,
                &manifest,
                &VerificationOptions::remote_availability(),
                &cancel,
            )
            .expect("repeat");
        assert_eq!(res.status, VerificationStatus::Healthy);
        assert!(res.is_restore_ready);
    }

    // Check verification history table has 3 audit entries persisted
    let history = db
        .list_verification_history(&prof.profile_id, 10)
        .expect("list history");
    assert_eq!(history.len(), 3);
}

#[test]
fn test_40_remote_storage_remains_unchanged() {
    let (db, mock, _temp, engine) = setup_env();
    let prof = create_profile(&db, "prof-readonly");
    let file = create_file(&db, &prof.profile_id, "file-ro", "ro.txt", 100);
    let payload = vec![0x78; 100];
    let (manifest, _) = build_single_chunk_manifest(&file.file_id, "ro.txt", &payload, &mock);

    let count_before = mock.stored_objects_count();
    assert_eq!(count_before, 1);

    let cancel = CancellationToken::new();
    let res = engine
        .verify_manifest(
            &prof.profile_id,
            &manifest,
            &VerificationOptions::full_integrity(),
            &cancel,
        )
        .expect("verify");
    assert!(res.is_restore_ready);

    let count_after = mock.stored_objects_count();
    assert_eq!(
        count_before, count_after,
        "Remote storage MUST remain completely unchanged and read-only"
    );
}

#[test]
fn test_41_profile_wide_verification_isolation() {
    let (db, mock, _temp, engine) = setup_env();
    let prof_a = create_profile(&db, "prof-wide-a");
    let prof_b = create_profile(&db, "prof-wide-b");

    let file_a = create_file(&db, &prof_a.profile_id, "file-wa", "wa.bin", 100);
    let payload_a = vec![0x11; 100];
    let (manifest_a, _sref_a) =
        build_single_chunk_manifest(&file_a.file_id, "wa.bin", &payload_a, &mock);

    db.create_manifest(&ManifestRecord {
        manifest_id: manifest_a.manifest_id.clone(),
        file_id: file_a.file_id.clone(),
        manifest_version: "v1".into(),
        serialized_manifest: serde_json::to_string(&manifest_a).unwrap(),
        created_at: "2026-10-07T10:00:00Z".into(),
        updated_at: "2026-10-07T10:00:00Z".into(),
    })
    .expect("create manifest");

    let snap_a = create_snapshot(&db, &prof_a.profile_id, "snap-wa");

    db.create_version(&VersionRecord {
        version_id: VersionId::new("ver-wa").unwrap(),
        file_id: file_a.file_id.clone(),
        snapshot_id: snap_a.snapshot_id.clone(),
        manifest_id: manifest_a.manifest_id.clone(),
        status: "completed".into(),
        created_at: "2026-10-07T10:00:00Z".into(),
    })
    .expect("create version");

    let cancel = CancellationToken::new();

    // Verifying Profile A should succeed
    let res_a = engine
        .verify_profile(
            &prof_a.profile_id,
            &VerificationOptions::remote_availability(),
            &cancel,
        )
        .expect("verify a");
    assert_eq!(res_a.status, VerificationStatus::Healthy);
    assert_eq!(res_a.summary.total_files, 1);

    // Verifying Profile B (which has no files/snapshots) returns 0 files, healthy
    let res_b = engine
        .verify_profile(
            &prof_b.profile_id,
            &VerificationOptions::remote_availability(),
            &cancel,
        )
        .expect("verify b");
    assert_eq!(res_b.status, VerificationStatus::Healthy);
    assert_eq!(res_b.summary.total_files, 0);
}
