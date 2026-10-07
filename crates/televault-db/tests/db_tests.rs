//! Comprehensive database integration tests for TELEVAULT embedded persistence.

use televault_core::ids::{ChunkId, FileId, JobId, ProfileId, SnapshotId, VersionId};
use televault_core::models::{BackupStatus, TransferDirection, TransferStatus};
use televault_db::{
    ChunkRecord, Database, FileRecord, HealthStatus, ProfileRecord, SnapshotRecord,
    TransferJobRecord, VersionRecord,
};
use televault_manifest::{
    calculate_expected_chunk_count, ChunkManifest, CompressionMetadata, EncryptionMetadata,
    IntegrityMetadata, KdfInfo, LogicalFileMetadata, ManifestV1, ManifestVersion, StorageReference,
    TARGET_CHUNK_SIZE_BYTES,
};

#[test]
fn test_database_in_memory_initialization_and_health() {
    let db = Database::open_in_memory().expect("open in memory db");
    let health: HealthStatus = db.health_check().expect("health check");

    assert!(health.is_healthy);
    assert!(health.foreign_keys_enabled);
    assert_eq!(health.integrity_check, "ok");
    assert!(health.applied_migrations >= 1);
    assert_eq!(health.total_profiles, 0);
    assert_eq!(health.total_files, 0);
    assert_eq!(health.total_chunks, 0);
}

#[test]
fn test_profile_crud_and_uniqueness() {
    let db = Database::open_in_memory().expect("open db");

    let pid = ProfileId::new("prof-personal").unwrap();
    let profile = ProfileRecord {
        profile_id: pid.clone(),
        name: "Personal Documents".into(),
        description: Some("Important docs".into()),
        source_path: "D:\\Documents".into(),
        enabled: true,
        created_at: "2026-10-07T10:00:00Z".into(),
        updated_at: "2026-10-07T10:00:00Z".into(),
    };

    // 1. Create
    db.create_profile(&profile).expect("create profile");

    // 2. Read
    let fetched = db.get_profile(&pid).expect("get profile").expect("found");
    assert_eq!(fetched, profile);

    // 3. Duplicate profile_id rejection
    let dup_err = db.create_profile(&profile);
    assert!(dup_err.is_err(), "Duplicate profile_id must be rejected");

    // 4. Duplicate name rejection
    let dup_name_pid = ProfileId::new("prof-different").unwrap();
    let dup_name_profile = ProfileRecord {
        profile_id: dup_name_pid,
        name: "Personal Documents".into(), // Duplicate name
        description: None,
        source_path: "D:\\Other".into(),
        enabled: true,
        created_at: "2026-10-07T10:00:00Z".into(),
        updated_at: "2026-10-07T10:00:00Z".into(),
    };
    assert!(
        db.create_profile(&dup_name_profile).is_err(),
        "Duplicate profile name must be rejected"
    );

    // 5. Update
    let mut updated = profile.clone();
    updated.description = Some("Updated docs description".into());
    updated.enabled = false;
    db.update_profile(&updated).expect("update profile");

    let fetched_updated = db.get_profile(&pid).expect("get").expect("found");
    assert_eq!(
        fetched_updated.description,
        Some("Updated docs description".into())
    );
    assert!(!fetched_updated.enabled);

    // 6. Set enabled
    db.set_profile_enabled(&pid, true).expect("set enabled");
    assert!(db.get_profile(&pid).unwrap().unwrap().enabled);

    // 7. List
    let profiles = db.list_profiles().expect("list");
    assert_eq!(profiles.len(), 1);

    // 8. Delete
    db.delete_profile(&pid).expect("delete");
    assert!(db.get_profile(&pid).unwrap().is_none());
}

