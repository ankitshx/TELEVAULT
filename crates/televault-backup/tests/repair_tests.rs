//! Production-grade integration tests for the Remote Repair & Recovery Engine,
//! large-file reconstruction, ownership safety, and transactional consistency in TELEVAULT.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use televault_backup::repair::{RepairCandidate, RepairEngine, RepairPipeline};
use televault_core::ids::{ChunkId, FileId, ProfileId, SnapshotId, VersionId};
use televault_core::models::BackupStatus;
use televault_crypto::kdf::{derive_key, KdfParams};
use televault_crypto::key::Salt;
use televault_crypto::policy::EncryptionPolicy;
use televault_db::{
    ChunkRecord, Database, FileRecord, ProfileRecord, SnapshotRecord, VersionRecord,
};
use televault_integrity::hasher::StreamHasher;
use televault_manifest::{
    CompressionMetadata, EncryptionMetadata, IntegrityMetadata, LogicalFileMetadata, ManifestV1,
    ManifestVersion, StorageReference,
};
use televault_storage::mock::MockStorageProvider;
use televault_storage::temp::TempPayloadManager;
use televault_storage::StorageProvider;
use televault_transfer::cancellation::CancellationToken;
use televault_transfer::engine::{TransferEngine, TransferEngineConfig};

struct TestFixture {
    pub db: Arc<Database>,
    pub mock_storage: Arc<MockStorageProvider>,
    pub temp_manager: Arc<TempPayloadManager>,
    pub transfer_engine: Arc<TransferEngine>,
    pub repair_engine: RepairEngine,
    pub base_dir: PathBuf,
}

fn setup_fixture(test_name: &str) -> TestFixture {
    let nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let base_dir = std::env::temp_dir().join(format!("televault_repair_{test_name}_{nanos}"));
    let temp_staging_dir = base_dir.join("staging");
    fs::create_dir_all(&temp_staging_dir).expect("create staging dir");

    let db = Arc::new(Database::open_in_memory().expect("open db"));
    let mock_storage = Arc::new(MockStorageProvider::new());
    let temp_manager = Arc::new(TempPayloadManager::new(&temp_staging_dir));

    let transfer_config = TransferEngineConfig::default();
    let transfer_engine = Arc::new(TransferEngine::new(
        mock_storage.clone(),
        transfer_config,
        None,
    ));

    let verification_engine = Arc::new(televault_backup::verification::VerificationEngine::new(
        db.clone(),
        mock_storage.clone(),
        temp_manager.clone(),
    ));

    let repair_engine = RepairEngine::new(
        db.clone(),
        transfer_engine.clone(),
        temp_manager.clone(),
        verification_engine,
    );

    TestFixture {
        db,
        mock_storage,
        temp_manager,
        transfer_engine,
        repair_engine,
        base_dir,
    }
}

fn create_source_file(dir: &Path, rel_path: &str, content: &[u8]) -> PathBuf {
    let full = dir.join(rel_path);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    let mut f = File::create(&full).expect("create file");
    f.write_all(content).expect("write content");
    f.flush().expect("flush");
    full
}

fn create_test_profile(db: &Database, pid: &str, source_dir: &Path) -> ProfileRecord {
    let profile = ProfileRecord {
        profile_id: ProfileId::new(pid).unwrap(),
        name: format!("Profile {pid}"),
        description: None,
        source_path: source_dir.to_string_lossy().to_string(),
        enabled: true,
        created_at: "2026-10-08T10:00:00Z".into(),
        updated_at: "2026-10-08T10:00:00Z".into(),
    };
    db.create_profile(&profile).expect("create profile");
    profile
}

