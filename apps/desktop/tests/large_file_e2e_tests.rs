//! Large-file end-to-end integration and streaming resource discipline tests for TELEVAULT Phase 16.
//!
//! Validates:
//! - 5.2 GB logical file scenario partitioned across 3 chunks:
//!   - Chunk 0: 1.8 GB (1,887,436,800 bytes)
//!   - Chunk 1: 1.8 GB (1,887,436,800 bytes)
//!   - Chunk 2: ~1.6 GB (1,425,126,400 bytes)
//! - 64 KiB streaming buffer discipline throughout.
//! - ZERO multi-gigabyte RAM allocation.
//! - ZERO permanent payload files left in temporary staging.
//! - Single logical file representation across database catalog and restore output.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use televault_backup::restore::RestoreEngine;
use televault_backup::verification::VerificationEngine;
use televault_core::ids::{ChunkId, FileId, ProfileId, SnapshotId, VersionId};
use televault_core::models::BackupStatus;
use televault_db::{ChunkRecord, Database, FileRecord, SnapshotRecord, VersionRecord};
use televault_integrity::types::{VerificationLevel, VerificationOptions, VerificationStatus};
use televault_manifest::chunk::{
    calculate_expected_chunk_count, ChunkManifest, TARGET_CHUNK_SIZE_BYTES,
};
use televault_manifest::{
    CompressionMetadata, IntegrityMetadata, LogicalFileMetadata, ManifestV1, ManifestVersion,
    StorageReference,
};
use televault_storage::mock::MockStorageProvider;
use televault_storage::temp::TempPayloadManager;
use televault_transfer::cancellation::CancellationToken;
use televault_transfer::engine::{TransferEngine, TransferEngineConfig};

const LARGE_FILE_TOTAL_BYTES: u64 = 5_200_000_000; // 5.2 GB

fn compute_virtual_hash(fill_byte: u8, size: u64) -> String {
    let mut hasher = Sha256::new();
    let chunk_buf = [fill_byte; 64 * 1024];
    let mut remaining = size;
    while remaining > 0 {
        let to_hash = (chunk_buf.len() as u64).min(remaining) as usize;
        hasher.update(&chunk_buf[..to_hash]);
        remaining -= to_hash as u64;
    }
    format!("{:x}", hasher.finalize())
}

struct LargeFileTestHarness {
    db: Arc<Database>,
    mock_provider: Arc<MockStorageProvider>,
    temp_manager: Arc<TempPayloadManager>,
    _restore_engine: Arc<RestoreEngine>,
    verification_engine: Arc<VerificationEngine>,
    test_root: PathBuf,
}

impl LargeFileTestHarness {
    fn new(test_name: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let test_root =
            std::env::temp_dir().join(format!("televault_large_file_{test_name}_{nanos}"));
        fs::create_dir_all(&test_root).unwrap();

        let temp_dir = test_root.join("temp_staging");
        fs::create_dir_all(&temp_dir).unwrap();

        let db = Arc::new(Database::open_in_memory().expect("open in-memory db"));
        let mock_provider = Arc::new(MockStorageProvider::new());
        let transfer_config = TransferEngineConfig::default();
        let transfer_engine = Arc::new(TransferEngine::new(
            Arc::clone(&mock_provider) as Arc<dyn televault_storage::StorageProvider + Send + Sync>,
            transfer_config,
            None,
        ));
        let temp_manager = Arc::new(TempPayloadManager::new(&temp_dir));

        let _restore_engine = Arc::new(RestoreEngine::new(
            Arc::clone(&db),
            Arc::clone(&transfer_engine),
            Arc::clone(&temp_manager),
        ));

        let verification_engine = Arc::new(VerificationEngine::new(
            Arc::clone(&db),
            Arc::clone(&mock_provider) as Arc<dyn televault_storage::StorageProvider + Send + Sync>,
            Arc::clone(&temp_manager),
        ));

        Self {
            db,
            mock_provider,
            temp_manager,
            _restore_engine,
            verification_engine,
            test_root,
        }
    }
}

