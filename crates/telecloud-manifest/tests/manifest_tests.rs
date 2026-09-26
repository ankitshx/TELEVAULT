use telecloud_manifest::FileManifest;
use uuid::Uuid;

#[test]
fn test_manifest_creation_and_validation() {
    let file_id = Uuid::new_v4();
    let name = "database_backup.sql".to_string();
    let size = 350 * 1024 * 1024; // 350 MB
    let chunk_size = 100 * 1024 * 1024; // 100 MB chunks -> 4 chunks (100, 100, 100, 50)
    let full_sha = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string();

    let manifest = FileManifest::new(file_id, name.clone(), size, full_sha.clone(), chunk_size, None);

    assert_eq!(manifest.total_chunks, 4);
    assert_eq!(manifest.chunks.len(), 4);
    assert_eq!(manifest.chunks[0].size, 100 * 1024 * 1024);
    assert_eq!(manifest.chunks[3].size, 50 * 1024 * 1024);
    assert!(manifest.validate().is_ok());

    // JSON round-trip
    let json = manifest.to_json().unwrap();
    let parsed = FileManifest::from_json(&json).unwrap();
    assert_eq!(parsed.file_id, file_id);
    assert_eq!(parsed.total_chunks, 4);
    assert_eq!(parsed.name, name);
}

#[test]
fn test_tampered_manifest_rejected() {
    let file_id = Uuid::new_v4();
    let size = 200 * 1024 * 1024;
    let chunk_size = 100 * 1024 * 1024;
    let full_sha = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string();

    let mut manifest = FileManifest::new(file_id, "test.bin".to_string(), size, full_sha, chunk_size, None);
    // Tamper with chunk size so sum does not match total size
    manifest.chunks[0].size = 50;

    assert!(manifest.validate().is_err());
}
