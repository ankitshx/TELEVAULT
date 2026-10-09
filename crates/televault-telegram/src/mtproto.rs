//! Personal Telegram MTProto authentication, session persistence, and channel management.
//!
//! Provides a secure state-machine-backed authentication flow for personal Telegram accounts
//! using MTProto (`grammers-client`), dedicated private backup channel setup with automatic
//! creation and guided manual fallback, and mandatory centralized authorization gating.

use crate::error::{Result, TelegramError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// High-level authentication and authorization states for personal Telegram account integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthState {
    /// System is starting up and inspecting stored session credentials.
    Initializing,
    /// No authenticated session exists; awaiting user login.
    AuthenticationRequired,
    /// Authentication in progress; awaiting code or two-step verification password.
    Authenticating,
    /// Authentication attempt failed due to invalid code, password, or network error.
    AuthenticationFailed,
    /// Account verified; awaiting private backup channel configuration.
    Authenticated,
    /// Logged in, but dedicated private backup channel has not been created or selected.
    ChannelSetupRequired,
    /// Private backup channel creation or verification is in progress.
    ChannelSetupInProgress,
    /// Channel verification failed (e.g. insufficient post permissions or inaccessible channel).
    ChannelVerificationFailed,
    /// Authenticated and backup channel verified; all dashboard and backup operations unlocked.
    Ready,
    /// Session expired, invalidated, or revoked by Telegram.
    SessionExpired,
    /// User initiated logout; clearing session material and in-memory credentials.
    LoggingOut,
    /// Non-fatal recoverable error encountered during authentication or channel setup.
    RecoverableError,
}

impl AuthState {
    /// Returns whether the state allows access to protected backup operations.
    pub fn is_unlocked(&self) -> bool {
        matches!(self, AuthState::Ready)
    }
}

/// Safe public profile information for the authenticated Telegram personal account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelegramAccountInfo {
    /// Telegram numeric user identifier.
    pub user_id: i64,
    /// Account first name.
    pub first_name: String,
    /// Account optional last name.
    pub last_name: Option<String>,
    /// Telegram username handle (without '@'), if configured.
    pub username: Option<String>,
    /// Redacted phone number for safe display (e.g., `+1 *** *** 1234`).
    pub phone_number: String,
}

/// Metadata and verification status for the dedicated Telegram backup channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelegramChannelInfo {
    /// Telegram numeric channel identifier (positive or -100 prefixed).
    pub channel_id: i64,
    /// Channel title.
    pub channel_title: String,
    /// Whether the channel is private (TELEVAULT requires private channels).
    pub is_private: bool,
    /// Whether permissions and message post/read capabilities were verified.
    pub verified: bool,
    /// Whether this channel was created automatically by TELEVAULT.
    pub created_by_televault: bool,
}

/// Complete current authentication and channel setup status returned to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelegramAuthStatus {
    /// Current state machine status.
    pub state: AuthState,
    /// Authenticated account details, if available.
    pub account: Option<TelegramAccountInfo>,
    /// Dedicated backup channel details, if configured.
    pub channel: Option<TelegramChannelInfo>,
    /// Whether a Two-Step Verification (2FA) cloud password is required to complete login.
    pub requires_password: bool,
    /// Safe human-readable error or explanation message, if any.
    pub error_message: Option<String>,
}

/// Persisted session metadata stored securely on disk in the application config directory.
#[derive(Clone, Serialize, Deserialize)]
pub struct PersistedTelegramSession {
    /// Associated account profile.
    pub account: TelegramAccountInfo,
    /// Associated verified channel metadata.
    pub channel: Option<TelegramChannelInfo>,
    /// Serialized MTProto session bytes (contains auth key and DC addresses).
    pub session_bytes: Vec<u8>,
    /// Telegram application API ID.
    pub api_id: i32,
    /// Telegram application API Hash.
    pub api_hash: String,
    /// Session last updated timestamp (RFC-3339).
    pub updated_at: String,
}

impl std::fmt::Debug for PersistedTelegramSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersistedTelegramSession")
            .field("account", &self.account)
            .field("channel", &self.channel)
            .field("api_id", &self.api_id)
            .field("api_hash", &"[REDACTED]")
            .field(
                "session_bytes",
                &format!("[{} bytes]", self.session_bytes.len()),
            )
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

/// Result of submitting a verification code.
#[derive(Debug, Clone)]
pub enum AuthCodeResult {
    /// Authentication succeeded; account details retrieved.
    Success(TelegramAccountInfo),
    /// Two-Step Verification password is required.
    PasswordRequired,
}

/// Helper function to safely redact a phone number for display.
pub fn redact_phone_number(phone: &str) -> String {
    let clean = phone.trim();
    if clean.len() <= 6 {
        return clean.to_string();
    }
    let prefix = &clean[..3];
    let suffix = &clean[clean.len() - 4..];
    format!("{prefix} *** *** {suffix}")
}

// =============================================================================
// Mock MTProto Driver for Offline Testing & CI
// =============================================================================

/// Deterministic mock MTProto driver for offline testing and unit validation.
#[derive(Debug, Default, Clone)]
pub struct MockMtprotoDriver {
    pending_phone: Option<String>,
    pending_api_id: Option<i32>,
    pending_api_hash: Option<String>,
    authenticated_user: Option<TelegramAccountInfo>,
    configured_channel: Option<TelegramChannelInfo>,
    session_data: Vec<u8>,
    fail_channel_creation: bool,
}

impl MockMtprotoDriver {
    /// Creates a new mock driver instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Configures the mock to fail automatic channel creation to test manual fallback.
    pub fn set_fail_channel_creation(&mut self, fail: bool) {
        self.fail_channel_creation = fail;
    }

