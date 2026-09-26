use std::sync::Arc;
use telecloud_core::TransferState;
use telecloud_db::DatabaseRepository;
use telecloud_storage::MockStorageProvider;
use telecloud_transfer::{TransferManager, UploadParams};
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

#[tokio::test]
async fn test_transfer_lifecycle_queue_pause_resume() {
    let temp_dir = std::env::temp_dir().join(format!("telecloud_lifecycle_{}", Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).await.unwrap();
    let file_path = temp_dir.join("test_file.bin");

    {
        let mut f = File::create(&file_path).await.unwrap();
        f.write_all(b"Lifecycle test payload").await.unwrap();
        f.flush().await.unwrap();
    }

    let storage = Arc::new(MockStorageProvider::new(-1001234567890));
    let db = Arc::new(DatabaseRepository::open_in_memory().unwrap());
    let manager = TransferManager::new(storage, db);

    let transfer_id = Uuid::new_v4();
    let upload_params = UploadParams {
        transfer_id,
        file_id: None,
        folder_id: None,
        local_path: file_path.clone(),
        file_name: "test_file.bin".to_string(),
        passphrase: None,
        chunk_size: Some(1024 * 1024),
    };

    // 1. Enqueue upload
    manager.enqueue_upload(upload_params).await.unwrap();

    let record = manager.get_transfer(transfer_id).await.unwrap().unwrap();
    assert!(record.state == TransferState::Queued || record.state == TransferState::Uploading);

    // 2. Pause transfer
    manager.pause_transfer(transfer_id).await.unwrap();
    let record = manager.get_transfer(transfer_id).await.unwrap().unwrap();
    assert_eq!(record.state, TransferState::Paused);

    // 3. Resume transfer
    manager.resume_transfer(transfer_id).await.unwrap();
    let record = manager.get_transfer(transfer_id).await.unwrap().unwrap();
    assert!(record.state == TransferState::Uploading || record.state == TransferState::Completed);

    // 4. Cancel transfer
    manager.cancel_transfer(transfer_id).await.unwrap();
    let record = manager.get_transfer(transfer_id).await.unwrap().unwrap();
    assert_eq!(record.state, TransferState::Cancelled);

    let _ = fs::remove_dir_all(&temp_dir).await;
}