#[test]
fn test_logical_file_and_5_2gb_three_chunks_regression() {
    let db = Database::open_in_memory().expect("open db");

    let pid = ProfileId::new("prof-videos").unwrap();
    let profile = ProfileRecord {
        profile_id: pid.clone(),
        name: "Video Vault".into(),
        description: None,
        source_path: "D:\\Videos".into(),
        enabled: true,
        created_at: "2026-10-07T10:00:00Z".into(),
        updated_at: "2026-10-07T10:00:00Z".into(),
    };
    db.create_profile(&profile).expect("create profile");

    // A 5.2 GB logical file
    let total_file_size: u64 = 5_583_457_485; // ~5.2 GB
    let file_id = FileId::new("file-video-01").unwrap();
    let logical_file = FileRecord {
        file_id: file_id.clone(),
        profile_id: Some(pid.clone()),
        file_name: "MyHolidayVideo.mp4".into(),
        relative_path: "2026/MyHolidayVideo.mp4".into(),
        original_size: total_file_size,
        mime_type: Some("video/mp4".into()),
        status: "backed_up".into(),
        logical_file_hash: Some(
            "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890".into(),
        ),
        created_at: "2026-10-07T10:00:00Z".into(),
        modified_at: Some("2026-10-07T10:00:00Z".into()),
        created_timestamp: "2026-10-07T10:00:00Z".into(),
        updated_timestamp: "2026-10-07T10:00:00Z".into(),
    };

    // 1. Insert ONE logical file
    db.create_file(&logical_file).expect("create file");

    // Verify exactly ONE logical file exists in the database
    assert_eq!(db.count_files().expect("count"), 1);
    let files = db.list_files_by_profile(Some(&pid)).expect("list");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].original_size, total_file_size);
    assert_eq!(files[0].file_name, "MyHolidayVideo.mp4");

    // Calculate chunks: 5.2 GB split into target 1.8 GB chunks
    let expected_chunks = calculate_expected_chunk_count(total_file_size);
    assert_eq!(expected_chunks, 3);

    let c0_size = TARGET_CHUNK_SIZE_BYTES; // 1.8 GB
    let c1_size = TARGET_CHUNK_SIZE_BYTES; // 1.8 GB
    let c2_size = total_file_size - (c0_size * 2); // remainder
    assert_eq!(c0_size + c1_size + c2_size, total_file_size);

    // Create manifest record
    let manifest_id = "man-video-01".to_string();
    let manifest_v1 = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: manifest_id.clone(),
        logical_file: LogicalFileMetadata {
            file_id: file_id.clone(),
            file_name: "MyHolidayVideo.mp4".into(),
            relative_path: "2026/MyHolidayVideo.mp4".into(),
            original_size: total_file_size,
            created_at: Some(1700000000000),
            modified_at: Some(1700000000000),
            mime_type: Some("video/mp4".into()),
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        integrity: IntegrityMetadata::sha256(
            "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
        ),
        chunks: vec![
            ChunkManifest {
                chunk_id: ChunkId::new("chunk-0").unwrap(),
                index: 0,
                plaintext_size: c0_size,
                stored_size: c0_size,
                integrity: IntegrityMetadata::sha256(
                    "1111111111111111111111111111111111111111111111111111111111111111",
                ),
                storage_reference: StorageReference::LocalStaging {
                    relative_path: "temp/0".into(),
                },
            },
            ChunkManifest {
                chunk_id: ChunkId::new("chunk-1").unwrap(),
                index: 1,
                plaintext_size: c1_size,
                stored_size: c1_size,
                integrity: IntegrityMetadata::sha256(
                    "2222222222222222222222222222222222222222222222222222222222222222",
                ),
                storage_reference: StorageReference::LocalStaging {
                    relative_path: "temp/1".into(),
                },
            },
            ChunkManifest {
                chunk_id: ChunkId::new("chunk-2").unwrap(),
                index: 2,
                plaintext_size: c2_size,
                stored_size: c2_size,
                integrity: IntegrityMetadata::sha256(
                    "3333333333333333333333333333333333333333333333333333333333333333",
                ),
                storage_reference: StorageReference::LocalStaging {
                    relative_path: "temp/2".into(),
                },
            },
        ],
    };
    manifest_v1.validate().expect("manifest validate");
    db.save_manifest(&manifest_v1).expect("save manifest");

    // Insert the 3 physical chunks in OUT OF ORDER sequence (to test ordering invariant)
    let chunk2 = ChunkRecord {
        chunk_id: ChunkId::new("chunk-vid-02").unwrap(),
        file_id: file_id.clone(),
        manifest_id: manifest_id.clone(),
        chunk_index: 2,
        plaintext_size: c2_size,
        stored_size: c2_size,
        integrity_hash: "3333333333333333333333333333333333333333333333333333333333333333".into(),
        storage_reference: "telegram:msg_202".into(),
        status: "uploaded".into(),
        created_at: "2026-10-07T10:00:00Z".into(),
        updated_at: "2026-10-07T10:00:00Z".into(),
    };
    let chunk0 = ChunkRecord {
        chunk_id: ChunkId::new("chunk-vid-00").unwrap(),
        file_id: file_id.clone(),
        manifest_id: manifest_id.clone(),
        chunk_index: 0,
        plaintext_size: c0_size,
        stored_size: c0_size,
        integrity_hash: "1111111111111111111111111111111111111111111111111111111111111111".into(),
        storage_reference: "telegram:msg_200".into(),
        status: "uploaded".into(),
        created_at: "2026-10-07T10:00:00Z".into(),
        updated_at: "2026-10-07T10:00:00Z".into(),
    };
    let chunk1 = ChunkRecord {
        chunk_id: ChunkId::new("chunk-vid-01").unwrap(),
        file_id: file_id.clone(),
        manifest_id: manifest_id.clone(),
        chunk_index: 1,
        plaintext_size: c1_size,
        stored_size: c1_size,
        integrity_hash: "2222222222222222222222222222222222222222222222222222222222222222".into(),
        storage_reference: "telegram:msg_201".into(),
        status: "uploaded".into(),
        created_at: "2026-10-07T10:00:00Z".into(),
        updated_at: "2026-10-07T10:00:00Z".into(),
    };

    // Insert out of order: chunk 2, chunk 0, chunk 1
    db.create_chunk(&chunk2).expect("create chunk 2");
    db.create_chunk(&chunk0).expect("create chunk 0");
    db.create_chunk(&chunk1).expect("create chunk 1");

    // CRITICAL INVARIANT:
    // Exactly 1 logical file exists in the database
    assert_eq!(db.count_files().expect("count"), 1);
    // Exactly 3 chunks exist in the database
    assert_eq!(db.count_chunks_for_file(&file_id).expect("count"), 3);

    // CRITICAL ORDERING INVARIANT:
    // list_chunks_by_file MUST return chunks sorted by chunk_index 0, 1, 2 deterministically!
    let chunks = db.list_chunks_by_file(&file_id).expect("list chunks");
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].chunk_index, 0);
    assert_eq!(chunks[1].chunk_index, 1);
    assert_eq!(chunks[2].chunk_index, 2);
    assert_eq!(chunks[0].plaintext_size, c0_size);
    assert_eq!(chunks[1].plaintext_size, c1_size);
    assert_eq!(chunks[2].plaintext_size, c2_size);

    // Reassembly summation invariant
    let reassembled_total: u64 = chunks.iter().map(|c| c.plaintext_size).sum();
    assert_eq!(reassembled_total, total_file_size);
}

