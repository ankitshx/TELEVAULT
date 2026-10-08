//! Low-level Telegram client transport trait and test mock implementation.

use crate::contracts::TelegramDocumentMessage;
use crate::error::{Result, TelegramError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use televault_storage::STREAM_CHUNK_BUFFER_SIZE;

/// Non-sensitive diagnostic information confirming Telegram connectivity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelegramConnectionInfo {
    /// Telegram bot numeric user identifier.
    pub bot_id: i64,
    /// Telegram bot username (e.g. "TeleVaultBackupBot").
    pub bot_username: Option<String>,
    /// Telegram bot display name.
    pub first_name: String,
    /// Target chat or channel title, if accessible.
    pub chat_title: Option<String>,
}

/// Low-level transport abstraction for Telegram MTProto / Bot API communication.
///
/// Decouples the storage provider logic from the concrete MTProto or HTTP Bot API engine.
pub trait TelegramTransport: Send + Sync {
    /// Tests connection and access to the target chat without creating backup payloads.
    fn test_connection(&self, chat_id: i64) -> Result<TelegramConnectionInfo>;

    /// Uploads a document to the specified chat via streaming I/O.
    fn send_document(
        &self,
        chat_id: i64,
        caption: &str,
        file_name: &str,
        reader: &mut dyn Read,
    ) -> Result<TelegramDocumentMessage>;

    /// Downloads a document payload from a message directly to the target writer.
    fn get_document_stream(
        &self,
        chat_id: i64,
        message_id: i64,
        file_id: &str,
        writer: &mut dyn Write,
    ) -> Result<u64>;

    /// Retrieves message metadata without downloading the document payload.
    fn get_message_metadata(
        &self,
        chat_id: i64,
        message_id: i64,
        file_id: &str,
    ) -> Result<TelegramDocumentMessage>;

    /// Deletes a message from the designated chat.
    fn delete_message(&self, chat_id: i64, message_id: i64) -> Result<()>;
}

type MessageStore = Arc<Mutex<HashMap<(i64, i64), (TelegramDocumentMessage, Vec<u8>)>>>;

/// In-memory mock transport simulating Telegram Cloud document storage and network fault conditions.
#[derive(Debug, Clone)]
pub struct MockTelegramTransport {
    messages: MessageStore,
    next_message_id: Arc<AtomicI64>,
    simulated_upload_failures: Arc<AtomicI64>,
    simulated_download_failures: Arc<AtomicI64>,
    simulate_unreachable: Arc<AtomicBool>,
    simulate_rate_limit: Arc<AtomicU64>,
}

impl MockTelegramTransport {
    /// Creates a new [`MockTelegramTransport`].
    pub fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(HashMap::new())),
            next_message_id: Arc::new(AtomicI64::new(500)),
            simulated_upload_failures: Arc::new(AtomicI64::new(0)),
            simulated_download_failures: Arc::new(AtomicI64::new(0)),
            simulate_unreachable: Arc::new(AtomicBool::new(false)),
            simulate_rate_limit: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Sets the number of consecutive upload failures to simulate before succeeding.
    pub fn set_simulated_upload_failures(&self, count: i64) {
        self.simulated_upload_failures
            .store(count, Ordering::SeqCst);
    }

    /// Sets the number of consecutive download failures to simulate before succeeding.
    pub fn set_simulated_download_failures(&self, count: i64) {
        self.simulated_download_failures
            .store(count, Ordering::SeqCst);
    }

    /// Toggles whether Telegram is simulated as unreachable (network timeout).
    pub fn set_simulate_unreachable(&self, unreachable: bool) {
        self.simulate_unreachable
            .store(unreachable, Ordering::SeqCst);
    }

    /// Sets simulated rate limiting (FloodWait).
    pub fn set_simulate_rate_limit(&self, retry_after_secs: u64) {
        self.simulate_rate_limit
            .store(retry_after_secs, Ordering::SeqCst);
    }

    /// Simulates remote bitrot / corruption by modifying document payload bytes.
    pub fn corrupt_message(&self, chat_id: i64, message_id: i64) -> Result<()> {
        let mut guard = self.messages.lock().unwrap();
        if let Some((_, bytes)) = guard.get_mut(&(chat_id, message_id)) {
            if !bytes.is_empty() {
                bytes[0] ^= 0xFF; // Invert first byte to corrupt content
                Ok(())
            } else {
                Err(TelegramError::Transport(
                    "Empty payload cannot be corrupted".into(),
                ))
            }
        } else {
            Err(TelegramError::MessageNotFound {
                chat_id,
                message_id,
            })
        }
    }
}