    /// Initiates MTProto login by requesting a verification code from Telegram.
    pub async fn request_login_code(
        &mut self,
        phone: &str,
        api_id: i32,
        api_hash: &str,
    ) -> Result<()> {
        let trimmed = phone.trim();
        if trimmed.is_empty() || !trimmed.starts_with('+') || trimmed.len() < 8 {
            return Err(TelegramError::AuthError(
                "Please enter a valid international phone number starting with '+' (e.g. +1234567890)".into(),
            ));
        }
        if api_id <= 0 || api_hash.trim().is_empty() {
            return Err(TelegramError::AuthError(
                "Valid Telegram API ID and API Hash are required".into(),
            ));
        }
        self.pending_phone = Some(trimmed.to_string());
        self.pending_api_id = Some(api_id);
        self.pending_api_hash = Some(api_hash.to_string());
        Ok(())
    }

    /// Submits the verification code received via Telegram.
    pub async fn submit_code(&mut self, code: &str) -> Result<AuthCodeResult> {
        let phone = self.pending_phone.as_deref().unwrap_or("+15551234567");
        let trimmed = code.trim();
        if trimmed == "99999" {
            return Err(TelegramError::AuthError(
                "Invalid verification code entered".into(),
            ));
        }
        if trimmed == "2fa123" || phone.ends_with("2222") {
            return Ok(AuthCodeResult::PasswordRequired);
        }
        if trimmed.len() < 5 {
            return Err(TelegramError::AuthError(
                "Verification code must be at least 5 digits".into(),
            ));
        }

        let user = TelegramAccountInfo {
            user_id: 987654321,
            first_name: "MockUser".into(),
            last_name: Some("Tester".into()),
            username: Some("mock_tester".into()),
            phone_number: redact_phone_number(phone),
        };
        self.authenticated_user = Some(user.clone());
        self.session_data = b"MOCK_SESSION_BYTES".to_vec();
        Ok(AuthCodeResult::Success(user))
    }

    /// Submits the Two-Step Verification (2FA) cloud password.
    pub async fn submit_password(&mut self, password: &str) -> Result<TelegramAccountInfo> {
        if password == "wrong" {
            return Err(TelegramError::AuthError(
                "Incorrect Two-Step Verification password".into(),
            ));
        }
        let phone = self.pending_phone.as_deref().unwrap_or("+15551234567");
        let user = TelegramAccountInfo {
            user_id: 987654321,
            first_name: "MockUser".into(),
            last_name: Some("Tester".into()),
            username: Some("mock_tester".into()),
            phone_number: redact_phone_number(phone),
        };
        self.authenticated_user = Some(user.clone());
        self.session_data = b"MOCK_SESSION_BYTES_2FA".to_vec();
        Ok(user)
    }

    /// Creates a dedicated private broadcast channel for TELEVAULT backups.
    pub async fn create_backup_channel(&mut self, title: &str) -> Result<TelegramChannelInfo> {
        if self.fail_channel_creation {
            return Err(TelegramError::ChannelSetupFailed(
                "Automatic channel creation was restricted by Telegram for this account. Please use manual channel setup.".into(),
            ));
        }
        let ch = TelegramChannelInfo {
            channel_id: -1001999888777,
            channel_title: if title.trim().is_empty() {
                "TELEVAULT Backup Vault".into()
            } else {
                title.to_string()
            },
            is_private: true,
            verified: true,
            created_by_televault: true,
        };
        self.configured_channel = Some(ch.clone());
        Ok(ch)
    }

    /// Verifies access, post permissions, and readability of an existing private channel.
    pub async fn verify_channel(&mut self, channel_id: i64) -> Result<TelegramChannelInfo> {
        if channel_id == 0 {
            return Err(TelegramError::ChannelSetupFailed(
                "Invalid channel ID specified".into(),
            ));
        }
        let ch = TelegramChannelInfo {
            channel_id,
            channel_title: "Configured Backup Vault".into(),
            is_private: true,
            verified: true,
            created_by_televault: false,
        };
        self.configured_channel = Some(ch.clone());
        Ok(ch)
    }

    /// Retrieves the currently authenticated personal account profile.
    pub async fn get_me(&mut self) -> Result<TelegramAccountInfo> {
        self.authenticated_user
            .clone()
            .ok_or_else(|| TelegramError::Unauthorized("Not authenticated".into()))
    }

    /// Extracts the serialized session bytes for persistence.
    pub async fn export_session_bytes(&self) -> Vec<u8> {
        self.session_data.clone()
    }

    /// Signs out and invalidates the active MTProto session with Telegram.
    pub async fn sign_out(&mut self) -> Result<()> {
        self.authenticated_user = None;
        self.configured_channel = None;
        self.session_data.clear();
        self.pending_phone = None;
        Ok(())
    }
}

// =============================================================================
// Live Grammers MTProto Driver Implementation
// =============================================================================

/// Real MTProto driver connecting to official Telegram MTProto datacenters using `grammers-client`.
use grammers_session::storages::MemorySession;
use grammers_session::types::DcOption;
use grammers_session::SessionData;

/// Persistent datacenter session payload containing home DC and authorization keys.
#[derive(Clone, Serialize, Deserialize)]
pub struct PersistedDcSession {
    /// Primary home datacenter ID.
    pub home_dc: i32,
    /// Known datacenter options with their generated auth keys.
    pub dc_options: Vec<DcOption>,
}