#[test]
fn test_duplicate_chunk_index_rejection() {
    let db = Database::open_in_memory().expect("open db");

    let file_id = FileId::new("file-dup-idx").unwrap();
    let file = FileRecord {
        file_id: file_id.clone(),
        profile_id: None,
        file_name: "test.bin".into(),
        relative_path: "test.bin".into(),
        original_size: 1000,
        mime_type: None,
        status: "tracked".into(),
        logical_file_hash: None,
        created_at: "2026-10-07T10:00:00Z".into(),
        modified_at: None,
        created_timestamp: "2026-10-07T10:00:00Z".into(),
        updated_timestamp: "2026-10-07T10:00:00Z".into(),
    };
    db.create_file(&file).expect("create file");

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-dup".into(),
        logical_file: LogicalFileMetadata {
            file_id: file_id.clone(),
            file_name: "test.bin".into(),
            relative_path: "test.bin".into(),
            original_size: 1000,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        integrity: IntegrityMetadata::sha256(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ),
        chunks: vec![ChunkManifest {
            chunk_id: ChunkId::new("chunk-0").unwrap(),
            index: 0,
            plaintext_size: 1000,
            stored_size: 1000,
            integrity: IntegrityMetadata::sha256(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ),
            storage_reference: StorageReference::Pending,
        }],
    };
    db.save_manifest(&manifest).expect("save manifest");

    let c0 = ChunkRecord {
        chunk_id: ChunkId::new("chunk-a").unwrap(),
        file_id: file_id.clone(),
        manifest_id: "man-dup".into(),
        chunk_index: 0,
        plaintext_size: 1000,
        stored_size: 1000,
        integrity_hash: "aaa".into(),
        storage_reference: "pending".into(),
        status: "pending".into(),
        created_at: "2026-10-07T10:00:00Z".into(),
        updated_at: "2026-10-07T10:00:00Z".into(),
    };
    db.create_chunk(&c0).expect("create chunk 0");

    // Second chunk with SAME chunk_index (0) for same file must fail
    let c0_duplicate = ChunkRecord {
        chunk_id: ChunkId::new("chunk-b").unwrap(),
        file_id: file_id.clone(),
        manifest_id: "man-dup".into(),
        chunk_index: 0, // Duplicate index!
        plaintext_size: 1000,
        stored_size: 1000,
        integrity_hash: "aaa".into(),
        storage_reference: "pending".into(),
        status: "pending".into(),
        created_at: "2026-10-07T10:00:00Z".into(),
        updated_at: "2026-10-07T10:00:00Z".into(),
    };
    let err = db.create_chunk(&c0_duplicate);
    assert!(
        err.is_err(),
        "Duplicate chunk_index must trigger SQLite constraint violation"
    );
}

