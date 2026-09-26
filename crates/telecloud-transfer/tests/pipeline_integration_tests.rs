use std::sync::Arc;
use telecloud_db::DatabaseRepository;
use telecloud_storage::MockStorageProvider;
use telecloud_transfer::{DownloadParams, TransferManager, UploadParams};
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

#[tokio::test]
async fn test_streaming_upload_and_download_roundtrip() {
    let temp_dir = std::env::temp_dir().join(format!("telecloud_test_{}", Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).await.unwrap();

    let src_file_path = temp_dir.join("sample_dataset.bin");
    let dest_file_path = temp_dir.join("sample_dataset_downloaded.bin");

    // Create a 3.5 MB file to ensure multi-chunking with 1 MB chunk size
    let file_size: usize = 3 * 1024 * 1024 + 512 * 1024;
    let mut pattern_data = Vec::with_capacity(file_size);
    for i in 0..file_size {
        pattern_data.push((i % 251) as u8);
    }

    {
        let mut f = File::create(&src_file_path).await.unwrap();
        f.write_all(&pattern_data).await.unwrap();
        f.flush().await.unwrap();
    }

    let storage = Arc::new(MockStorageProvider::new(-1001234567890));
    let db = Arc::new(DatabaseRepository::open_in_memory().unwrap());
    let manager = TransferManager::new(storage.clone(), db.clone());

    // 1. Upload
    let transfer_id = Uuid::new_v4();
    let upload_params = UploadParams {
        transfer_id,
        file_id: None,
        folder_id: None,
        local_path: src_file_path.clone(),
        file_name: "sample_dataset.bin".to_string(),
        passphrase: None,
        chunk_size: Some(1024 * 1024), // 1 MB chunks -> 4 chunks total
    };

    let uploaded_file = manager.execute_upload_now(upload_params).await.unwrap();
    assert_eq!(uploaded_file.size, file_size as u64);

    // Verify storage chunks
    assert_eq!(storage.stored_count().await, 4);

    // Verify DB chunks
    let db_chunks = db.get_file_chunks(uploaded_file.id).unwrap();
    assert_eq!(db_chunks.len(), 4);

    // Verify Manifest
    let manifest_str = db.get_manifest(uploaded_file.id).unwrap();
    assert!(manifest_str.is_some());

    // 2. Download
    let download_transfer_id = Uuid::new_v4();
    let download_params = DownloadParams {
        transfer_id: download_transfer_id,
        file_id: uploaded_file.id,
        dest_path: dest_file_path.clone(),
        passphrase: None,
    };

    manager.execute_download_now(download_params).await.unwrap();

    // Verify downloaded file exists and content matches exactly
    let downloaded_data = fs::read(&dest_file_path).await.unwrap();
    assert_eq!(downloaded_data.len(), file_size);
    assert_eq!(downloaded_data, pattern_data);

    // Verify integrity record in DB
    let integrity = db.get_integrity_record(uploaded_file.id).unwrap().unwrap();
    assert!(integrity.is_valid);

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir).await;
}

#[tokio::test]
async fn test_encrypted_upload_and_download_with_passphrase() {
    let temp_dir = std::env::temp_dir().join(format!("telecloud_test_enc_{}", Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).await.unwrap();

    let src_file_path = temp_dir.join("secret_document.pdf");
    let dest_file_path = temp_dir.join("secret_document_restored.pdf");

    let file_size: usize = 2 * 1024 * 1024 + 100;
    let mut payload = vec![0x42u8; file_size];
    // Add unique watermark
    payload[0..14].copy_from_slice(b"TOP_SECRET_DOC");

    {
        let mut f = File::create(&src_file_path).await.unwrap();
        f.write_all(&payload).await.unwrap();
        f.flush().await.unwrap();
    }

    let storage = Arc::new(MockStorageProvider::new(-1009876543210));
    let db = Arc::new(DatabaseRepository::open_in_memory().unwrap());
    let manager = TransferManager::new(storage.clone(), db.clone());

    let passphrase = "my_ultra_secure_passphrase_2026";

    // 1. Upload Encrypted
    let upload_id = Uuid::new_v4();
    let upload_params = UploadParams {
        transfer_id: upload_id,
        file_id: None,
        folder_id: None,
        local_path: src_file_path.clone(),
        file_name: "secret_document.pdf".to_string(),
        passphrase: Some(passphrase.to_string()),
        chunk_size: Some(1024 * 1024), // 1 MB chunks -> 3 chunks
    };

    let uploaded_file = manager.execute_upload_now(upload_params).await.unwrap();
    assert!(uploaded_file.is_encrypted);

    // 2. Download with WRONG passphrase should fail
    let wrong_download_id = Uuid::new_v4();
    let wrong_download_params = DownloadParams {
        transfer_id: wrong_download_id,
        file_id: uploaded_file.id,
        dest_path: dest_file_path.clone(),
        passphrase: Some("wrong_password".to_string()),
    };

    let wrong_res = manager.execute_download_now(wrong_download_params).await;
    assert!(wrong_res.is_err());

    // 3. Download with CORRECT passphrase should succeed
    let correct_download_id = Uuid::new_v4();
    let correct_download_params = DownloadParams {
        transfer_id: correct_download_id,
        file_id: uploaded_file.id,
        dest_path: dest_file_path.clone(),
        passphrase: Some(passphrase.to_string()),
    };

    manager.execute_download_now(correct_download_params).await.unwrap();

    let decrypted_data = fs::read(&dest_file_path).await.unwrap();
    assert_eq!(decrypted_data.len(), file_size);
    assert_eq!(&decrypted_data[0..14], b"TOP_SECRET_DOC");
    assert_eq!(decrypted_data, payload);

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir).await;
}