impl Default for MockTelegramTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl TelegramTransport for MockTelegramTransport {
    fn test_connection(&self, chat_id: i64) -> Result<TelegramConnectionInfo> {
        if self.simulate_unreachable.load(Ordering::SeqCst) {
            return Err(TelegramError::Transport(
                "Simulated network timeout: Telegram service unreachable".into(),
            ));
        }
        if chat_id == 0 {
            return Err(TelegramError::InvalidReference(
                "Target chat ID cannot be 0".into(),
            ));
        }
        Ok(TelegramConnectionInfo {
            bot_id: 123456789,
            bot_username: Some("TeleVaultMockBot".into()),
            first_name: "TELEVAULT Mock Bot".into(),
            chat_title: Some(format!("Vault Channel ({chat_id})")),
        })
    }

    fn send_document(
        &self,
        chat_id: i64,
        caption: &str,
        file_name: &str,
        reader: &mut dyn Read,
    ) -> Result<TelegramDocumentMessage> {
        if self.simulate_unreachable.load(Ordering::SeqCst) {
            return Err(TelegramError::Transport(
                "Simulated network timeout: Telegram service unreachable".into(),
            ));
        }

        let rate_limit = self.simulate_rate_limit.swap(0, Ordering::SeqCst);
        if rate_limit > 0 {
            return Err(TelegramError::RateLimited {
                retry_after_secs: rate_limit,
            });
        }

        let remaining_fails = self.simulated_upload_failures.load(Ordering::SeqCst);
        if remaining_fails > 0 {
            self.simulated_upload_failures
                .fetch_sub(1, Ordering::SeqCst);
            return Err(TelegramError::Transport(
                "Simulated upload network interruption (connection reset)".into(),
            ));
        }

        let mut buffer = [0u8; STREAM_CHUNK_BUFFER_SIZE];
        let mut bytes = Vec::new();
        let mut total_read = 0;

        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..n]);
            total_read += n as u64;
        }

        let msg_id = self.next_message_id.fetch_add(1, Ordering::SeqCst);
        let file_id = format!("BAADBAAD_file_{msg_id}");

        let doc = TelegramDocumentMessage {
            chat_id,
            message_id: msg_id,
            file_id,
            size_bytes: total_read,
            mime_type: Some("application/octet-stream".into()),
            file_name: Some(file_name.to_string()),
            caption: Some(caption.to_string()),
        };

        self.messages
            .lock()
            .unwrap()
            .insert((chat_id, msg_id), (doc.clone(), bytes));

        Ok(doc)
    }

    fn get_document_stream(
        &self,
        chat_id: i64,
        message_id: i64,
        _file_id: &str,
        writer: &mut dyn Write,
    ) -> Result<u64> {
        if self.simulate_unreachable.load(Ordering::SeqCst) {
            return Err(TelegramError::Transport(
                "Simulated network timeout: Telegram service unreachable".into(),
            ));
        }

        let remaining_fails = self.simulated_download_failures.load(Ordering::SeqCst);
        if remaining_fails > 0 {
            self.simulated_download_failures
                .fetch_sub(1, Ordering::SeqCst);
            return Err(TelegramError::Transport(
                "Simulated download network interruption (peer closed stream)".into(),
            ));
        }

        let guard = self.messages.lock().unwrap();
        let (_, bytes) =
            guard
                .get(&(chat_id, message_id))
                .ok_or(TelegramError::MessageNotFound {
                    chat_id,
                    message_id,
                })?;

        for chunk in bytes.chunks(STREAM_CHUNK_BUFFER_SIZE) {
            writer.write_all(chunk)?;
        }

        Ok(bytes.len() as u64)
    }

    fn get_message_metadata(
        &self,
        chat_id: i64,
        message_id: i64,
        _file_id: &str,
    ) -> Result<TelegramDocumentMessage> {
        if self.simulate_unreachable.load(Ordering::SeqCst) {
            return Err(TelegramError::Transport(
                "Simulated network timeout: Telegram service unreachable".into(),
            ));
        }

        let guard = self.messages.lock().unwrap();
        let (doc, _) = guard
            .get(&(chat_id, message_id))
            .ok_or(TelegramError::MessageNotFound {
                chat_id,
                message_id,
            })?;

        Ok(doc.clone())
    }

    fn delete_message(&self, chat_id: i64, message_id: i64) -> Result<()> {
        if self.simulate_unreachable.load(Ordering::SeqCst) {
            return Err(TelegramError::Transport(
                "Simulated network timeout: Telegram service unreachable".into(),
            ));
        }

        let mut guard = self.messages.lock().unwrap();
        guard.remove(&(chat_id, message_id));
        Ok(())
    }
}