#[test]
fn test_foreign_key_enforcement() {
    let db = Database::open_in_memory().expect("open db");

    // Attempting to create a chunk for non-existent file must fail with foreign key violation
    let orphan_chunk = ChunkRecord {
        chunk_id: ChunkId::new("chunk-orphan").unwrap(),
        file_id: FileId::new("file-does-not-exist").unwrap(),
        manifest_id: "man-none".into(),
        chunk_index: 0,
        plaintext_size: 500,
        stored_size: 500,
        integrity_hash: "hash".into(),
        storage_reference: "none".into(),
        status: "pending".into(),
        created_at: "2026-10-07T10:00:00Z".into(),
        updated_at: "2026-10-07T10:00:00Z".into(),
    };
    assert!(
        db.create_chunk(&orphan_chunk).is_err(),
        "Foreign key constraint must reject chunk with non-existent file_id"
    );

    // Profile deletion restriction when snapshot references it
    let pid = ProfileId::new("prof-snapped").unwrap();
    db.create_profile(&ProfileRecord {
        profile_id: pid.clone(),
        name: "Snap Profile".into(),
        description: None,
        source_path: "D:\\Source".into(),
        enabled: true,
        created_at: "2026-10-07T10:00:00Z".into(),
        updated_at: "2026-10-07T10:00:00Z".into(),
    })
    .expect("create profile");

    let snap_id = SnapshotId::new("snap-01").unwrap();
    db.create_snapshot(&SnapshotRecord {
        snapshot_id: snap_id,
        profile_id: pid.clone(),
        status: BackupStatus::Completed,
        metadata: None,
        created_at: "2026-10-07T10:00:00Z".into(),
    })
    .expect("create snapshot");

    // Deleting profile must fail because snapshot references it (ON DELETE RESTRICT)
    assert!(
        db.delete_profile(&pid).is_err(),
        "Cannot delete profile when snapshots reference it (ON DELETE RESTRICT)"
    );
}