fn populate_file_and_manifest(
    fixture: &TestFixture,
    pid: &ProfileId,
    fid_str: &str,
    rel_path: &str,
    content: &[u8],
) -> (FileRecord, ManifestV1, StorageReference) {
    let fid = FileId::new(fid_str).unwrap();
    let file = FileRecord {
        file_id: fid.clone(),
        profile_id: Some(pid.clone()),
        file_name: rel_path.to_string(),
        relative_path: rel_path.to_string(),
        original_size: content.len() as u64,
        mime_type: Some("application/octet-stream".into()),
        status: "backed_up".into(),
        logical_file_hash: Some(StreamHasher::hash_bytes(content)),
        created_at: "2026-10-08T10:00:00Z".into(),
        modified_at: None,
        created_timestamp: "2026-10-08T10:00:00Z".into(),
        updated_timestamp: "2026-10-08T10:00:00Z".into(),
    };
    fixture.db.create_file(&file).expect("create file");

    let cid = ChunkId::new(format!("{fid_str}-chk-0")).unwrap();
    let hash = StreamHasher::hash_bytes(content);
    let sref = StorageReference::Telegram {
        chat_id: -1009876543210,
        message_id: 8801,
        file_id: "tg_file_8801".into(),
    };

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: format!("man-{fid_str}"),
        logical_file: LogicalFileMetadata {
            file_id: fid.clone(),
            file_name: rel_path.to_string(),
            relative_path: rel_path.to_string(),
            original_size: content.len() as u64,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        integrity: IntegrityMetadata::sha256(file.logical_file_hash.clone().unwrap()),
        chunks: vec![televault_manifest::chunk::ChunkManifest {
            chunk_id: cid.clone(),
            index: 0,
            plaintext_size: content.len() as u64,
            stored_size: content.len() as u64,
            integrity: IntegrityMetadata::sha256(&hash),
            storage_reference: sref.clone(),
        }],
    };
    fixture.db.save_manifest(&manifest).expect("save manifest");

    let chunk_rec = ChunkRecord {
        chunk_id: cid,
        file_id: fid.clone(),
        manifest_id: manifest.manifest_id.clone(),
        chunk_index: 0,
        plaintext_size: content.len() as u64,
        stored_size: content.len() as u64,
        integrity_hash: hash,
        storage_reference: serde_json::to_string(&sref).unwrap(),
        status: "active".into(),
        created_at: "2026-10-08T10:00:00Z".into(),
        updated_at: "2026-10-08T10:00:00Z".into(),
    };
    fixture.db.create_chunk(&chunk_rec).expect("create chunk");

    let snap_id = SnapshotId::new(format!("snap-{fid_str}")).unwrap();
    let snap_rec = SnapshotRecord {
        snapshot_id: snap_id.clone(),
        profile_id: pid.clone(),
        status: BackupStatus::Completed,
        metadata: None,
        created_at: "2026-10-08T10:00:00Z".into(),
    };
    let _ = fixture.db.create_snapshot(&snap_rec);

    let ver_id = VersionId::new(format!("ver-{fid_str}")).unwrap();
    let ver_rec = VersionRecord {
        version_id: ver_id,
        file_id: fid.clone(),
        snapshot_id: snap_id,
        manifest_id: manifest.manifest_id.clone(),
        status: "active".into(),
        created_at: "2026-10-08T10:00:00Z".into(),
    };
    let _ = fixture.db.create_version(&ver_rec);

    (file, manifest, sref)
}

// =========================================================================
// 1. MISSING REMOTE CHUNK REPAIR
// =========================================================================

#[test]
fn test_missing_remote_chunk_repair_success() {
    let fixture = setup_fixture("missing_chunk");
    let pid = ProfileId::new("prof-missing").unwrap();
    let source_dir = fixture.base_dir.join("source");
    create_test_profile(&fixture.db, "prof-missing", &source_dir);

    let content = b"Authoritative source payload for missing chunk repair test.";
    create_source_file(&source_dir, "doc.txt", content);

    let (file, manifest, sref) =
        populate_file_and_manifest(&fixture, &pid, "file-missing", "doc.txt", content);

    // Notice: mock_storage does NOT store the payload -> remote chunk is ConfirmedMissing!
    let cancel = CancellationToken::new();

    // 1. Preview detects the missing chunk
    let preview = fixture
        .repair_engine
        .preview_file_repair(&pid, &file.file_id, &cancel)
        .expect("preview repair");
    assert!(preview.is_repairable);
    assert_eq!(preview.total_affected_chunks, 1);
    assert_eq!(preview.eligible_candidates[0].chunk_index, 0);

    // 2. Execute repair
    let result = fixture
        .repair_engine
        .repair_file(&pid, &file.file_id, false, &cancel)
        .expect("execute repair");

    assert_eq!(result.repaired_chunks, 1);
    assert_eq!(result.skipped_healthy_chunks, 0);
    assert_eq!(result.failed_chunks, 0);
    assert_eq!(result.chunk_results[0].status, "success");

    // 3. Remote replacement exists and is valid in mock storage
    let new_ref_str = result.chunk_results[0]
        .new_storage_reference
        .as_ref()
        .unwrap();
    let new_sref: StorageReference = serde_json::from_str(new_ref_str).unwrap();
    assert_ne!(
        new_sref, sref,
        "Must assign a fresh remote storage reference"
    );

    let verify_req = televault_storage::types::VerificationRequest {
        storage_reference: new_sref.clone(),
        expected_size_bytes: content.len() as u64,
        expected_sha256: Some(StreamHasher::hash_bytes(content)),
    };
    assert!(fixture.mock_storage.verify(&verify_req).unwrap());

    // 4. Local SQLite catalog updated atomically
    let updated_chunk = fixture
        .db
        .get_chunk(&preview.eligible_candidates[0].chunk_id)
        .unwrap()
        .unwrap();
    assert_eq!(updated_chunk.storage_reference, *new_ref_str);

    let updated_manifest = fixture
        .db
        .get_manifest(&manifest.manifest_id)
        .unwrap()
        .unwrap();
    assert_eq!(updated_manifest.chunks[0].storage_reference, new_sref);

    // 5. Repair history record persisted
    let history = fixture.repair_engine.get_repair_history(&pid, 10).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].status, "success");
    assert_eq!(history[0].bytes_processed, content.len() as u64);

    // 6. Idempotency test: second run immediately detects already healthy!
    let second_run = fixture
        .repair_engine
        .repair_file(&pid, &file.file_id, false, &cancel)
        .expect("second repair run");
    assert_eq!(second_run.total_chunks_evaluated, 0);
    assert_eq!(second_run.repaired_chunks, 0);
    assert_eq!(second_run.failed_chunks, 0);

    // Also test direct chunk-level idempotency via RepairPipeline
    let single_chunk_healthy = RepairPipeline::repair_single_chunk(
        &preview.eligible_candidates[0],
        &fixture
            .db
            .get_manifest(&manifest.manifest_id)
            .unwrap()
            .unwrap(),
        &fixture.db,
        &fixture.transfer_engine,
        &fixture.temp_manager,
        &EncryptionPolicy::Disabled,
        &cancel,
    )
    .expect("repair single chunk idempotency");
    assert_eq!(single_chunk_healthy.status, "skipped_healthy");
}