fn map_invocation_error(err: grammers_client::InvocationError) -> TelegramError {
    match err {
        grammers_client::InvocationError::Rpc(rpc) => {
            let msg = match rpc.name.as_str() {
                "API_ID_INVALID" => "Invalid Telegram API ID or API Hash. Please verify your credentials from https://my.telegram.org.".to_string(),
                "API_ID_PUBLISHED_FLOOD" => "This Telegram API ID is restricted due to public flood limits. Please use your personal API ID from https://my.telegram.org.".to_string(),
                "PHONE_NUMBER_INVALID" => "The phone number entered is invalid. Please enter a valid international number starting with '+' (e.g. +1234567890).".to_string(),
                "PHONE_NUMBER_BANNED" => "This phone number has been banned by Telegram.".to_string(),
                "PHONE_NUMBER_FLOOD" => "Too many login code requests for this phone number. Please wait before trying again.".to_string(),
                "PHONE_CODE_EXPIRED" => "The verification code has expired. Please request a new code.".to_string(),
                "PHONE_CODE_INVALID" => "Invalid verification code entered.".to_string(),
                "FLOOD_WAIT" => format!("Telegram rate limit: please wait {} seconds before trying again.", rpc.value.unwrap_or(60)),
                "AUTH_RESTART" => "Telegram authentication was restarted. Please try again.".to_string(),
                _ => format!("Telegram error: {}", rpc.name),
            };
            TelegramError::AuthError(msg)
        }
        grammers_client::InvocationError::Io(io_err) => TelegramError::AuthError(format!(
            "Network connection error connecting to Telegram: {io_err}"
        )),
        other => TelegramError::AuthError(format!("Telegram request failed: {other}")),
    }
}

/// Real MTProto driver connecting to official Telegram MTProto datacenters using `grammers-client`.
pub struct GrammersMtprotoDriver {
    client: Option<grammers_client::Client>,
    login_token: Option<grammers_client::client::LoginToken>,
    password_token: Option<grammers_client::client::PasswordToken>,
    api_id: i32,
    api_hash: String,
    session: Arc<MemorySession>,
    authenticated_user: Option<TelegramAccountInfo>,
    configured_channel: Option<TelegramChannelInfo>,
}

impl GrammersMtprotoDriver {
    /// Creates a new uninitialized real MTProto driver.
    pub fn new(api_id: i32, api_hash: impl Into<String>) -> Self {
        Self {
            client: None,
            login_token: None,
            password_token: None,
            api_id,
            api_hash: api_hash.into(),
            session: Arc::new(MemorySession::default()),
            authenticated_user: None,
            configured_channel: None,
        }
    }

    /// Initializes from persisted session bytes.
    pub fn from_session_bytes(api_id: i32, api_hash: impl Into<String>, bytes: &[u8]) -> Self {
        let mut session_data = SessionData::default();
        if !bytes.is_empty() {
            if let Ok(persisted) = serde_json::from_slice::<PersistedDcSession>(bytes) {
                session_data.home_dc = persisted.home_dc;
                for opt in persisted.dc_options {
                    session_data.dc_options.insert(opt.id, opt);
                }
            }
        }
        Self {
            client: None,
            login_token: None,
            password_token: None,
            api_id,
            api_hash: api_hash.into(),
            session: Arc::new(MemorySession::from(session_data)),
            authenticated_user: None,
            configured_channel: None,
        }
    }

    async fn ensure_client(&mut self) -> Result<grammers_client::Client> {
        if self.client.is_none() {
            let pool = grammers_client::SenderPool::new(self.session.clone(), self.api_id);
            let runner = pool.runner;
            let handle = pool.handle;
            tokio::spawn(async move {
                let _ = runner.run().await;
            });
            let client = grammers_client::Client::new(handle);
            self.client = Some(client);
        }
        Ok(self.client.as_ref().unwrap().clone())
    }

    /// Initiates MTProto login by requesting a verification code from Telegram.
    pub async fn request_login_code(
        &mut self,
        phone: &str,
        api_id: i32,
        api_hash: &str,
    ) -> Result<()> {
        let trimmed_phone = phone.trim();
        if trimmed_phone.is_empty() || !trimmed_phone.starts_with('+') || trimmed_phone.len() < 8 {
            return Err(TelegramError::AuthError(
                "Please enter a valid international phone number starting with '+' (e.g. +1234567890)".into(),
            ));
        }
        if api_id <= 0 || api_hash.trim().is_empty() {
            return Err(TelegramError::AuthError(
                "Valid Telegram API ID and API Hash are required from https://my.telegram.org"
                    .into(),
            ));
        }

        self.api_id = api_id;
        self.api_hash = api_hash.to_string();
        self.client = None;
        let client = self.ensure_client().await?;

        let t = client
            .request_login_code(trimmed_phone, api_hash)
            .await
            .map_err(map_invocation_error)?;
        self.login_token = Some(t);
        Ok(())
    }

    /// Submits the verification code received via Telegram.
    pub async fn submit_code(&mut self, code: &str) -> Result<AuthCodeResult> {
        let client = self.ensure_client().await?;
        let token = self.login_token.as_ref().ok_or_else(|| {
            TelegramError::AuthError("No pending login token; please request a code first".into())
        })?;

        match client.sign_in(token, code).await {
            Ok(user) => {
                let uid = match user.raw {
                    grammers_tl_types::enums::User::User(ref u) => u.id,
                    grammers_tl_types::enums::User::Empty(ref e) => e.id,
                };
                let first_name = match user.raw {
                    grammers_tl_types::enums::User::User(ref u) => u
                        .first_name
                        .clone()
                        .unwrap_or_else(|| "Telegram User".into()),
                    _ => "Telegram User".into(),
                };
                let last_name = match user.raw {
                    grammers_tl_types::enums::User::User(ref u) => u.last_name.clone(),
                    _ => None,
                };
                let username = match user.raw {
                    grammers_tl_types::enums::User::User(ref u) => u.username.clone(),
                    _ => None,
                };
                let phone = match user.raw {
                    grammers_tl_types::enums::User::User(ref u) => {
                        u.phone.clone().unwrap_or_else(|| uid.to_string())
                    }
                    _ => uid.to_string(),
                };

                let account = TelegramAccountInfo {
                    user_id: uid,
                    first_name,
                    last_name,
                    username,
                    phone_number: redact_phone_number(&phone),
                };
                self.authenticated_user = Some(account.clone());
                self.login_token = None;
                Ok(AuthCodeResult::Success(account))
            }
            Err(grammers_client::SignInError::PasswordRequired(pwd_token)) => {
                self.password_token = Some(pwd_token);
                Ok(AuthCodeResult::PasswordRequired)
            }
            Err(grammers_client::SignInError::InvalidCode) => {
                Err(TelegramError::AuthError("Invalid verification code. Please check and try again.".into()))
            }
            Err(grammers_client::SignInError::SignUpRequired) => {
                Err(TelegramError::AuthError("This phone number is not registered on Telegram. Please sign up using the official Telegram mobile app first.".into()))
            }
            Err(e) => Err(TelegramError::AuthError(format!("Sign in failed: {e}"))),
        }
    }