#[test]
fn test_manifest_roundtrip_with_optional_encryption() {
    let db = Database::open_in_memory().expect("open db");

    // 1. Unencrypted Manifest Roundtrip
    let fid_unenc = FileId::new("file-unenc").unwrap();
    let file_unenc = FileRecord {
        file_id: fid_unenc.clone(),
        profile_id: None,
        file_name: "public.txt".into(),
        relative_path: "public.txt".into(),
        original_size: 120,
        mime_type: Some("text/plain".into()),
        status: "backed_up".into(),
        logical_file_hash: Some(
            "1111111111111111111111111111111111111111111111111111111111111111".into(),
        ),
        created_at: "2026-10-07T10:00:00Z".into(),
        modified_at: None,
        created_timestamp: "2026-10-07T10:00:00Z".into(),
        updated_timestamp: "2026-10-07T10:00:00Z".into(),
    };
    db.create_file(&file_unenc).expect("create file unenc");

    let manifest_unenc = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-unenc".into(),
        logical_file: LogicalFileMetadata {
            file_id: fid_unenc.clone(),
            file_name: "public.txt".into(),
            relative_path: "public.txt".into(),
            original_size: 120,
            created_at: Some(1700000000000),
            modified_at: None,
            mime_type: Some("text/plain".into()),
        },
        encryption: None, // Encryption strictly disabled
        compression: CompressionMetadata::none(),
        integrity: IntegrityMetadata::sha256(
            "1111111111111111111111111111111111111111111111111111111111111111",
        ),
        chunks: vec![ChunkManifest {
            chunk_id: ChunkId::new("chunk-u0").unwrap(),
            index: 0,
            plaintext_size: 120,
            stored_size: 120,
            integrity: IntegrityMetadata::sha256(
                "1111111111111111111111111111111111111111111111111111111111111111",
            ),
            storage_reference: StorageReference::Telegram {
                chat_id: -100123,
                message_id: 101,
                file_id: "tg_doc_101".into(),
            },
        }],
    };
    manifest_unenc.validate().expect("validate unenc");
    db.save_manifest(&manifest_unenc)
        .expect("save unenc manifest");

    let fetched_unenc = db.get_manifest("man-unenc").expect("get").expect("found");
    assert_eq!(fetched_unenc, manifest_unenc);
    assert!(fetched_unenc.encryption.is_none());

    // 2. Encrypted Manifest Roundtrip
    let fid_enc = FileId::new("file-enc").unwrap();
    let file_enc = FileRecord {
        file_id: fid_enc.clone(),
        profile_id: None,
        file_name: "secret.pdf".into(),
        relative_path: "secret.pdf".into(),
        original_size: 5000,
        mime_type: Some("application/pdf".into()),
        status: "backed_up".into(),
        logical_file_hash: Some(
            "2222222222222222222222222222222222222222222222222222222222222222".into(),
        ),
        created_at: "2026-10-07T10:00:00Z".into(),
        modified_at: None,
        created_timestamp: "2026-10-07T10:00:00Z".into(),
        updated_timestamp: "2026-10-07T10:00:00Z".into(),
    };
    db.create_file(&file_enc).expect("create file enc");

    let manifest_enc = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-enc".into(),
        logical_file: LogicalFileMetadata {
            file_id: fid_enc.clone(),
            file_name: "secret.pdf".into(),
            relative_path: "secret.pdf".into(),
            original_size: 5000,
            created_at: Some(1700000000000),
            modified_at: None,
            mime_type: Some("application/pdf".into()),
        },
        encryption: Some(EncryptionMetadata {
            format_version: 1,
            algorithm: televault_core::models::EncryptionAlgorithm::Aes256Gcm,
            kdf: Some(KdfInfo {
                salt_hex: "0102030405060708090a0b0c0d0e0f100102030405060708090a0b0c0d0e0f10".into(),
                memory_kib: 65536,
                iterations: 3,
                parallelism: 2,
            }),
        }),
        compression: CompressionMetadata::zstd(),
        integrity: IntegrityMetadata::sha256(
            "2222222222222222222222222222222222222222222222222222222222222222",
        ),
        chunks: vec![ChunkManifest {
            chunk_id: ChunkId::new("chunk-e0").unwrap(),
            index: 0,
            plaintext_size: 5000,
            stored_size: 5040,
            integrity: IntegrityMetadata::sha256(
                "3333333333333333333333333333333333333333333333333333333333333333",
            ),
            storage_reference: StorageReference::Telegram {
                chat_id: -100123,
                message_id: 102,
                file_id: "tg_doc_102".into(),
            },
        }],
    };
    manifest_enc.validate().expect("validate enc");
    db.save_manifest(&manifest_enc).expect("save enc manifest");

    let fetched_enc = db.get_manifest("man-enc").expect("get").expect("found");
    assert_eq!(fetched_enc, manifest_enc);
    assert!(fetched_enc.encryption.is_some());
    assert_eq!(fetched_enc.compression, CompressionMetadata::zstd());
}