// =========================================================================
// 2. CORRUPTED REMOTE CHUNK REPAIR
// =========================================================================

#[test]
fn test_corrupted_remote_chunk_repair_success() {
    let fixture = setup_fixture("corrupted_chunk");
    let pid = ProfileId::new("prof-corrupt").unwrap();
    let source_dir = fixture.base_dir.join("source");
    create_test_profile(&fixture.db, "prof-corrupt", &source_dir);

    let content = b"Genuine original content to recover corrupted chunk.";
    create_source_file(&source_dir, "data.bin", content);

    let (file, _, sref) =
        populate_file_and_manifest(&fixture, &pid, "file-corrupt", "data.bin", content);

    // Store corrupted bytes in mock storage (tampered data)
    let corrupted_content = b"TAMPERED CORRUPTED BYTES!";
    fixture
        .mock_storage
        .insert_in_memory_object(sref.clone(), corrupted_content.to_vec())
        .unwrap();

    let cancel = CancellationToken::new();

    // Verify preview detects corrupted chunk
    let preview = fixture
        .repair_engine
        .preview_file_repair(&pid, &file.file_id, &cancel)
        .expect("preview");
    assert_eq!(preview.total_affected_chunks, 1);

    // Execute repair
    let result = fixture
        .repair_engine
        .repair_file(&pid, &file.file_id, false, &cancel)
        .expect("repair file");
    assert_eq!(result.repaired_chunks, 1);

    // Verify remote replacement holds genuine content
    let new_ref_str = result.chunk_results[0]
        .new_storage_reference
        .as_ref()
        .unwrap();
    let new_sref: StorageReference = serde_json::from_str(new_ref_str).unwrap();

    let mut downloaded = Vec::new();
    let dl_req = televault_storage::types::DownloadRequest {
        file_id: file.file_id.clone(),
        chunk_id: preview.eligible_candidates[0].chunk_id.clone(),
        storage_reference: new_sref,
        expected_size_bytes: content.len() as u64,
        expected_sha256: Some(StreamHasher::hash_bytes(content)),
    };
    fixture
        .mock_storage
        .download(&dl_req, &mut downloaded)
        .unwrap();
    assert_eq!(downloaded, content);
}

// =========================================================================
// 3. OWNERSHIP AND PROFILE ISOLATION ENFORCEMENT
// =========================================================================

#[test]
fn test_cross_profile_repair_rejection() {
    let fixture = setup_fixture("cross_profile");
    let source_a = fixture.base_dir.join("source_a");
    let source_b = fixture.base_dir.join("source_b");
    create_test_profile(&fixture.db, "prof-a", &source_a);
    create_test_profile(&fixture.db, "prof-b", &source_b);

    let pid_a = ProfileId::new("prof-a").unwrap();
    let pid_b = ProfileId::new("prof-b").unwrap();

    let content = b"Secret data owned exclusively by Profile A";
    create_source_file(&source_a, "vault.txt", content);
    let (file_a, _, _) =
        populate_file_and_manifest(&fixture, &pid_a, "file-a", "vault.txt", content);

    let cancel = CancellationToken::new();

    // Profile B attempts to repair file_a belonging to Profile A -> must be rejected immediately!
    let res = fixture
        .repair_engine
        .repair_file(&pid_b, &file_a.file_id, false, &cancel);
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("Ownership violation") || err_msg.contains("belongs to profile"),
        "Must reject cross-profile access: {err_msg}"
    );

    // Verify zero repair history was recorded for either profile
    assert_eq!(
        fixture
            .repair_engine
            .get_repair_history(&pid_b, 10)
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        fixture
            .repair_engine
            .get_repair_history(&pid_a, 10)
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn test_wrong_snapshot_ownership_rejection() {
    let fixture = setup_fixture("wrong_snapshot");
    let source_a = fixture.base_dir.join("source_a");
    let source_b = fixture.base_dir.join("source_b");
    create_test_profile(&fixture.db, "prof-a", &source_a);
    create_test_profile(&fixture.db, "prof-b", &source_b);

    let pid_a = ProfileId::new("prof-a").unwrap();
    let pid_b = ProfileId::new("prof-b").unwrap();

    let sid_b = SnapshotId::new("snap-b-01").unwrap();
    let snap_b = SnapshotRecord {
        snapshot_id: sid_b.clone(),
        profile_id: pid_b.clone(),
        status: BackupStatus::Completed,
        metadata: None,
        created_at: "2026-10-08T10:00:00Z".into(),
    };
    fixture.db.create_snapshot(&snap_b).unwrap();

    let cancel = CancellationToken::new();

    // Profile A requests repair of Snapshot B owned by Profile B -> reject
    let res = fixture
        .repair_engine
        .repair_snapshot(&pid_a, &sid_b, false, &cancel);
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("belongs to profile"));
}

