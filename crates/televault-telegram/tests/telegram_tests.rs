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
        .get_message_metadata(tg_ref.chat_id, tg_ref.message_id, &tg_ref.file_id)
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

#[test]
fn test_telegram_connection_test_valid_invalid_unreachable() {
    let transport = MockTelegramTransport::new();
    let config = TelegramStorageConfig {
        target_chat_id: -1001234567890,
        chunk_upload_timeout_secs: 180,
        max_retries: 3,
    };
    let provider = TelegramStorageProvider::new(transport.clone(), config);

    // 1. Valid configuration connection test
    let info = provider.test_connection().expect("valid test connection");
    assert_eq!(info.bot_id, 123456789);
    assert_eq!(info.bot_username.as_deref(), Some("TeleVaultMockBot"));
    assert!(info.chat_title.unwrap().contains("-1001234567890"));

    // 2. Invalid target chat ID
    let invalid_config = TelegramStorageConfig {
        target_chat_id: 0,
        chunk_upload_timeout_secs: 180,
        max_retries: 3,
    };
    let invalid_provider = TelegramStorageProvider::new(transport.clone(), invalid_config);
    assert!(invalid_provider.test_connection().is_err());

    // 3. Unreachable service
    transport.set_simulate_unreachable(true);
    let err = provider.test_connection().unwrap_err();
    assert!(err.to_string().contains("unreachable") || err.to_string().contains("timeout"));
    transport.set_simulate_unreachable(false);
}

#[test]
fn test_telegram_simulated_network_failures_and_retryability() {
    let transport = MockTelegramTransport::new();
    let config = TelegramStorageConfig {
        target_chat_id: -100555666777,
        chunk_upload_timeout_secs: 60,
        max_retries: 3,
    };
    let provider = TelegramStorageProvider::new(transport.clone(), config);

    let payload = b"critical backup bytes subject to network drop";
    let file_id = FileId::new("file-net-fail").unwrap();
    let chunk_id = ChunkId::new("chunk-net-fail").unwrap();

    let upload_req = UploadRequest {
        file_id: file_id.clone(),
        chunk_id: chunk_id.clone(),
        chunk_index: 0,
        total_chunks: 1,
        expected_size_bytes: payload.len() as u64,
        expected_sha256: None,
        resumable: false,
    };

    // 1. Simulate 2 transient upload failures, then succeed on 3rd attempt
    transport.set_simulated_upload_failures(2);

    let err1 = provider.upload(&upload_req, &mut &payload[..]).unwrap_err();
    assert!(matches!(
        err1,
        televault_storage::StorageError::ProviderUnavailable(_)
    ));

    let err2 = provider.upload(&upload_req, &mut &payload[..]).unwrap_err();
    assert!(matches!(
        err2,
        televault_storage::StorageError::ProviderUnavailable(_)
    ));

    let ok_res = provider
        .upload(&upload_req, &mut &payload[..])
        .expect("3rd attempt succeeds");
    assert_eq!(ok_res.bytes_uploaded, payload.len() as u64);

    // 2. Simulate rate limit (FloodWait)
    transport.set_simulate_rate_limit(15);
    let rl_err = provider.upload(&upload_req, &mut &payload[..]).unwrap_err();
    match rl_err {
        televault_storage::StorageError::UploadFailed { retryable, reason } => {
            assert!(retryable);
            assert!(reason.contains("rate limited"));
        }
        other => panic!("Expected rate limited error, got: {other:?}"),
    }

    // 3. Simulate download network interruption
    let dl_req = DownloadRequest {
        file_id,
        chunk_id,
        storage_reference: ok_res.storage_reference,
        expected_size_bytes: payload.len() as u64,
        expected_sha256: ok_res.verified_sha256,
    };

    transport.set_simulated_download_failures(1);
    let mut dl_buf = Vec::new();
    let dl_err = provider.download(&dl_req, &mut dl_buf).unwrap_err();
    assert!(matches!(
        dl_err,
        televault_storage::StorageError::ProviderUnavailable(_)
    ));

    // Next download succeeds
    dl_buf.clear();
    let dl_ok = provider
        .download(&dl_req, &mut dl_buf)
        .expect("retry download succeeds");
    assert_eq!(dl_ok.bytes_downloaded, payload.len() as u64);
    assert_eq!(dl_buf, payload);
}