#[test]
fn test_fts5_full_text_search_synchronization() {
    let db = Database::open_in_memory().expect("open db");

    let fid1 = FileId::new("file-secret-01").unwrap();
    let file1 = FileRecord {
        file_id: fid1.clone(),
        profile_id: None,
        file_name: "my-secret-document.pdf".into(),
        relative_path: "confidential/my-secret-document.pdf".into(),
        original_size: 1024,
        mime_type: Some("application/pdf".into()),
        status: "tracked".into(),
        logical_file_hash: None,
        created_at: "2026-10-07T10:00:00Z".into(),
        modified_at: None,
        created_timestamp: "2026-10-07T10:00:00Z".into(),
        updated_timestamp: "2026-10-07T10:00:00Z".into(),
    };

    let fid2 = FileId::new("file-tax-02").unwrap();
    let file2 = FileRecord {
        file_id: fid2.clone(),
        profile_id: None,
        file_name: "tax-return-2025.xlsx".into(),
        relative_path: "finance/tax-return-2025.xlsx".into(),
        original_size: 2048,
        mime_type: Some("application/vnd.ms-excel".into()),
        status: "tracked".into(),
        logical_file_hash: None,
        created_at: "2026-10-07T10:00:00Z".into(),
        modified_at: None,
        created_timestamp: "2026-10-07T10:00:00Z".into(),
        updated_timestamp: "2026-10-07T10:00:00Z".into(),
    };

    // 1. Insert files and test search
    db.create_file(&file1).expect("create file 1");
    db.create_file(&file2).expect("create file 2");

    // Search by word in file_name
    let results = db.search_files("secret").expect("search");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].file_id, fid1);
    assert_eq!(results[0].file_name, "my-secret-document.pdf");

    // Search by word in relative_path
    let results = db.search_files("finance").expect("search");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].file_id, fid2);

    // Search by prefix
    let results = db.search_files("sec").expect("search prefix");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].file_id, fid1);

    // 2. Update file and verify FTS index synchronizes
    let mut file1_updated = file1.clone();
    file1_updated.file_name = "my-archived-report.pdf".into();
    file1_updated.relative_path = "archive/my-archived-report.pdf".into();
    db.update_file(&file1_updated).expect("update file");

    // Old term "secret" should no longer match
    let old_results = db.search_files("secret").expect("search old");
    assert_eq!(old_results.len(), 0);

    // New term "archived" should match
    let new_results = db.search_files("archived").expect("search new");
    assert_eq!(new_results.len(), 1);
    assert_eq!(new_results[0].file_id, fid1);
    assert_eq!(new_results[0].file_name, "my-archived-report.pdf");

    // 3. Delete file and verify FTS index removes entry
    db.delete_file(&fid1).expect("delete file");
    let deleted_search = db.search_files("archived").expect("search deleted");
    assert_eq!(deleted_search.len(), 0);

    // File 2 is still searchable
    let tax_search = db.search_files("tax").expect("search tax");
    assert_eq!(tax_search.len(), 1);
    assert_eq!(tax_search[0].file_id, fid2);
}