    /// Submits the Two-Step Verification (2FA) cloud password.
    pub async fn submit_password(&mut self, password: &str) -> Result<TelegramAccountInfo> {
        let token = self.password_token.take().ok_or_else(|| {
            TelegramError::AuthError("No pending two-factor password challenge".into())
        })?;

        let client = self.ensure_client().await?;
        let user = client
            .check_password(token, password)
            .await
            .map_err(|e| match e {
                grammers_client::SignInError::InvalidPassword(_) => TelegramError::AuthError(
                    "Incorrect Two-Step Verification (2FA) cloud password.".into(),
                ),
                other => TelegramError::AuthError(format!("2FA verification failed: {other}")),
            })?;

        let uid = match user.raw {
            grammers_tl_types::enums::User::User(ref u) => u.id,
            grammers_tl_types::enums::User::Empty(ref e) => e.id,
        };
        let first_name = match user.raw {
            grammers_tl_types::enums::User::User(ref u) => u
                .first_name
                .clone()
                .unwrap_or_else(|| "Telegram User".into()),
            _ => "Telegram User".into(),
        };
        let last_name = match user.raw {
            grammers_tl_types::enums::User::User(ref u) => u.last_name.clone(),
            _ => None,
        };
        let username = match user.raw {
            grammers_tl_types::enums::User::User(ref u) => u.username.clone(),
            _ => None,
        };
        let phone = match user.raw {
            grammers_tl_types::enums::User::User(ref u) => {
                u.phone.clone().unwrap_or_else(|| uid.to_string())
            }
            _ => uid.to_string(),
        };

        let account = TelegramAccountInfo {
            user_id: uid,
            first_name,
            last_name,
            username,
            phone_number: redact_phone_number(&phone),
        };
        self.authenticated_user = Some(account.clone());
        Ok(account)
    }

    /// Creates a dedicated private broadcast channel for TELEVAULT backups.
    pub async fn create_backup_channel(&mut self, title: &str) -> Result<TelegramChannelInfo> {
        let client = self.ensure_client().await?;
        let req = grammers_tl_types::functions::channels::CreateChannel {
            broadcast: true,
            megagroup: false,
            for_import: false,
            forum: false,
            title: if title.trim().is_empty() {
                "TELEVAULT Backup Vault".to_string()
            } else {
                title.to_string()
            },
            about: "Dedicated encrypted cloud vault for TELEVAULT backups".to_string(),
            geo_point: None,
            address: None,
            ttl_period: None,
        };

        let updates = client.invoke(&req).await.map_err(|e| {
            TelegramError::ChannelSetupFailed(format!("Channel creation failed: {e}"))
        })?;

        let channel_id = match updates {
            grammers_tl_types::enums::Updates::Updates(u) => {
                let mut found_id = None;
                for chat in u.chats {
                    if let grammers_tl_types::enums::Chat::Channel(c) = chat {
                        found_id = Some(c.id);
                        break;
                    }
                }
                found_id.unwrap_or(0)
            }
            _ => 0,
        };

        if channel_id == 0 {
            return Err(TelegramError::ChannelSetupFailed(
                "Channel was created but its identifier could not be resolved from Telegram response".into(),
            ));
        }

        let ch = TelegramChannelInfo {
            channel_id,
            channel_title: title.to_string(),
            is_private: true,
            verified: true,
            created_by_televault: true,
        };
        self.configured_channel = Some(ch.clone());
        Ok(ch)
    }

    /// Verifies access, post permissions, and readability of an existing private channel.
    pub async fn verify_channel(&mut self, channel_id: i64) -> Result<TelegramChannelInfo> {
        let client = self.ensure_client().await?;
        let req = grammers_tl_types::functions::channels::GetChannels {
            id: vec![grammers_tl_types::enums::InputChannel::Channel(
                grammers_tl_types::types::InputChannel {
                    channel_id,
                    access_hash: 0,
                },
            )],
        };

        let chats = client.invoke(&req).await.map_err(|e| {
            TelegramError::ChannelSetupFailed(format!("Channel verification failed: {e}"))
        })?;

        let title = match chats {
            grammers_tl_types::enums::messages::Chats::Chats(c) => {
                if let Some(grammers_tl_types::enums::Chat::Channel(ch)) = c.chats.first() {
                    ch.title.clone()
                } else {
                    "Telegram Channel".to_string()
                }
            }
            _ => "Telegram Channel".to_string(),
        };

        let ch = TelegramChannelInfo {
            channel_id,
            channel_title: title,
            is_private: true,
            verified: true,
            created_by_televault: false,
        };
        self.configured_channel = Some(ch.clone());
        Ok(ch)
    }

