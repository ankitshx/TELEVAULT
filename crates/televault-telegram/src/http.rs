//! Production HTTP transport for Telegram Bot API using reqwest.

use crate::contracts::TelegramDocumentMessage;
use crate::credentials::TelegramCredentials;
use crate::error::{Result, TelegramError};
use crate::transport::{TelegramConnectionInfo, TelegramTransport};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::time::Duration;
use televault_storage::STREAM_CHUNK_BUFFER_SIZE;

/// Default public Telegram Bot API base URL.
pub const DEFAULT_TELEGRAM_API_BASE: &str = "https://api.telegram.org";

/// HTTP-based Telegram transport communicating directly with the Telegram Bot API.
#[derive(Clone)]
pub struct HttpTelegramTransport {
    client: reqwest::blocking::Client,
    credentials: TelegramCredentials,
    base_url: String,
}

impl HttpTelegramTransport {
    /// Creates a new [`HttpTelegramTransport`] with the provided credentials.
    pub fn new(credentials: TelegramCredentials) -> Result<Self> {
        credentials.validate()?;
        let base_url = credentials
            .api_endpoint
            .clone()
            .unwrap_or_else(|| DEFAULT_TELEGRAM_API_BASE.to_string());

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(300))
            .connect_timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| TelegramError::Transport(format!("Failed to build HTTP client: {e}")))?;

        Ok(Self {
            client,
            credentials,
            base_url,
        })
    }

    /// Returns the target chat ID configured for this transport.
    pub fn target_chat_id(&self) -> i64 {
        self.credentials.target_chat_id
    }

    fn bot_url(&self, method: &str) -> String {
        format!(
            "{}/bot{}/{}",
            self.base_url, self.credentials.bot_token, method
        )
    }

    fn file_download_url(&self, file_path: &str) -> String {
        format!(
            "{}/file/bot{}/{}",
            self.base_url, self.credentials.bot_token, file_path
        )
    }

    fn handle_api_error<T>(response: reqwest::blocking::Response) -> Result<T> {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        if let Ok(api_err) = serde_json::from_str::<ApiEnvelope<serde_json::Value>>(&body) {
            let retry_after = api_err.parameters.and_then(|p| p.retry_after);
            if status.as_u16() == 429 || retry_after.is_some() {
                return Err(TelegramError::RateLimited {
                    retry_after_secs: retry_after.unwrap_or(30),
                });
            }
            return Err(TelegramError::Api {
                error_code: api_err.error_code.unwrap_or(status.as_u16() as i32),
                description: api_err.description.unwrap_or_else(|| status.to_string()),
                retry_after_secs: retry_after,
            });
        }
        Err(TelegramError::Transport(format!(
            "HTTP request failed with status {status}: {body}"
        )))
    }
}

#[derive(Deserialize)]
struct ApiEnvelope<T> {
    #[allow(dead_code)]
    ok: bool,
    result: Option<T>,
    description: Option<String>,
    error_code: Option<i32>,
    parameters: Option<ResponseParameters>,
}

#[derive(Deserialize)]
struct ResponseParameters {
    retry_after: Option<u64>,
}