#[test]
fn test_snapshots_versions_and_transfer_jobs_crud() {
    let db = Database::open_in_memory().expect("open db");

    let pid = ProfileId::new("prof-backup").unwrap();
    db.create_profile(&ProfileRecord {
        profile_id: pid.clone(),
        name: "Backup Prof".into(),
        description: None,
        source_path: "D:\\Data".into(),
        enabled: true,
        created_at: "2026-10-07T10:00:00Z".into(),
        updated_at: "2026-10-07T10:00:00Z".into(),
    })
    .expect("create profile");

    let fid = FileId::new("file-track-01").unwrap();
    db.create_file(&FileRecord {
        file_id: fid.clone(),
        profile_id: Some(pid.clone()),
        file_name: "data.csv".into(),
        relative_path: "data.csv".into(),
        original_size: 500,
        mime_type: None,
        status: "tracked".into(),
        logical_file_hash: None,
        created_at: "2026-10-07T10:00:00Z".into(),
        modified_at: None,
        created_timestamp: "2026-10-07T10:00:00Z".into(),
        updated_timestamp: "2026-10-07T10:00:00Z".into(),
    })
    .expect("create file");

    let man_id = "man-track-01".to_string();
    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: man_id.clone(),
        logical_file: LogicalFileMetadata {
            file_id: fid.clone(),
            file_name: "data.csv".into(),
            relative_path: "data.csv".into(),
            original_size: 500,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: CompressionMetadata::none(),
        integrity: IntegrityMetadata::sha256(
            "4444444444444444444444444444444444444444444444444444444444444444",
        ),
        chunks: vec![ChunkManifest {
            chunk_id: ChunkId::new("chunk-t0").unwrap(),
            index: 0,
            plaintext_size: 500,
            stored_size: 500,
            integrity: IntegrityMetadata::sha256(
                "4444444444444444444444444444444444444444444444444444444444444444",
            ),
            storage_reference: StorageReference::Pending,
        }],
    };
    db.save_manifest(&manifest).expect("save manifest");

    // 1. Snapshot CRUD
    let snap_id = SnapshotId::new("snap-20261007").unwrap();
    let snapshot = SnapshotRecord {
        snapshot_id: snap_id.clone(),
        profile_id: pid.clone(),
        status: BackupStatus::BackingUp,
        metadata: Some("{\"files_count\": 1}".into()),
        created_at: "2026-10-07T10:00:00Z".into(),
    };
    db.create_snapshot(&snapshot).expect("create snapshot");

    let fetched_snap = db.get_snapshot(&snap_id).expect("get").expect("found");
    assert_eq!(fetched_snap.status, BackupStatus::BackingUp);

    db.update_snapshot_status(&snap_id, BackupStatus::Completed)
        .expect("update status");
    let fetched_snap2 = db.get_snapshot(&snap_id).expect("get").expect("found");
    assert_eq!(fetched_snap2.status, BackupStatus::Completed);

    let profile_snaps = db.list_snapshots_by_profile(&pid).expect("list snaps");
    assert_eq!(profile_snaps.len(), 1);

    // 2. Version CRUD
    let ver_id = VersionId::new("ver-001").unwrap();
    let version = VersionRecord {
        version_id: ver_id.clone(),
        file_id: fid.clone(),
        snapshot_id: snap_id.clone(),
        manifest_id: man_id.clone(),
        status: "active".into(),
        created_at: "2026-10-07T10:00:00Z".into(),
    };
    db.create_version(&version).expect("create version");

    let fetched_ver = db.get_version(&ver_id).expect("get ver").expect("found");
    assert_eq!(fetched_ver.file_id, fid);
    assert_eq!(fetched_ver.snapshot_id, snap_id);

    let snap_versions = db
        .list_versions_by_snapshot(&snap_id)
        .expect("list snap vers");
    assert_eq!(snap_versions.len(), 1);

    let file_versions = db.list_versions_by_file(&fid).expect("list file vers");
    assert_eq!(file_versions.len(), 1);

    // 3. Transfer Job CRUD
    let job_id = JobId::new("job-upload-01").unwrap();
    let job = TransferJobRecord {
        job_id: job_id.clone(),
        file_id: fid.clone(),
        chunk_id: None,
        direction: TransferDirection::Upload,
        status: TransferStatus::Pending,
        progress: 0,
        retry_count: 0,
        error_message: None,
        created_at: "2026-10-07T10:00:00Z".into(),
        updated_at: "2026-10-07T10:00:00Z".into(),
    };
    db.create_transfer_job(&job).expect("create job");

    let fetched_job = db
        .get_transfer_job(&job_id)
        .expect("get job")
        .expect("found");
    assert_eq!(fetched_job.status, TransferStatus::Pending);

    // Update progress
    db.update_transfer_job_progress(&job_id, 250, "2026-10-07T10:01:00Z")
        .expect("update prog");
    assert_eq!(db.get_transfer_job(&job_id).unwrap().unwrap().progress, 250);

    // Increment retry
    db.increment_transfer_job_retry(&job_id, "2026-10-07T10:02:00Z")
        .expect("retry");
    assert_eq!(
        db.get_transfer_job(&job_id).unwrap().unwrap().retry_count,
        1
    );

    // Update status to transferring then failed then completed
    db.update_transfer_job_status(
        &job_id,
        TransferStatus::Transferring,
        None,
        "2026-10-07T10:03:00Z",
    )
    .expect("transferring");
    assert_eq!(
        db.get_transfer_job(&job_id).unwrap().unwrap().status,
        TransferStatus::Transferring
    );

    db.update_transfer_job_status(
        &job_id,
        TransferStatus::Failed,
        Some("Network error"),
        "2026-10-07T10:04:00Z",
    )
    .expect("failed");
    let failed_job = db.get_transfer_job(&job_id).unwrap().unwrap();
    assert_eq!(failed_job.status, TransferStatus::Failed);
    assert_eq!(failed_job.error_message, Some("Network error".into()));

    let failed_jobs = db
        .list_transfer_jobs_by_status(TransferStatus::Failed)
        .expect("list failed");
    assert_eq!(failed_jobs.len(), 1);
}