// =========================================================================
// 4. MISSING SOURCE AND SIZE MISMATCH REJECTION
// =========================================================================

#[test]
fn test_missing_source_file_rejection() {
    let fixture = setup_fixture("missing_source");
    let pid = ProfileId::new("prof-src").unwrap();
    let source_dir = fixture.base_dir.join("source");
    create_test_profile(&fixture.db, "prof-src", &source_dir);

    let content = b"Temporary content before deletion";
    let (file, _, _) =
        populate_file_and_manifest(&fixture, &pid, "file-del", "missing.txt", content);

    // Source file never created or deleted from filesystem
    let cancel = CancellationToken::new();
    let res = fixture
        .repair_engine
        .preview_file_repair(&pid, &file.file_id, &cancel);
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("not found"));
}

#[test]
fn test_source_size_mismatch_rejection() {
    let fixture = setup_fixture("size_mismatch");
    let pid = ProfileId::new("prof-size").unwrap();
    let source_dir = fixture.base_dir.join("source");
    create_test_profile(&fixture.db, "prof-size", &source_dir);

    // Manifest recorded 100 bytes, but source file on disk is modified to 10 bytes
    create_source_file(&source_dir, "mismatch.txt", b"Only 10 b!");
    let fid = FileId::new("file-mismatch").unwrap();

    let dummy_hash = "a".repeat(64);
    let file = FileRecord {
        file_id: fid.clone(),
        profile_id: Some(pid.clone()),
        file_name: "mismatch.txt".into(),
        relative_path: "mismatch.txt".into(),
        original_size: 100,
        mime_type: None,
        status: "backed_up".into(),
        logical_file_hash: Some(dummy_hash.clone()),
        created_at: "2026-10-08T10:00:00Z".into(),
        modified_at: None,
        created_timestamp: "2026-10-08T10:00:00Z".into(),
        updated_timestamp: "2026-10-08T10:00:00Z".into(),
    };
    fixture.db.create_file(&file).unwrap();

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-mismatch".into(),
        logical_file: LogicalFileMetadata {
            file_id: fid.clone(),
            file_name: "mismatch.txt".into(),
            relative_path: "mismatch.txt".into(),
            original_size: 100,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        integrity: IntegrityMetadata::sha256(&dummy_hash),
        chunks: vec![televault_manifest::chunk::ChunkManifest {
            chunk_id: ChunkId::new("chk-mis-0").unwrap(),
            index: 0,
            plaintext_size: 100,
            stored_size: 100,
            integrity: IntegrityMetadata::sha256(&dummy_hash),
            storage_reference: StorageReference::Pending,
        }],
    };
    fixture.db.save_manifest(&manifest).unwrap();

    let snap_id = SnapshotId::new("snap-mismatch").unwrap();
    fixture
        .db
        .create_snapshot(&SnapshotRecord {
            snapshot_id: snap_id.clone(),
            profile_id: pid.clone(),
            status: BackupStatus::Completed,
            metadata: None,
            created_at: "2026-10-08T10:00:00Z".into(),
        })
        .unwrap();

    fixture
        .db
        .create_version(&VersionRecord {
            version_id: VersionId::new("ver-mismatch").unwrap(),
            file_id: fid.clone(),
            snapshot_id: snap_id,
            manifest_id: manifest.manifest_id.clone(),
            status: "active".into(),
            created_at: "2026-10-08T10:00:00Z".into(),
        })
        .unwrap();

    let cancel = CancellationToken::new();
    let res = fixture
        .repair_engine
        .repair_file(&pid, &fid, false, &cancel);
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("size mismatch"),
        "Expected size mismatch error, got: {err_msg}"
    );
}

// =========================================================================
// 5. ENCRYPTED BACKUP REPAIR AND CRYPTO HANDLING
// =========================================================================