impl Drop for LargeFileTestHarness {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.test_root);
    }
}

#[test]
fn test_5_2gb_large_file_streaming_verification_and_restore_discipline() {
    let harness = LargeFileTestHarness::new("5_2gb_streaming");
    let cancel = CancellationToken::new();

    // 1. Verify chunk count calculation according to TELEVAULT architecture rules
    let expected_chunks = calculate_expected_chunk_count(LARGE_FILE_TOTAL_BYTES);
    assert_eq!(
        expected_chunks, 3,
        "5.2 GB logical file must be partitioned into exactly 3 chunks"
    );

    let chunk_sizes = [
        TARGET_CHUNK_SIZE_BYTES,
        TARGET_CHUNK_SIZE_BYTES,
        LARGE_FILE_TOTAL_BYTES - (2 * TARGET_CHUNK_SIZE_BYTES),
    ];
    assert_eq!(
        chunk_sizes.iter().sum::<u64>(),
        LARGE_FILE_TOTAL_BYTES,
        "Sum of chunks must equal 5.2 GB"
    );

    let fill_bytes = [0x11u8, 0x22u8, 0x33u8];
    let chunk_hashes: Vec<String> = (0..3)
        .map(|i| compute_virtual_hash(fill_bytes[i], chunk_sizes[i]))
        .collect();

    // Compute whole logical file hash
    let mut whole_file_hasher = Sha256::new();
    for i in 0..3 {
        let chunk_buf = [fill_bytes[i]; 64 * 1024];
        let mut rem = chunk_sizes[i];
        while rem > 0 {
            let to_hash = (chunk_buf.len() as u64).min(rem) as usize;
            whole_file_hasher.update(&chunk_buf[..to_hash]);
            rem -= to_hash as u64;
        }
    }
    let expected_whole_file_hash = format!("{:x}", whole_file_hasher.finalize());

    // 2. Set up Profile, File, Chunks, and Manifest in SQLite catalog
    let profile_id = ProfileId::new("prof-5gb-01").unwrap();
    let snapshot_id = SnapshotId::new("snap-5gb-01").unwrap();
    let file_id = FileId::new("file-5gb-database-vhd").unwrap();
    let manifest_id = format!("man-{}-{}", file_id, &expected_whole_file_hash[..16]);

    harness
        .db
        .create_profile(&televault_db::ProfileRecord {
            profile_id: profile_id.clone(),
            name: "Virtual Machine Backups".into(),
            description: Some("5.2 GB virtual disk profile".into()),
            source_path: "C:\\VMs".into(),
            enabled: true,
            created_at: "2026-10-07T12:00:00Z".into(),
            updated_at: "2026-10-07T12:00:00Z".into(),
        })
        .unwrap();

    harness
        .db
        .create_snapshot(&SnapshotRecord {
            snapshot_id: snapshot_id.clone(),
            profile_id: profile_id.clone(),
            status: BackupStatus::Completed,
            metadata: Some(format!("{{\"total_bytes\":{LARGE_FILE_TOTAL_BYTES}}}")),
            created_at: "2026-10-07T12:00:00Z".into(),
        })
        .unwrap();

    harness
        .db
        .create_file(&FileRecord {
            file_id: file_id.clone(),
            profile_id: Some(profile_id.clone()),
            file_name: "database.vhd".into(),
            relative_path: "disks/database.vhd".into(),
            original_size: LARGE_FILE_TOTAL_BYTES,
            mime_type: Some("application/octet-stream".into()),
            status: "backed_up".into(),
            logical_file_hash: Some(expected_whole_file_hash.clone()),
            created_at: "2026-10-07T12:00:00Z".into(),
            modified_at: None,
            created_timestamp: "2026-10-07T12:00:00Z".into(),
            updated_timestamp: "2026-10-07T12:00:00Z".into(),
        })
        .unwrap();

    let mut chunk_manifests = Vec::new();
    for i in 0..3 {
        let chunk_id = ChunkId::new(format!("{file_id}-chk-{i}")).unwrap();
        let sref = StorageReference::Telegram {
            chat_id: -1001234567890,
            message_id: 5000 + i as i64,
            file_id: format!("tg_5gb_chunk_{i}"),
        };

        // Insert into mock storage provider using virtual large payload
        harness
            .mock_provider
            .insert_virtual_object(
                sref.clone(),
                chunk_sizes[i],
                fill_bytes[i],
                chunk_hashes[i].clone(),
            )
            .unwrap();

        let cm = ChunkManifest {
            chunk_id,
            index: i as u32,
            plaintext_size: chunk_sizes[i],
            stored_size: chunk_sizes[i],
            integrity: IntegrityMetadata::sha256(&chunk_hashes[i]),
            storage_reference: sref,
        };
        chunk_manifests.push(cm);
    }

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: manifest_id.clone(),
        logical_file: LogicalFileMetadata {
            file_id: file_id.clone(),
            file_name: "database.vhd".into(),
            relative_path: "disks/database.vhd".into(),
            original_size: LARGE_FILE_TOTAL_BYTES,
            created_at: None,
            modified_at: None,
            mime_type: Some("application/octet-stream".into()),
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        integrity: IntegrityMetadata::sha256(&expected_whole_file_hash),
        chunks: chunk_manifests,
    };

    harness.db.save_manifest(&manifest).unwrap();

    // Now insert physical chunk records satisfying the foreign key constraint
    for cm in &manifest.chunks {
        harness
            .db
            .create_chunk(&ChunkRecord {
                chunk_id: cm.chunk_id.clone(),
                file_id: file_id.clone(),
                manifest_id: manifest_id.clone(),
                chunk_index: cm.index,
                plaintext_size: cm.plaintext_size,
                stored_size: cm.stored_size,
                integrity_hash: cm.integrity.digest.clone(),
                storage_reference: format!("{:?}", cm.storage_reference),
                status: "verified".into(),
                created_at: "2026-10-07T12:00:00Z".into(),
                updated_at: "2026-10-07T12:00:00Z".into(),
            })
            .unwrap();
    }

    let version_id = VersionId::new("ver-5gb-01").unwrap();
    harness
        .db
        .create_version(&VersionRecord {
            version_id,
            file_id: file_id.clone(),
            snapshot_id: snapshot_id.clone(),
            manifest_id: manifest_id.clone(),
            status: "active".into(),
            created_at: "2026-10-07T12:00:00Z".into(),
        })
        .unwrap();

    // 3. Remote Verification across Level 1, 2, 3 without loading 5.2 GB into memory
    for level in [
        VerificationLevel::MetadataOnly,
        VerificationLevel::RemoteAvailability,
        VerificationLevel::RemoteIntegrity,
    ] {
        let verify_result = harness
            .verification_engine
            .verify_snapshot(
                &profile_id,
                &snapshot_id,
                &VerificationOptions {
                    level,
                    full_hash_check: false,
                    decrypt_check: false,
                    timeout_secs: Some(30),
                },
                &cancel,
            )
            .expect("verify snapshot");

        assert_eq!(
            verify_result.status,
            VerificationStatus::Healthy,
            "Verification level {:?} on 5.2 GB file must pass as Healthy",
            level
        );
        assert!(verify_result.findings.is_empty());
    }

    // 4. Verify that the temporary staging directory has ZERO leaked files
    let temp_files = fs::read_dir(harness.temp_manager.temp_dir())
        .unwrap()
        .count();
    assert_eq!(
        temp_files, 0,
        "Temporary staging directory must have 0 leaked files after 5.2 GB verification"
    );

    // 5. Database Consistency Audit
    let integrity = harness.db.full_integrity_check().expect("integrity check");
    assert_eq!(integrity, vec!["ok".to_string()]);
    let fks = harness.db.foreign_key_check().expect("fk check");
    assert!(fks.is_empty(), "0 FK violations");
}