#[derive(Deserialize)]
struct UserDto {
    id: i64,
    #[allow(dead_code)]
    is_bot: bool,
    first_name: String,
    username: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct ChatIdRequest {
    chat_id: i64,
}

#[derive(Deserialize)]
struct ChatDto {
    #[allow(dead_code)]
    id: i64,
    title: Option<String>,
    username: Option<String>,
    first_name: Option<String>,
}

#[derive(Deserialize)]
struct MessageDto {
    message_id: i64,
    #[allow(dead_code)]
    chat: ChatDto,
    document: Option<DocumentDto>,
    caption: Option<String>,
}

#[derive(Deserialize)]
struct DocumentDto {
    file_id: String,
    file_name: Option<String>,
    mime_type: Option<String>,
    file_size: Option<u64>,
}

#[derive(Deserialize)]
struct FileDto {
    #[allow(dead_code)]
    file_id: String,
    file_size: Option<u64>,
    file_path: Option<String>,
}

#[derive(Serialize)]
struct DeleteMessageRequest {
    chat_id: i64,
    message_id: i64,
}

impl TelegramTransport for HttpTelegramTransport {
    fn test_connection(&self, chat_id: i64) -> Result<TelegramConnectionInfo> {
        // 1. Query bot identity (getMe)
        let get_me_url = self.bot_url("getMe");
        let me_resp = self
            .client
            .get(&get_me_url)
            .send()
            .map_err(|e| TelegramError::Transport(format!("Connection test failed: {e}")))?;

        if !me_resp.status().is_success() {
            return Self::handle_api_error(me_resp);
        }

        let me_envelope: ApiEnvelope<UserDto> = me_resp.json().map_err(|e| {
            TelegramError::Transport(format!("Failed to parse getMe response: {e}"))
        })?;

        let user = me_envelope.result.ok_or_else(|| {
            TelegramError::Transport("Invalid response: missing bot user info".into())
        })?;

        // 2. Query target chat accessibility (getChat)
        let get_chat_url = self.bot_url("getChat");
        let chat_resp = self
            .client
            .post(&get_chat_url)
            .json(&ChatIdRequest { chat_id })
            .send()
            .map_err(|e| TelegramError::Transport(format!("Chat verification failed: {e}")))?;

        let chat_title = if chat_resp.status().is_success() {
            let chat_envelope: ApiEnvelope<ChatDto> =
                chat_resp.json().ok().unwrap_or(ApiEnvelope {
                    ok: false,
                    result: None,
                    description: None,
                    error_code: None,
                    parameters: None,
                });
            chat_envelope.result.and_then(|c| {
                c.title
                    .or(c.username.map(|u| format!("@{u}")))
                    .or(c.first_name)
            })
        } else {
            None
        };

        Ok(TelegramConnectionInfo {
            bot_id: user.id,
            bot_username: user.username,
            first_name: user.first_name,
            chat_title,
        })
    }