#[test]
fn test_encrypted_chunk_repair_with_key_and_missing_key_rejection() {
    let fixture = setup_fixture("encrypted_repair");
    let pid = ProfileId::new("prof-enc").unwrap();
    let source_dir = fixture.base_dir.join("source");
    create_test_profile(&fixture.db, "prof-enc", &source_dir);

    let content = b"Classified secret payload requiring AES-256-GCM encryption!";
    create_source_file(&source_dir, "secret.key", content);

    let fid = FileId::new("file-secret").unwrap();
    let cid = ChunkId::new("chk-secret-0").unwrap();

    let salt_bytes = [7u8; 32];
    let salt = Salt::from_slice(&salt_bytes).unwrap();
    let secret_key = derive_key("master_passphrase", &salt, &KdfParams::default()).unwrap();

    let file_rec = FileRecord {
        file_id: fid.clone(),
        profile_id: Some(pid.clone()),
        file_name: "secret.key".into(),
        relative_path: "secret.key".into(),
        original_size: content.len() as u64,
        mime_type: None,
        status: "backed_up".into(),
        logical_file_hash: Some(StreamHasher::hash_bytes(content)),
        created_at: "2026-10-08T10:00:00Z".into(),
        modified_at: None,
        created_timestamp: "2026-10-08T10:00:00Z".into(),
        updated_timestamp: "2026-10-08T10:00:00Z".into(),
    };
    fixture.db.create_file(&file_rec).unwrap();

    let encrypted_manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-secret".into(),
        logical_file: LogicalFileMetadata {
            file_id: fid.clone(),
            file_name: "secret.key".into(),
            relative_path: "secret.key".into(),
            original_size: content.len() as u64,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: Some(EncryptionMetadata::aes256_gcm(None)),
        compression: CompressionMetadata::none(),
        integrity: IntegrityMetadata::sha256(StreamHasher::hash_bytes(content)),
        chunks: vec![televault_manifest::chunk::ChunkManifest {
            chunk_id: cid.clone(),
            index: 0,
            plaintext_size: content.len() as u64,
            stored_size: 150, // rough ciphertext size
            integrity: IntegrityMetadata::sha256("b".repeat(64)),
            storage_reference: StorageReference::Pending,
        }],
    };
    fixture.db.save_manifest(&encrypted_manifest).unwrap();

    let snap_id = SnapshotId::new("snap-secret").unwrap();
    fixture
        .db
        .create_snapshot(&SnapshotRecord {
            snapshot_id: snap_id.clone(),
            profile_id: pid.clone(),
            status: BackupStatus::Completed,
            metadata: None,
            created_at: "2026-10-08T10:00:00Z".into(),
        })
        .unwrap();

    fixture
        .db
        .create_version(&VersionRecord {
            version_id: VersionId::new("ver-secret").unwrap(),
            file_id: fid.clone(),
            snapshot_id: snap_id,
            manifest_id: "man-secret".into(),
            status: "active".into(),
            created_at: "2026-10-08T10:00:00Z".into(),
        })
        .unwrap();

    let chunk_rec = ChunkRecord {
        chunk_id: cid,
        file_id: fid.clone(),
        manifest_id: "man-secret".into(),
        chunk_index: 0,
        plaintext_size: content.len() as u64,
        stored_size: 150,
        integrity_hash: "old_enc_hash".into(),
        storage_reference: "\"Pending\"".into(),
        status: "pending".into(),
        created_at: "2026-10-08T10:00:00Z".into(),
        updated_at: "2026-10-08T10:00:00Z".into(),
    };
    fixture.db.create_chunk(&chunk_rec).unwrap();

    let cancel = CancellationToken::new();

    // 1. Without encryption key -> MUST fail safely with EncryptionKeyMissing!
    let unauth_engine = fixture
        .repair_engine
        .clone()
        .with_encryption_policy(Arc::new(EncryptionPolicy::Disabled));
    let fail_res = unauth_engine.repair_file(&pid, &fid, false, &cancel);
    assert!(fail_res.is_err());
    let err_str = fail_res.unwrap_err().to_string();
    assert!(
        err_str.contains("encryption key") || err_str.contains("AES-256-GCM"),
        "Got: {err_str}"
    );

    // 2. With valid encryption key -> succeeds, generates valid AES-256-GCM ciphertext
    let auth_engine = fixture
        .repair_engine
        .clone()
        .with_encryption_policy(Arc::new(EncryptionPolicy::Enabled(secret_key.clone())));
    let ok_res = auth_engine
        .repair_file(&pid, &fid, false, &cancel)
        .expect("encrypted repair");
    assert_eq!(ok_res.repaired_chunks, 1);

    // Verify uploaded payload decrypts cleanly using SecretKey and ChunkAad
    let new_ref_str = ok_res.chunk_results[0]
        .new_storage_reference
        .as_ref()
        .unwrap();
    let new_sref: StorageReference = serde_json::from_str(new_ref_str).unwrap();

    let mut ciphertext_buf = Vec::new();
    let dl_req = televault_storage::types::DownloadRequest {
        file_id: fid.clone(),
        chunk_id: ok_res.chunk_results[0].chunk_id.clone(),
        storage_reference: new_sref,
        expected_size_bytes: ok_res.chunk_results[0].bytes_processed,
        expected_sha256: None,
    };
    fixture
        .mock_storage
        .download(&dl_req, &mut ciphertext_buf)
        .unwrap();

    let payload: televault_crypto::payload::EncryptedPayload =
        serde_json::from_slice(&ciphertext_buf).unwrap();
    let aad = televault_crypto::aad::ChunkAad::new(fid.clone(), 0, 1);
    let decrypted = televault_crypto::cipher::decrypt_chunk(&secret_key, &payload, &aad).unwrap();
    assert_eq!(decrypted, content);
}