#[test]
fn test_transaction_rollback_on_failure() {
    let db = Database::open_in_memory().expect("open db");

    let pid = ProfileId::new("prof-tx").unwrap();
    let profile = ProfileRecord {
        profile_id: pid.clone(),
        name: "Tx Profile".into(),
        description: None,
        source_path: "D:\\Tx".into(),
        enabled: true,
        created_at: "2026-10-07T10:00:00Z".into(),
        updated_at: "2026-10-07T10:00:00Z".into(),
    };
    db.create_profile(&profile).expect("create profile");

    // Attempt transaction that inserts a file then errors out
    let fid = FileId::new("file-tx-01").unwrap();
    let tx_res: televault_db::Result<()> = db.with_transaction(|tx| {
        tx.execute(
            "INSERT INTO files (file_id, profile_id, file_name, relative_path, original_size, status, created_at, created_timestamp, updated_timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9);",
            rusqlite::params![
                fid.as_str(),
                pid.as_str(),
                "file.bin",
                "file.bin",
                100,
                "tracked",
                "2026-10-07T10:00:00Z",
                "2026-10-07T10:00:00Z",
                "2026-10-07T10:00:00Z",
            ],
        )?;

        // Deliberately trigger an error
        Err(televault_db::DbError::Constraint("Simulated failure".into()))
    });

    assert!(tx_res.is_err(), "Transaction must fail");

    // Verify file was NOT inserted (rollback succeeded)
    let fetched = db.get_file(&fid).expect("get file");
    assert!(
        fetched.is_none(),
        "Uncommitted transaction changes must be rolled back"
    );
}

#[test]
fn test_file_backed_database_lifecycle_and_reopen() {
    let temp_dir = std::env::temp_dir().join(format!(
        "televault_test_db_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let db_path = temp_dir.join("subfolder").join("televault.db");

    // 1. Open new database (tests parent directory creation & WAL mode)
    {
        let db = Database::open(&db_path).expect("open file db");
        let health = db.health_check().expect("health");
        assert!(health.is_healthy);

        let pid = ProfileId::new("prof-reopen").unwrap();
        db.create_profile(&ProfileRecord {
            profile_id: pid.clone(),
            name: "Reopen Prof".into(),
            description: None,
            source_path: "D:\\Reopen".into(),
            enabled: true,
            created_at: "2026-10-07T10:00:00Z".into(),
            updated_at: "2026-10-07T10:00:00Z".into(),
        })
        .expect("create profile");
    }

    // 2. Re-open existing database (tests idempotency, migrations on existing db)
    {
        let db = Database::open(&db_path).expect("reopen file db");
        let health = db.health_check().expect("health on reopen");
        assert!(health.is_healthy);
        assert_eq!(health.total_profiles, 1);

        let pid = ProfileId::new("prof-reopen").unwrap();
        let fetched = db.get_profile(&pid).expect("get").expect("found");
        assert_eq!(fetched.name, "Reopen Prof");
    }

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
}
