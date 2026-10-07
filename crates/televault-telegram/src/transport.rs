//! Low-level Telegram client transport trait and test mock implementation.

use crate::contracts::TelegramDocumentMessage;
use crate::error::{Result, TelegramError};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use televault_storage::STREAM_CHUNK_BUFFER_SIZE;

/// Low-level transport abstraction for Telegram MTProto / Bot API communication.
///
/// Decouples the storage provider logic from the concrete MTProto engine.
pub trait TelegramTransport: Send + Sync {
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
        writer: &mut dyn Write,
    ) -> Result<u64>;

    /// Retrieves message metadata without downloading the document payload.
    fn get_message_metadata(
        &self,
        chat_id: i64,
        message_id: i64,
    ) -> Result<TelegramDocumentMessage>;

    /// Deletes a message from the designated chat.
    fn delete_message(&self, chat_id: i64, message_id: i64) -> Result<()>;
}

type MessageStore = Arc<Mutex<HashMap<(i64, i64), (TelegramDocumentMessage, Vec<u8>)>>>;

/// In-memory mock transport simulating Telegram Cloud document storage.
#[derive(Debug, Clone)]
pub struct MockTelegramTransport {
    messages: MessageStore,
    next_message_id: Arc<AtomicI64>,
}

impl MockTelegramTransport {
    /// Creates a new [`MockTelegramTransport`].
    pub fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(HashMap::new())),
            next_message_id: Arc::new(AtomicI64::new(500)),
        }
    }
}

impl Default for MockTelegramTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl TelegramTransport for MockTelegramTransport {
    fn send_document(
        &self,
        chat_id: i64,
        caption: &str,
        file_name: &str,
        reader: &mut dyn Read,
    ) -> Result<TelegramDocumentMessage> {
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
        writer: &mut dyn Write,
    ) -> Result<u64> {
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
    ) -> Result<TelegramDocumentMessage> {
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
        let mut guard = self.messages.lock().unwrap();
        guard.remove(&(chat_id, message_id));
        Ok(())
    }
}