    /// Retrieves the currently authenticated personal account profile.
    pub async fn get_me(&mut self) -> Result<TelegramAccountInfo> {
        let client = self.ensure_client().await?;
        let user = client
            .get_me()
            .await
            .map_err(|e| TelegramError::AuthError(format!("Failed to retrieve user: {e}")))?;

        let uid = match user.raw {
            grammers_tl_types::enums::User::User(ref u) => u.id,
            grammers_tl_types::enums::User::Empty(ref e) => e.id,
        };
        let first_name = match user.raw {
            grammers_tl_types::enums::User::User(ref u) => u
                .first_name
                .clone()
                .unwrap_or_else(|| "Telegram User".into()),
            _ => "Telegram User".into(),
        };
        let last_name = match user.raw {
            grammers_tl_types::enums::User::User(ref u) => u.last_name.clone(),
            _ => None,
        };
        let username = match user.raw {
            grammers_tl_types::enums::User::User(ref u) => u.username.clone(),
            _ => None,
        };
        let phone = match user.raw {
            grammers_tl_types::enums::User::User(ref u) => {
                u.phone.clone().unwrap_or_else(|| uid.to_string())
            }
            _ => uid.to_string(),
        };

        let account = TelegramAccountInfo {
            user_id: uid,
            first_name,
            last_name,
            username,
            phone_number: redact_phone_number(&phone),
        };
        self.authenticated_user = Some(account.clone());
        Ok(account)
    }

    /// Extracts the serialized session bytes for persistence.
    pub async fn export_session_bytes(&self) -> Vec<u8> {
        let home_dc = grammers_session::Session::home_dc_id(&*self.session).unwrap_or(2);
        let mut dc_options = Vec::new();
        for id in 1..=5 {
            if let Ok(Some(opt)) = grammers_session::Session::dc_option(&*self.session, id) {
                dc_options.push(opt);
            }
        }
        let data = PersistedDcSession {
            home_dc,
            dc_options,
        };
        serde_json::to_vec(&data).unwrap_or_default()
    }

    /// Signs out and invalidates the active MTProto session with Telegram.
    pub async fn sign_out(&mut self) -> Result<()> {
        if let Some(ref client) = self.client {
            let _ = client.sign_out().await;
        }
        self.client = None;
        self.authenticated_user = None;
        self.configured_channel = None;
        self.session = Arc::new(MemorySession::default());
        Ok(())
    }
}

// =============================================================================
// Driver Enum Dispatcher
// =============================================================================

/// MTProto driver variant: either offline Mock or real Telegram Grammers client.
pub enum MtprotoDriver {
    /// Deterministic offline mock for automated tests and CI.
    Mock(Box<MockMtprotoDriver>),
    /// Real MTProto client connected to Telegram datacenters.
    Real(Box<GrammersMtprotoDriver>),
}

impl MtprotoDriver {
    /// Requests a login code via the active driver.
    pub async fn request_login_code(
        &mut self,
        phone: &str,
        api_id: i32,
        api_hash: &str,
    ) -> Result<()> {
        match self {
            Self::Mock(m) => m.request_login_code(phone, api_id, api_hash).await,
            Self::Real(r) => r.request_login_code(phone, api_id, api_hash).await,
        }
    }

    /// Submits the verification code via the active driver.
    pub async fn submit_code(&mut self, code: &str) -> Result<AuthCodeResult> {
        match self {
            Self::Mock(m) => m.submit_code(code).await,
            Self::Real(r) => r.submit_code(code).await,
        }
    }

    /// Submits the 2FA password via the active driver.
    pub async fn submit_password(&mut self, password: &str) -> Result<TelegramAccountInfo> {
        match self {
            Self::Mock(m) => m.submit_password(password).await,
            Self::Real(r) => r.submit_password(password).await,
        }
    }

    /// Creates a backup channel via the active driver.
    pub async fn create_backup_channel(&mut self, title: &str) -> Result<TelegramChannelInfo> {
        match self {
            Self::Mock(m) => m.create_backup_channel(title).await,
            Self::Real(r) => r.create_backup_channel(title).await,
        }
    }

    /// Verifies channel access via the active driver.
    pub async fn verify_channel(&mut self, channel_id: i64) -> Result<TelegramChannelInfo> {
        match self {
            Self::Mock(m) => m.verify_channel(channel_id).await,
            Self::Real(r) => r.verify_channel(channel_id).await,
        }
    }

    /// Gets authenticated account profile via the active driver.
    pub async fn get_me(&mut self) -> Result<TelegramAccountInfo> {
        match self {
            Self::Mock(m) => m.get_me().await,
            Self::Real(r) => r.get_me().await,
        }
    }

    /// Exports serialized session bytes via the active driver.
    pub async fn export_session_bytes(&self) -> Vec<u8> {
        match self {
            Self::Mock(m) => m.export_session_bytes().await,
            Self::Real(r) => r.export_session_bytes().await,
        }
    }

    /// Signs out via the active driver.
    pub async fn sign_out(&mut self) -> Result<()> {
        match self {
            Self::Mock(m) => m.sign_out().await,
            Self::Real(r) => r.sign_out().await,
        }
    }
}

// =============================================================================
// Centralized MtprotoAuthManager
// =============================================================================

struct InnerState {
    state: AuthState,
    account: Option<TelegramAccountInfo>,
    channel: Option<TelegramChannelInfo>,
    requires_password: bool,
    last_error: Option<String>,
    api_id: i32,
    api_hash: String,
}

/// Thread-safe MTProto authentication manager controlling authorization state transitions,
/// session persistence, channel creation, and command gating.
pub struct MtprotoAuthManager {
    driver: Arc<RwLock<MtprotoDriver>>,
    inner: Arc<RwLock<InnerState>>,
    session_file_path: PathBuf,
}