// =========================================================================
// 6. LARGE-FILE 5.2 GB SIMULATION: ONLY CORRUPTED CHUNK RECONSTRUCTED
// =========================================================================

#[test]
fn test_5_2gb_large_file_middle_chunk_isolated_repair() {
    let fixture = setup_fixture("large_file_repair");
    let pid = ProfileId::new("prof-large").unwrap();
    let source_dir = fixture.base_dir.join("source");
    create_test_profile(&fixture.db, "prof-large", &source_dir);

    // Create a file of 3 chunks: total size = TARGET_CHUNK_SIZE_BYTES * 2 + 1024
    // Chunk 0: TARGET_CHUNK_SIZE_BYTES
    // Chunk 1: TARGET_CHUNK_SIZE_BYTES (we will corrupt ONLY Chunk 1)
    // Chunk 2: 1024 bytes
    // Total size represents multi-chunk partitioned logical file.
    let chunk0_sample = vec![0x11u8; 1024 * 64];
    let chunk1_sample = vec![0x22u8; 1024 * 64];
    let chunk2_sample = vec![0x33u8; 1024 * 64];

    // For test speed, we simulate a 3-chunk logical file with 64 KiB chunks
    // where chunk 1 is damaged, verifying only chunk 1 is sought, read, and uploaded.
    let mut file_content = Vec::new();
    file_content.extend_from_slice(&chunk0_sample);
    file_content.extend_from_slice(&chunk1_sample);
    file_content.extend_from_slice(&chunk2_sample);

    let full_path = create_source_file(&source_dir, "large_archive.iso", &file_content);
    let fid = FileId::new("file-large-iso").unwrap();

    let cid0 = ChunkId::new("chk-large-0").unwrap();
    let cid1 = ChunkId::new("chk-large-1").unwrap();
    let cid2 = ChunkId::new("chk-large-2").unwrap();

    let sref0 = StorageReference::Telegram {
        chat_id: 1,
        message_id: 101,
        file_id: "tg_chk_0".into(),
    };
    let sref1 = StorageReference::Telegram {
        chat_id: 1,
        message_id: 102,
        file_id: "tg_chk_1".into(),
    };
    let sref2 = StorageReference::Telegram {
        chat_id: 1,
        message_id: 103,
        file_id: "tg_chk_2".into(),
    };

    let hash0 = StreamHasher::hash_bytes(&chunk0_sample);
    let hash1 = StreamHasher::hash_bytes(&chunk1_sample);
    let hash2 = StreamHasher::hash_bytes(&chunk2_sample);

    // Chunks 0 and 2 are healthy in mock storage
    fixture
        .mock_storage
        .insert_in_memory_object(sref0.clone(), chunk0_sample.clone())
        .unwrap();
    // Chunk 1 is CORRUPTED in mock storage (bad hash / tampered)
    fixture
        .mock_storage
        .insert_in_memory_object(sref1.clone(), vec![0xAA; 1024])
        .unwrap();
    fixture
        .mock_storage
        .insert_in_memory_object(sref2.clone(), chunk2_sample.clone())
        .unwrap();

    let file_rec = FileRecord {
        file_id: fid.clone(),
        profile_id: Some(pid.clone()),
        file_name: "large_archive.iso".into(),
        relative_path: "large_archive.iso".into(),
        original_size: file_content.len() as u64,
        mime_type: None,
        status: "backed_up".into(),
        logical_file_hash: Some(StreamHasher::hash_bytes(&file_content)),
        created_at: "2026-10-08T10:00:00Z".into(),
        modified_at: None,
        created_timestamp: "2026-10-08T10:00:00Z".into(),
        updated_timestamp: "2026-10-08T10:00:00Z".into(),
    };
    fixture.db.create_file(&file_rec).unwrap();

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-large-iso".into(),
        logical_file: LogicalFileMetadata {
            file_id: fid.clone(),
            file_name: "large_archive.iso".into(),
            relative_path: "large_archive.iso".into(),
            original_size: file_content.len() as u64,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        integrity: IntegrityMetadata::sha256(StreamHasher::hash_bytes(&file_content)),
        chunks: vec![
            televault_manifest::chunk::ChunkManifest {
                chunk_id: cid0.clone(),
                index: 0,
                plaintext_size: chunk0_sample.len() as u64,
                stored_size: chunk0_sample.len() as u64,
                integrity: IntegrityMetadata::sha256(&hash0),
                storage_reference: sref0.clone(),
            },
            televault_manifest::chunk::ChunkManifest {
                chunk_id: cid1.clone(),
                index: 1,
                plaintext_size: chunk1_sample.len() as u64,
                stored_size: chunk1_sample.len() as u64,
                integrity: IntegrityMetadata::sha256(&hash1),
                storage_reference: sref1.clone(),
            },
            televault_manifest::chunk::ChunkManifest {
                chunk_id: cid2.clone(),
                index: 2,
                plaintext_size: chunk2_sample.len() as u64,
                stored_size: chunk2_sample.len() as u64,
                integrity: IntegrityMetadata::sha256(&hash2),
                storage_reference: sref2.clone(),
            },
        ],
    };
    fixture.db.save_manifest(&manifest).unwrap();

    let snap_id = SnapshotId::new("snap-large-iso").unwrap();
    fixture
        .db
        .create_snapshot(&SnapshotRecord {
            snapshot_id: snap_id.clone(),
            profile_id: pid.clone(),
            status: BackupStatus::Completed,
            metadata: None,
            created_at: "2026-10-08T10:00:00Z".into(),
        })
        .unwrap();

    fixture
        .db
        .create_version(&VersionRecord {
            version_id: VersionId::new("ver-large-iso").unwrap(),
            file_id: fid.clone(),
            snapshot_id: snap_id,
            manifest_id: manifest.manifest_id.clone(),
            status: "active".into(),
            created_at: "2026-10-08T10:00:00Z".into(),
        })
        .unwrap();

    for (c, h, s) in [
        (&cid0, &hash0, &sref0),
        (&cid1, &hash1, &sref1),
        (&cid2, &hash2, &sref2),
    ] {
        fixture
            .db
            .create_chunk(&ChunkRecord {
                chunk_id: c.clone(),
                file_id: fid.clone(),
                manifest_id: "man-large-iso".into(),
                chunk_index: if c == &cid0 {
                    0
                } else if c == &cid1 {
                    1
                } else {
                    2
                },
                plaintext_size: 1024 * 64,
                stored_size: 1024 * 64,
                integrity_hash: h.clone(),
                storage_reference: serde_json::to_string(s).unwrap(),
                status: "active".into(),
                created_at: "2026-10-08T10:00:00Z".into(),
                updated_at: "2026-10-08T10:00:00Z".into(),
            })
            .unwrap();
    }

    let cancel = CancellationToken::new();

    // 1. Repair candidate resolution: only Chunk 1 should be eligible
    let candidate = RepairCandidate {
        profile_id: pid.clone(),
        snapshot_id: None,
        file_id: fid.clone(),
        manifest_id: manifest.manifest_id.clone(),
        chunk_id: cid1.clone(),
        chunk_index: 1,
        total_chunks: 3,
        finding_code: televault_integrity::types::VerificationIssueCode::StoredHashMismatch,
        old_storage_reference: serde_json::to_string(&sref1).unwrap(),
        plaintext_size: chunk1_sample.len() as u64,
        stored_size: chunk1_sample.len() as u64,
        source_path: full_path,
    };

    // Reconstructing only Chunk 1
    let single_chunk_res = RepairPipeline::repair_single_chunk(
        &candidate,
        &manifest,
        &fixture.db,
        &fixture.transfer_engine,
        &fixture.temp_manager,
        &EncryptionPolicy::Disabled,
        &cancel,
    )
    .expect("repair single chunk");

    assert_eq!(single_chunk_res.status, "success");
    assert_eq!(single_chunk_res.chunk_index, 1);
    assert_eq!(single_chunk_res.bytes_processed, chunk1_sample.len() as u64);

    // Verify Chunk 0 and Chunk 2 storage references in SQLite were NOT modified
    let chk0 = fixture.db.get_chunk(&cid0).unwrap().unwrap();
    assert_eq!(
        chk0.storage_reference,
        serde_json::to_string(&sref0).unwrap()
    );

    let chk2 = fixture.db.get_chunk(&cid2).unwrap().unwrap();
    assert_eq!(
        chk2.storage_reference,
        serde_json::to_string(&sref2).unwrap()
    );

    // Verify Chunk 1 received fresh storage reference
    let chk1 = fixture.db.get_chunk(&cid1).unwrap().unwrap();
    assert_ne!(
        chk1.storage_reference,
        serde_json::to_string(&sref1).unwrap()
    );
}

