//! Integration tests for the Telegram storage adapter and transport boundary.

use televault_core::ids::{ChunkId, FileId};
use televault_storage::{
    DeleteRequest, DownloadRequest, StorageProvider, StorageStatus, UploadRequest,
    VerificationRequest,
};
use televault_telegram::{
    MockTelegramTransport, TelegramChunkHeader, TelegramReference, TelegramStorageConfig,
    TelegramStorageProvider, TelegramTransport,
};

#[test]
fn test_telegram_storage_provider_lifecycle() {
    let transport = MockTelegramTransport::new();
    let config = TelegramStorageConfig {
        target_chat_id: -1001234567890,
        chunk_upload_timeout_secs: 180,
        max_retries: 3,
    };
    let provider = TelegramStorageProvider::new(transport.clone(), config);

    let payload = b"encrypted chunk intended for Telegram cloud backup storage";
    let mut reader = &payload[..];

    let file_id = FileId::new("file-tg-01").unwrap();
    let chunk_id = ChunkId::new("chunk-tg-01").unwrap();

    let upload_req = UploadRequest {
        file_id: file_id.clone(),
        chunk_id: chunk_id.clone(),
        chunk_index: 0,
        total_chunks: 1,
        expected_size_bytes: payload.len() as u64,
        expected_sha256: None,
        resumable: false,
    };

    // 1. Upload
    let upload_res = provider.upload(&upload_req, &mut reader).expect("upload");
    assert_eq!(upload_res.bytes_uploaded, payload.len() as u64);
    assert!(upload_res.remote_status.is_remotely_complete());
    assert_eq!(upload_res.remote_status, StorageStatus::Verified);

    // Verify storage reference format
    let tg_ref = TelegramReference::from_storage_reference(&upload_res.storage_reference)
        .expect("must be valid TelegramReference");
    assert_eq!(tg_ref.chat_id, -1001234567890);
    assert!(tg_ref.message_id > 0);
    assert!(!tg_ref.file_id.is_empty());

    // 2. Verify caption header tagging on remote message
    let msg_meta = transport
        .get_message_metadata(tg_ref.chat_id, tg_ref.message_id)
        .expect("get meta");
    let caption = msg_meta.caption.expect("caption exists");
    let parsed_header = TelegramChunkHeader::parse_caption(&caption).expect("parse header");
    assert_eq!(parsed_header.file_id, file_id);
    assert_eq!(parsed_header.chunk_id, chunk_id);
    assert_eq!(parsed_header.chunk_index, 0);
    assert_eq!(parsed_header.total_chunks, 1);
    assert_eq!(parsed_header.size_bytes, payload.len() as u64);

    // 3. Verification request
    let v_req = VerificationRequest {
        storage_reference: upload_res.storage_reference.clone(),
        expected_size_bytes: payload.len() as u64,
        expected_sha256: upload_res.verified_sha256.clone(),
    };
    assert!(provider.verify(&v_req).expect("verify"));

    // 4. Download
    let dl_req = DownloadRequest {
        file_id,
        chunk_id,
        storage_reference: upload_res.storage_reference.clone(),
        expected_size_bytes: payload.len() as u64,
        expected_sha256: upload_res.verified_sha256,
    };
    let mut downloaded = Vec::new();
    let dl_res = provider
        .download(&dl_req, &mut downloaded)
        .expect("download");
    assert_eq!(dl_res.bytes_downloaded, payload.len() as u64);
    assert_eq!(downloaded, payload);

    // 5. Delete
    provider
        .delete(&DeleteRequest {
            storage_reference: upload_res.storage_reference.clone(),
        })
        .expect("delete");

    assert!(!provider.verify(&v_req).expect("verify after delete"));
}

#[test]
fn test_telegram_multi_chunk_headers() {
    let header0 = TelegramChunkHeader {
        format_version: 1,
        file_id: FileId::new("file-video-5gb").unwrap(),
        chunk_id: ChunkId::new("chunk-0").unwrap(),
        chunk_index: 0,
        total_chunks: 3,
        size_bytes: 1_887_436_800,
    };
    let header1 = TelegramChunkHeader {
        format_version: 1,
        file_id: FileId::new("file-video-5gb").unwrap(),
        chunk_id: ChunkId::new("chunk-1").unwrap(),
        chunk_index: 1,
        total_chunks: 3,
        size_bytes: 1_887_436_800,
    };
    let header2 = TelegramChunkHeader {
        format_version: 1,
        file_id: FileId::new("file-video-5gb").unwrap(),
        chunk_id: ChunkId::new("chunk-2").unwrap(),
        chunk_index: 2,
        total_chunks: 3,
        size_bytes: 1_808_583_885,
    };

    let cap0 = header0.to_caption();
    let cap1 = header1.to_caption();
    let cap2 = header2.to_caption();

    assert_eq!(TelegramChunkHeader::parse_caption(&cap0).unwrap(), header0);
    assert_eq!(TelegramChunkHeader::parse_caption(&cap1).unwrap(), header1);
    assert_eq!(TelegramChunkHeader::parse_caption(&cap2).unwrap(), header2);
}