    fn send_document(
        &self,
        chat_id: i64,
        caption: &str,
        file_name: &str,
        reader: &mut dyn Read,
    ) -> Result<TelegramDocumentMessage> {
        // Bounded streaming spool: spool the reader into a temporary file using 64 KiB buffer
        let temp_dir = std::env::temp_dir();
        let spool_path = temp_dir.join(format!(
            "televault_upload_{}_{}.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));

        let mut spool_file = std::fs::File::create(&spool_path)?;
        let mut buffer = [0u8; STREAM_CHUNK_BUFFER_SIZE];
        let mut total_bytes = 0u64;

        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            spool_file.write_all(&buffer[..n])?;
            total_bytes += n as u64;
        }
        spool_file.flush()?;
        drop(spool_file);

        let upload_url = self.bot_url("sendDocument");
        let file_to_send = match std::fs::File::open(&spool_path) {
            Ok(f) => f,
            Err(e) => {
                let _ = std::fs::remove_file(&spool_path);
                return Err(TelegramError::Io(e));
            }
        };

        let part = reqwest::blocking::multipart::Part::reader(file_to_send)
            .file_name(file_name.to_string())
            .mime_str("application/octet-stream")
            .map_err(|e| {
                let _ = std::fs::remove_file(&spool_path);
                TelegramError::Transport(format!("Failed to build multipart part: {e}"))
            })?;

        let form = reqwest::blocking::multipart::Form::new()
            .text("chat_id", chat_id.to_string())
            .text("caption", caption.to_string())
            .part("document", part);

        let response = self.client.post(&upload_url).multipart(form).send();
        // Staging cleanup guarantee
        let _ = std::fs::remove_file(&spool_path);

        let response = response.map_err(|e| {
            if e.is_timeout() {
                TelegramError::Transport(format!("Upload timed out: {e}"))
            } else {
                TelegramError::Transport(format!("Network error during sendDocument: {e}"))
            }
        })?;

        if !response.status().is_success() {
            return Self::handle_api_error(response);
        }

        let envelope: ApiEnvelope<MessageDto> = response.json().map_err(|e| {
            TelegramError::Transport(format!("Failed to parse sendDocument response: {e}"))
        })?;

        let msg = envelope.result.ok_or_else(|| {
            TelegramError::Transport("sendDocument succeeded but result message is missing".into())
        })?;

        let doc = msg.document.ok_or(TelegramError::DocumentNotFound {
            chat_id,
            message_id: msg.message_id,
        })?;

        Ok(TelegramDocumentMessage {
            chat_id,
            message_id: msg.message_id,
            file_id: doc.file_id,
            size_bytes: doc.file_size.unwrap_or(total_bytes),
            mime_type: doc.mime_type,
            file_name: doc.file_name.or_else(|| Some(file_name.to_string())),
            caption: msg.caption.or_else(|| Some(caption.to_string())),
        })
    }

    fn get_document_stream(
        &self,
        chat_id: i64,
        message_id: i64,
        file_id: &str,
        writer: &mut dyn Write,
    ) -> Result<u64> {
        // 1. Resolve remote file path via getFile
        let get_file_url = format!("{}?file_id={}", self.bot_url("getFile"), file_id);
        let resp = self
            .client
            .get(&get_file_url)
            .send()
            .map_err(|e| TelegramError::Transport(format!("getFile failed: {e}")))?;

        if !resp.status().is_success() {
            if resp.status().as_u16() == 400 || resp.status().as_u16() == 404 {
                return Err(TelegramError::DocumentNotFound {
                    chat_id,
                    message_id,
                });
            }
            return Self::handle_api_error(resp);
        }

        let envelope: ApiEnvelope<FileDto> = resp.json().map_err(|e| {
            TelegramError::Transport(format!("Failed to parse getFile response: {e}"))
        })?;

        let file_info = envelope.result.ok_or(TelegramError::DocumentNotFound {
            chat_id,
            message_id,
        })?;

        let file_path = file_info.file_path.ok_or_else(|| {
            TelegramError::Transport(
                "Telegram returned empty file_path (file may exceed download limits)".into(),
            )
        })?;

        // 2. Stream document payload directly into writer using bounded 64 KiB buffer
        let dl_url = self.file_download_url(&file_path);
        let mut dl_resp = self
            .client
            .get(&dl_url)
            .send()
            .map_err(|e| TelegramError::Transport(format!("Download request failed: {e}")))?;

        if !dl_resp.status().is_success() {
            return Self::handle_api_error(dl_resp);
        }

        let mut buffer = [0u8; STREAM_CHUNK_BUFFER_SIZE];
        let mut total_downloaded = 0u64;

        loop {
            let n = dl_resp.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            writer.write_all(&buffer[..n])?;
            total_downloaded += n as u64;
        }

        writer.flush()?;
        Ok(total_downloaded)
    }

    fn get_message_metadata(
        &self,
        chat_id: i64,
        message_id: i64,
        file_id: &str,
    ) -> Result<TelegramDocumentMessage> {
        let get_file_url = format!("{}?file_id={}", self.bot_url("getFile"), file_id);
        let resp =
            self.client.get(&get_file_url).send().map_err(|e| {
                TelegramError::Transport(format!("getFile metadata check failed: {e}"))
            })?;

        if !resp.status().is_success() {
            if resp.status().as_u16() == 400 || resp.status().as_u16() == 404 {
                return Err(TelegramError::DocumentNotFound {
                    chat_id,
                    message_id,
                });
            }
            return Self::handle_api_error(resp);
        }

        let envelope: ApiEnvelope<FileDto> = resp.json().map_err(|e| {
            TelegramError::Transport(format!("Failed to parse getFile response: {e}"))
        })?;

        let file_info = envelope.result.ok_or(TelegramError::DocumentNotFound {
            chat_id,
            message_id,
        })?;

        Ok(TelegramDocumentMessage {
            chat_id,
            message_id,
            file_id: file_id.to_string(),
            size_bytes: file_info.file_size.unwrap_or(0),
            mime_type: Some("application/octet-stream".into()),
            file_name: None,
            caption: None,
        })
    }

    fn delete_message(&self, chat_id: i64, message_id: i64) -> Result<()> {
        let delete_url = self.bot_url("deleteMessage");
        let resp = self
            .client
            .post(&delete_url)
            .json(&DeleteMessageRequest {
                chat_id,
                message_id,
            })
            .send()
            .map_err(|e| TelegramError::Transport(format!("deleteMessage failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            if body.contains("message to delete not found") {
                return Ok(()); // Already deleted or nonexistent
            }
            return Err(TelegramError::Transport(format!(
                "Failed to delete Telegram message: HTTP {status} {body}"
            )));
        }

        Ok(())
    }
}