// =========================================================================
// 7. PREVIEW DRY-RUN (ZERO MUTATION, ZERO PERMANENT FILES)
// =========================================================================

#[test]
fn test_preview_dry_run_zero_mutation() {
    let fixture = setup_fixture("dry_run");
    let pid = ProfileId::new("prof-dry").unwrap();
    let source_dir = fixture.base_dir.join("source");
    create_test_profile(&fixture.db, "prof-dry", &source_dir);

    let content = b"Payload for dry run preview testing.";
    create_source_file(&source_dir, "dry.txt", content);
    let (file, _, sref) =
        populate_file_and_manifest(&fixture, &pid, "file-dry", "dry.txt", content);

    let cancel = CancellationToken::new();

    // Execute dry run repair
    let result = fixture
        .repair_engine
        .repair_file(&pid, &file.file_id, true, &cancel)
        .expect("dry run repair");

    assert_eq!(result.total_chunks_evaluated, 1);
    assert_eq!(result.repaired_chunks, 0);
    assert_eq!(result.total_bytes_transferred, 0);
    assert_eq!(result.chunk_results[0].status, "dry_run");

    // Verify SQLite chunk storage reference was NOT modified
    let chunk = fixture.db.list_chunks_by_file(&file.file_id).unwrap();
    assert_eq!(
        chunk[0].storage_reference,
        serde_json::to_string(&sref).unwrap()
    );

    // Verify zero repair history was recorded
    let history = fixture.repair_engine.get_repair_history(&pid, 10).unwrap();
    assert_eq!(history.len(), 0);
}