#[test]
fn test_telegram_remote_bitrot_corruption_detection() {
    let transport = MockTelegramTransport::new();
    let config = TelegramStorageConfig {
        target_chat_id: -100999111222,
        chunk_upload_timeout_secs: 120,
        max_retries: 3,
    };
    let provider = TelegramStorageProvider::new(transport.clone(), config);

    let payload = b"original pristine backup payload data";
    let file_id = FileId::new("file-corrupt-01").unwrap();
    let chunk_id = ChunkId::new("chunk-corrupt-01").unwrap();

    let upload_req = UploadRequest {
        file_id: file_id.clone(),
        chunk_id: chunk_id.clone(),
        chunk_index: 0,
        total_chunks: 1,
        expected_size_bytes: payload.len() as u64,
        expected_sha256: None,
        resumable: false,
    };

    let upload_res = provider
        .upload(&upload_req, &mut &payload[..])
        .expect("upload");
    let tg_ref = TelegramReference::from_storage_reference(&upload_res.storage_reference).unwrap();

    // Verify before corruption
    let v_req = VerificationRequest {
        storage_reference: upload_res.storage_reference.clone(),
        expected_size_bytes: payload.len() as u64,
        expected_sha256: upload_res.verified_sha256.clone(),
    };
    assert!(provider.verify(&v_req).unwrap());

    // Inject corruption directly into remote storage message
    transport
        .corrupt_message(tg_ref.chat_id, tg_ref.message_id)
        .expect("corrupt message");

    // Download with expected hash detects corruption
    let dl_req = DownloadRequest {
        file_id,
        chunk_id,
        storage_reference: upload_res.storage_reference,
        expected_size_bytes: payload.len() as u64,
        expected_sha256: upload_res.verified_sha256,
    };
    let mut corrupted_out = Vec::new();
    let dl_err = provider.download(&dl_req, &mut corrupted_out).unwrap_err();
    assert!(matches!(
        dl_err,
        televault_storage::StorageError::VerificationFailed { .. }
    ));
}

#[test]
fn test_telegram_progressive_streaming_sizes() {
    let transport = MockTelegramTransport::new();
    let config = TelegramStorageConfig {
        target_chat_id: -100888999,
        chunk_upload_timeout_secs: 180,
        max_retries: 3,
    };
    let provider = TelegramStorageProvider::new(transport, config);

    // Test progressive sizes: 1 KB (small), 256 KB (medium), 1 MB (larger stream spanning 16 buffer chunks)
    let sizes = [1024usize, 256 * 1024, 1024 * 1024];

    for (idx, &size) in sizes.iter().enumerate() {
        let pattern_byte = ((idx * 37 + 13) % 256) as u8;
        let synthetic_data = vec![pattern_byte; size];

        let file_id = FileId::new(format!("file-prog-{idx}")).unwrap();
        let chunk_id = ChunkId::new(format!("chunk-prog-{idx}")).unwrap();

        let req = UploadRequest {
            file_id: file_id.clone(),
            chunk_id: chunk_id.clone(),
            chunk_index: 0,
            total_chunks: 1,
            expected_size_bytes: size as u64,
            expected_sha256: None,
            resumable: false,
        };

        let upload_res = provider
            .upload(&req, &mut &synthetic_data[..])
            .expect("upload");
        assert_eq!(upload_res.bytes_uploaded, size as u64);

        let dl_req = DownloadRequest {
            file_id,
            chunk_id,
            storage_reference: upload_res.storage_reference,
            expected_size_bytes: size as u64,
            expected_sha256: upload_res.verified_sha256,
        };

        let mut downloaded = Vec::new();
        let dl_res = provider
            .download(&dl_req, &mut downloaded)
            .expect("download");
        assert_eq!(dl_res.bytes_downloaded, size as u64);
        assert_eq!(downloaded, synthetic_data);
    }
}