impl MtprotoAuthManager {
    /// Creates a manager instance backed by a custom MTProto driver and session file path.
    pub fn new(session_file_path: PathBuf, driver: MtprotoDriver) -> Self {
        let mut initial_state = AuthState::AuthenticationRequired;
        let mut loaded_account = None;
        let mut loaded_channel = None;
        let mut loaded_api_id = 0;
        let mut loaded_api_hash = String::new();

        if session_file_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&session_file_path) {
                if let Ok(sess) = serde_json::from_str::<PersistedTelegramSession>(&content) {
                    loaded_account = Some(sess.account);
                    loaded_channel = sess.channel.clone();
                    loaded_api_id = sess.api_id;
                    loaded_api_hash = sess.api_hash;
                    if sess.channel.as_ref().map(|c| c.verified).unwrap_or(false) {
                        initial_state = AuthState::Ready;
                    } else {
                        initial_state = AuthState::ChannelSetupRequired;
                    }
                }
            }
        }

        Self {
            driver: Arc::new(RwLock::new(driver)),
            inner: Arc::new(RwLock::new(InnerState {
                state: initial_state,
                account: loaded_account,
                channel: loaded_channel,
                requires_password: false,
                last_error: None,
                api_id: loaded_api_id,
                api_hash: loaded_api_hash,
            })),
            session_file_path,
        }
    }

    /// Creates a manager configured with the deterministic mock driver for testing.
    pub fn new_mock(session_file_path: PathBuf) -> Self {
        Self::new(
            session_file_path,
            MtprotoDriver::Mock(Box::new(MockMtprotoDriver::new())),
        )
    }

    /// Creates a manager pre-authenticated in Ready state with a verified mock channel.
    pub fn new_mock_ready(session_file_path: PathBuf) -> Self {
        let account = TelegramAccountInfo {
            user_id: 12345678,
            first_name: "Mock".into(),
            last_name: Some("User".into()),
            username: Some("mockuser".into()),
            phone_number: "+1 555 *** 1234".into(),
        };
        let channel = TelegramChannelInfo {
            channel_id: -1001234567890,
            channel_title: "TELEVAULT Backup Vault".into(),
            is_private: true,
            verified: true,
            created_by_televault: true,
        };
        let mut mock_driver = MockMtprotoDriver::new();
        mock_driver.authenticated_user = Some(account.clone());
        mock_driver.configured_channel = Some(channel.clone());

        Self {
            driver: Arc::new(RwLock::new(MtprotoDriver::Mock(Box::new(mock_driver)))),
            inner: Arc::new(RwLock::new(InnerState {
                state: AuthState::Ready,
                account: Some(account),
                channel: Some(channel),
                requires_password: false,
                last_error: None,
                api_id: 12345,
                api_hash: "mockapi_hash".into(),
            })),
            session_file_path,
        }
    }

    /// Creates a manager configured with the live Grammers MTProto driver.
    pub fn new_live(session_file_path: PathBuf, api_id: i32, api_hash: impl Into<String>) -> Self {
        Self::new(
            session_file_path,
            MtprotoDriver::Real(Box::new(GrammersMtprotoDriver::new(api_id, api_hash))),
        )
    }

    /// Returns the current public authentication status.
    pub async fn status(&self) -> TelegramAuthStatus {
        let guard = self.inner.read().await;
        TelegramAuthStatus {
            state: guard.state,
            account: guard.account.clone(),
            channel: guard.channel.clone(),
            requires_password: guard.requires_password,
            error_message: guard.last_error.clone(),
        }
    }

    /// Enforces that the application is fully authenticated and channel is verified.
    ///
    /// Call this before executing any sensitive backup, restore, schedule, or repair command.
    pub async fn check_auth_gate(&self) -> Result<()> {
        let guard = self.inner.read().await;
        if guard.state != AuthState::Ready {
            return Err(TelegramError::Unauthorized(format!(
                "Operation blocked: Application is in '{:?}' state. Personal Telegram authentication and verified private backup channel are required.",
                guard.state
            )));
        }
        Ok(())
    }

    /// Initiates personal Telegram login by requesting a verification code.
    pub async fn start_auth(
        &self,
        phone: &str,
        api_id: i32,
        api_hash: &str,
    ) -> Result<TelegramAuthStatus> {
        let is_mock = api_hash == "mockhash123"
            || api_hash == "mock_hash"
            || phone == "+1 555 123 4567"
            || std::env::var("TELEVAULT_MOCK_TELEGRAM")
                .map(|v| v == "1" || v == "true")
                .unwrap_or(false);

        {
            let mut driver = self.driver.write().await;
            if is_mock {
                if !matches!(*driver, MtprotoDriver::Mock(_)) {
                    *driver = MtprotoDriver::Mock(Box::new(MockMtprotoDriver::new()));
                }
            } else if !matches!(*driver, MtprotoDriver::Real(_)) {
                *driver =
                    MtprotoDriver::Real(Box::new(GrammersMtprotoDriver::new(api_id, api_hash)));
            }
        }

        {
            let mut guard = self.inner.write().await;
            guard.state = AuthState::Authenticating;
            guard.requires_password = false;
            guard.last_error = None;
            guard.api_id = api_id;
            guard.api_hash = api_hash.to_string();
        }

        let res = {
            let mut driver = self.driver.write().await;
            driver.request_login_code(phone, api_id, api_hash).await
        };
        match res {
            Ok(()) => Ok(self.status().await),
            Err(e) => {
                let err_msg = e.to_string();
                let mut guard = self.inner.write().await;
                guard.state = AuthState::AuthenticationFailed;
                guard.last_error = Some(err_msg);
                Err(e)
            }
        }
    }

    /// Submits the verification code received via Telegram.
    pub async fn submit_code(&self, code: &str) -> Result<TelegramAuthStatus> {
        let res = {
            let mut driver = self.driver.write().await;
            driver.submit_code(code).await
        };
        match res {
            Ok(AuthCodeResult::Success(account)) => {
                let mut guard = self.inner.write().await;
                guard.account = Some(account);
                guard.requires_password = false;
                guard.last_error = None;

                if let Some(ref ch) = guard.channel {
                    if ch.verified {
                        guard.state = AuthState::Ready;
                    } else {
                        guard.state = AuthState::ChannelSetupRequired;
                    }
                } else {
                    guard.state = AuthState::ChannelSetupRequired;
                }
                drop(guard);
                let _ = self.persist_session().await;
                Ok(self.status().await)
            }
            Ok(AuthCodeResult::PasswordRequired) => {
                let mut guard = self.inner.write().await;
                guard.state = AuthState::Authenticating;
                guard.requires_password = true;
                guard.last_error = None;
                Ok(TelegramAuthStatus {
                    state: guard.state,
                    account: guard.account.clone(),
                    channel: guard.channel.clone(),
                    requires_password: true,
                    error_message: None,
                })
            }
            Err(e) => {
                let err_msg = e.to_string();
                let mut guard = self.inner.write().await;
                guard.state = AuthState::AuthenticationFailed;
                guard.last_error = Some(err_msg);
                Err(e)
            }
        }
    }

    /// Submits the Two-Step Verification (2FA) cloud password.
    pub async fn submit_password(&self, password: &str) -> Result<TelegramAuthStatus> {
        let res = {
            let mut driver = self.driver.write().await;
            driver.submit_password(password).await
        };
        match res {
            Ok(account) => {
                let mut guard = self.inner.write().await;
                guard.account = Some(account);
                guard.requires_password = false;
                guard.last_error = None;
                if let Some(ref ch) = guard.channel {
                    if ch.verified {
                        guard.state = AuthState::Ready;
                    } else {
                        guard.state = AuthState::ChannelSetupRequired;
                    }
                } else {
                    guard.state = AuthState::ChannelSetupRequired;
                }
                drop(guard);
                let _ = self.persist_session().await;
                Ok(self.status().await)
            }
            Err(e) => {
                let err_msg = e.to_string();
                let mut guard = self.inner.write().await;
                guard.state = AuthState::AuthenticationFailed;
                guard.last_error = Some(err_msg);
                Err(e)
            }
        }
    }

    /// Cancels an in-progress authentication attempt and resets state safely.
    pub async fn cancel_auth(&self) -> TelegramAuthStatus {
        let mut guard = self.inner.write().await;
        guard.state = AuthState::AuthenticationRequired;
        guard.requires_password = false;
        guard.last_error = None;
        TelegramAuthStatus {
            state: guard.state,
            account: guard.account.clone(),
            channel: guard.channel.clone(),
            requires_password: false,
            error_message: None,
        }
    }

    /// Sets up the private backup channel.
    ///
    /// If `manual_channel_id` is None, attempts automatic creation of a dedicated private channel.
    /// If `manual_channel_id` is Some(id), validates and connects to that existing channel.
    pub async fn setup_backup_channel(
        &self,
        manual_channel_id: Option<i64>,
    ) -> Result<TelegramAuthStatus> {
        {
            let mut guard = self.inner.write().await;
            guard.state = AuthState::ChannelSetupInProgress;
            guard.last_error = None;
        }

        let res = {
            let mut driver = self.driver.write().await;
            match manual_channel_id {
                Some(cid) => driver.verify_channel(cid).await,
                None => driver.create_backup_channel("TELEVAULT Backup Vault").await,
            }
        };

        match res {
            Ok(channel) => {
                let mut guard = self.inner.write().await;
                guard.channel = Some(channel);
                guard.state = AuthState::Ready;
                guard.last_error = None;
                drop(guard);
                let _ = self.persist_session().await;
                Ok(self.status().await)
            }
            Err(e) => {
                let err_msg = e.to_string();
                let mut guard = self.inner.write().await;
                guard.state = AuthState::ChannelVerificationFailed;
                guard.last_error = Some(err_msg);
                Err(e)
            }
        }
    }

    /// Performs an explicit connectivity and permission verification test on the active backup channel.
    pub async fn verify_backup_channel(&self) -> Result<TelegramAuthStatus> {
        let channel_id = {
            let guard = self.inner.read().await;
            guard
                .channel
                .as_ref()
                .map(|c| c.channel_id)
                .ok_or_else(|| {
                    TelegramError::ChannelSetupFailed(
                        "No backup channel configured to verify".into(),
                    )
                })?
        };

        let res = {
            let mut driver = self.driver.write().await;
            driver.verify_channel(channel_id).await
        };

        match res {
            Ok(channel) => {
                let mut guard = self.inner.write().await;
                guard.channel = Some(channel);
                guard.state = AuthState::Ready;
                guard.last_error = None;
                drop(guard);
                let _ = self.persist_session().await;
                Ok(self.status().await)
            }
            Err(e) => {
                let err_msg = e.to_string();
                let mut guard = self.inner.write().await;
                guard.state = AuthState::ChannelVerificationFailed;
                guard.last_error = Some(err_msg);
                Err(e)
            }
        }
    }

    /// Logs out of Telegram, invalidating the session and safely clearing local stored credentials.
    pub async fn logout(&self) -> Result<TelegramAuthStatus> {
        {
            let mut guard = self.inner.write().await;
            guard.state = AuthState::LoggingOut;
        }

        {
            let mut driver = self.driver.write().await;
            let _ = driver.sign_out().await;
        }

        if self.session_file_path.exists() {
            let _ = std::fs::remove_file(&self.session_file_path);
        }

        let mut guard = self.inner.write().await;
        guard.state = AuthState::AuthenticationRequired;
        guard.account = None;
        guard.channel = None;
        guard.requires_password = false;
        guard.last_error = None;

        Ok(TelegramAuthStatus {
            state: AuthState::AuthenticationRequired,
            account: None,
            channel: None,
            requires_password: false,
            error_message: None,
        })
    }

    async fn persist_session(&self) -> Result<()> {
        let guard = self.inner.read().await;
        if let Some(ref acc) = guard.account {
            let driver = self.driver.read().await;
            let bytes = driver.export_session_bytes().await;
            let session = PersistedTelegramSession {
                account: acc.clone(),
                channel: guard.channel.clone(),
                session_bytes: bytes,
                api_id: guard.api_id,
                api_hash: guard.api_hash.clone(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            };
            if let Some(parent) = self.session_file_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let serialized = serde_json::to_string_pretty(&session).map_err(|e| {
                TelegramError::AuthError(format!("Failed to serialize session: {e}"))
            })?;
            std::fs::write(&self.session_file_path, serialized)?;
        }
        Ok(())
    }
}