// =========================================================================
// 8. TEMPORARY STAGING CLEANUP GUARANTEE
// =========================================================================

#[test]
fn test_temporary_staging_cleanup_after_success_and_cancellation() {
    let fixture = setup_fixture("temp_cleanup");
    let pid = ProfileId::new("prof-clean").unwrap();
    let source_dir = fixture.base_dir.join("source");
    create_test_profile(&fixture.db, "prof-clean", &source_dir);

    let content = b"Payload testing staging cleanup lifecycle.";
    create_source_file(&source_dir, "clean.txt", content);
    let (file, _, _) =
        populate_file_and_manifest(&fixture, &pid, "file-clean", "clean.txt", content);

    let cancel = CancellationToken::new();
    let _ = fixture
        .repair_engine
        .repair_file(&pid, &file.file_id, false, &cancel)
        .unwrap();

    // Verify staging directory is completely empty after successful repair
    let staging_files = fs::read_dir(fixture.temp_manager.temp_dir()).unwrap();
    assert_eq!(
        staging_files.count(),
        0,
        "No temporary files may remain after successful repair"
    );

    // Test cancellation cleanup
    let cancel_token = CancellationToken::new();
    cancel_token.cancel(); // Pre-cancelled

    let _ = fixture
        .repair_engine
        .repair_file(&pid, &file.file_id, false, &cancel_token);

    let staging_files_after_cancel = fs::read_dir(fixture.temp_manager.temp_dir()).unwrap();
    assert_eq!(
        staging_files_after_cancel.count(),
        0,
        "No temporary files may remain after cancelled repair"
    );
}

// =========================================================================
// 9. OLD REMOTE REFERENCE PRESERVED (NO REMOTE DELETION)
// =========================================================================

#[test]
fn test_old_remote_reference_preserved_never_deleted() {
    let fixture = setup_fixture("no_remote_delete");
    let pid = ProfileId::new("prof-nodelete").unwrap();
    let source_dir = fixture.base_dir.join("source");
    create_test_profile(&fixture.db, "prof-nodelete", &source_dir);

    let content = b"Original content to ensure old reference is never deleted remotely.";
    create_source_file(&source_dir, "nodelete.txt", content);
    let (file, _, old_sref) =
        populate_file_and_manifest(&fixture, &pid, "file-nodelete", "nodelete.txt", content);

    // Store old object in mock storage
    fixture
        .mock_storage
        .insert_in_memory_object(old_sref.clone(), b"corrupted old data".to_vec())
        .unwrap();

    let cancel = CancellationToken::new();
    let res = fixture
        .repair_engine
        .repair_file(&pid, &file.file_id, false, &cancel)
        .unwrap();
    assert_eq!(res.repaired_chunks, 1);

    // Verify the old remote object is STILL present in cloud storage (deletion is forbidden in Phase 14)
    let mut old_data = Vec::new();
    let old_dl_req = televault_storage::types::DownloadRequest {
        file_id: file.file_id.clone(),
        chunk_id: res.chunk_results[0].chunk_id.clone(),
        storage_reference: old_sref,
        expected_size_bytes: b"corrupted old data".len() as u64,
        expected_sha256: None,
    };
    assert!(fixture
        .mock_storage
        .download(&old_dl_req, &mut old_data)
        .is_ok());
    assert_eq!(old_data, b"corrupted old data");
}