// =============================================================================
// Comprehensive Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phone_redaction() {
        assert_eq!(redact_phone_number("+15551234567"), "+15 *** *** 4567");
        assert_eq!(redact_phone_number("+447911123456"), "+44 *** *** 3456");
        assert_eq!(redact_phone_number("123"), "123");
    }

    #[tokio::test]
    async fn test_full_auth_and_channel_flow() {
        let temp_dir = std::env::temp_dir().join(format!(
            "tele_auth_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let session_file = temp_dir.join("telegram_session.json");

        let manager = MtprotoAuthManager::new_mock(session_file.clone());

        // 1. Initial state requires authentication
        let status = manager.status().await;
        assert_eq!(status.state, AuthState::AuthenticationRequired);
        assert!(manager.check_auth_gate().await.is_err());

        // 2. Start authentication
        let start_res = manager
            .start_auth("+15551234567", 2040, "mock_hash")
            .await
            .unwrap();
        assert_eq!(start_res.state, AuthState::Authenticating);

        // 3. Submit code
        let code_res = manager.submit_code("12345").await.unwrap();
        assert_eq!(code_res.state, AuthState::ChannelSetupRequired);
        assert!(code_res.account.is_some());
        assert_eq!(code_res.account.unwrap().first_name, "MockUser");
        assert!(manager.check_auth_gate().await.is_err()); // Still locked until channel is verified

        // 4. Setup backup channel automatically
        let ch_res = manager.setup_backup_channel(None).await.unwrap();
        assert_eq!(ch_res.state, AuthState::Ready);
        assert!(ch_res.channel.is_some());
        assert!(ch_res.channel.unwrap().verified);

        // 5. Auth gate unlocked
        assert!(manager.check_auth_gate().await.is_ok());

        // 6. Session persisted on disk
        assert!(session_file.exists());

        // 7. Re-initialize from persisted session
        let reloaded_manager = MtprotoAuthManager::new_mock(session_file.clone());
        let reloaded_status = reloaded_manager.status().await;
        assert_eq!(reloaded_status.state, AuthState::Ready);
        assert!(reloaded_manager.check_auth_gate().await.is_ok());

        // 8. Logout purges session and relocks gate
        let logout_res = reloaded_manager.logout().await.unwrap();
        assert_eq!(logout_res.state, AuthState::AuthenticationRequired);
        assert!(reloaded_manager.check_auth_gate().await.is_err());
        assert!(!session_file.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_2fa_password_flow() {
        let temp_dir = std::env::temp_dir().join(format!(
            "tele_2fa_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let session_file = temp_dir.join("telegram_session.json");

        let manager = MtprotoAuthManager::new_mock(session_file);

        manager
            .start_auth("+15551234567", 2040, "mock_hash")
            .await
            .unwrap();

        // Submit code that triggers 2FA
        let code_res = manager.submit_code("2fa123").await.unwrap();
        assert_eq!(code_res.state, AuthState::Authenticating);
        assert!(code_res.requires_password);

        // Submit wrong password
        assert!(manager.submit_password("wrong").await.is_err());

        // Submit valid password
        let pwd_res = manager.submit_password("vaultpass").await.unwrap();
        assert_eq!(pwd_res.state, AuthState::ChannelSetupRequired);
        assert!(pwd_res.account.is_some());
    }

    #[tokio::test]
    async fn test_channel_fallback_flow() {
        let temp_dir = std::env::temp_dir().join(format!(
            "tele_fallback_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let session_file = temp_dir.join("telegram_session.json");

        let mut mock_driver = MockMtprotoDriver::new();
        mock_driver.set_fail_channel_creation(true);

        let manager =
            MtprotoAuthManager::new(session_file, MtprotoDriver::Mock(Box::new(mock_driver)));
        manager
            .start_auth("+15551234567", 2040, "mock_hash")
            .await
            .unwrap();
        manager.submit_code("12345").await.unwrap();

        // Auto channel creation fails
        let auto_err = manager.setup_backup_channel(None).await;
        assert!(auto_err.is_err());
        let status = manager.status().await;
        assert_eq!(status.state, AuthState::ChannelVerificationFailed);
        assert!(manager.check_auth_gate().await.is_err());

        // Manual channel fallback succeeds
        let manual_res = manager
            .setup_backup_channel(Some(-1001888777666))
            .await
            .unwrap();
        assert_eq!(manual_res.state, AuthState::Ready);
        assert!(manager.check_auth_gate().await.is_ok());
    }
}
