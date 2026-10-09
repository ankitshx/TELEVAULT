//! Shared Desktop application state managed across Tauri IPC commands.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use televault_backup::checker::BackupChecker;
use televault_backup::engine::BackupEngine;
use televault_backup::repair::RepairEngine;
use televault_backup::restore::RestoreEngine;
use televault_backup::retention::RetentionEngine;
use televault_backup::verification::VerificationEngine;
use televault_core::error::AppError;
use televault_core::paths::PathManager;
use televault_db::Database;
use televault_storage::mock::MockStorageProvider;
use televault_storage::temp::TempPayloadManager;
use televault_storage::StorageProvider;
use televault_telegram::{
    HttpTelegramTransport, MtprotoAuthManager, TelegramCredentials, TelegramStorageConfig,
    TelegramStorageProvider, TelegramTransport,
};
use televault_transfer::cancellation::CancellationToken;
use televault_transfer::engine::{TransferEngine, TransferEngineConfig};

use crate::dto::{
    AuthStateDto, SaveTelegramConfigDto, StartupRecoveryReportDto, TelegramAccountInfoDto,
    TelegramAuthStatusDto, TelegramChannelInfoDto, TelegramConnectionStatus,
    TelegramConnectionTestResultDto, TelegramStatusDto,
};
use crate::error::IpcError;

/// Thread-safe delegating storage provider routing operations to the active backend.
///
/// Enables seamless runtime reconfiguration between unconfigured/mock and production
/// Telegram storage backends without restarting the desktop application.
pub struct DynamicStorageProvider {
    inner: Arc<RwLock<Arc<dyn StorageProvider + Send + Sync>>>,
}

impl DynamicStorageProvider {
    /// Creates a new [`DynamicStorageProvider`] wrapping the initial provider.
    pub fn new(initial: Arc<dyn StorageProvider + Send + Sync>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(initial)),
        }
    }

    /// Dynamically switches the active storage provider.
    pub fn set_active(&self, provider: Arc<dyn StorageProvider + Send + Sync>) {
        let mut guard = self.inner.write().unwrap();
        *guard = provider;
    }

    /// Returns a clone of the currently active storage provider.
    pub fn active(&self) -> Arc<dyn StorageProvider + Send + Sync> {
        let guard = self.inner.read().unwrap();
        Arc::clone(&*guard)
    }

    /// Returns the name of the currently active storage provider.
    pub fn active_name(&self) -> &'static str {
        self.active().name()
    }
}

impl StorageProvider for DynamicStorageProvider {
    fn name(&self) -> &'static str {
        self.active().name()
    }

    fn upload(
        &self,
        request: &televault_storage::UploadRequest,
        reader: &mut dyn std::io::Read,
    ) -> Result<televault_storage::UploadResult, televault_storage::StorageError> {
        self.active().upload(request, reader)
    }

    fn download(
        &self,
        request: &televault_storage::DownloadRequest,
        writer: &mut dyn std::io::Write,
    ) -> Result<televault_storage::DownloadResult, televault_storage::StorageError> {
        self.active().download(request, writer)
    }

    fn verify(
        &self,
        request: &televault_storage::VerificationRequest,
    ) -> Result<bool, televault_storage::StorageError> {
        self.active().verify(request)
    }

    fn get_metadata(
        &self,
        reference: &televault_manifest::StorageReference,
    ) -> Result<televault_storage::RemoteObjectMetadata, televault_storage::StorageError> {
        self.active().get_metadata(reference)
    }

    fn delete(
        &self,
        request: &televault_storage::DeleteRequest,
    ) -> Result<(), televault_storage::StorageError> {
        self.active().delete(request)
    }
}

/// Authoritative managed application state shared across all Tauri commands.
///
/// Ensures thread-safe access to persistent database connections, transfer workers,
/// and domain engines without recreating infrastructure per invocation.
#[derive(Clone)]
pub struct DesktopAppState {
    /// Embedded SQLite database connection.
    pub db: Arc<Database>,
    /// Production transfer coordinator and bounded queue.
    pub transfer_engine: Arc<TransferEngine>,
    /// Domain backup engine coordinating scanning, snapshot creation, and uploads.
    pub backup_engine: Arc<BackupEngine>,
    /// Domain restore engine coordinating downloads, verification, and reconstruction.
    pub restore_engine: Arc<RestoreEngine>,
    /// Read-only checker for querying backup states and inspecting versions.
    pub checker: Arc<BackupChecker>,
    /// Deterministic filesystem paths manager.
    pub paths: Arc<PathManager>,
    /// Temporary staging lifecycle manager.
    pub temp_manager: Arc<TempPayloadManager>,
    /// Dynamic storage router delegating transfers to the active backend.
    pub dynamic_storage: Arc<DynamicStorageProvider>,
    /// Remote storage abstraction provider.
    pub storage_provider: Arc<dyn StorageProvider + Send + Sync>,
    /// Production background scheduler service coordinating automated backups.
    pub scheduler_service: Arc<televault_scheduler::SchedulerService>,
    /// Production retention policy engine coordinating snapshot metadata pruning.
    pub retention_engine: Arc<RetentionEngine>,
    /// Production remote backup verification and integrity-audit subsystem.
    pub verification_engine: Arc<VerificationEngine>,
    /// Production remote repair and chunk recovery engine.
    pub repair_engine: Arc<RepairEngine>,
    /// Registry mapping in-flight operation IDs to cooperative cancellation tokens.
    pub cancellation_registry: Arc<Mutex<HashMap<String, CancellationToken>>>,
    /// Status tracking for Telegram connectivity.
    pub telegram_status_cache: Arc<Mutex<Option<TelegramTestCache>>>,
    /// Diagnostic report from startup recovery and deterministic reconciliation.
    pub startup_recovery_report: Arc<StartupRecoveryReportDto>,
    /// Personal Telegram MTProto authentication and private channel manager.
    pub mtproto_auth: Arc<MtprotoAuthManager>,
}

/// Cached connection test result status.
#[derive(Debug, Clone)]
pub struct TelegramTestCache {
    pub status: TelegramConnectionStatus,
    pub bot_username: Option<String>,
    pub last_error: Option<String>,
}

impl DesktopAppState {
    /// Initializes application state backed by the specified base directory.
    pub fn new(
        base_dir: PathBuf,
        custom_provider: Option<Arc<dyn StorageProvider + Send + Sync>>,
    ) -> Result<Self, AppError> {
        let paths = Arc::new(PathManager::new(base_dir));
        if let Some(parent) = paths.db_file().parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::create_dir_all(paths.temp_dir())?;
        std::fs::create_dir_all(paths.logs_dir())?;
        std::fs::create_dir_all(paths.config_dir())?;

        let db = Arc::new(
            Database::open(paths.db_file())
                .map_err(|e| AppError::Io(std::io::Error::other(e.to_string())))?,
        );

        // --- Deterministic Startup Recovery & Reconciliation ---
        // 1. Run database integrity and foreign key checks
        let database_integrity_details = db
            .full_integrity_check()
            .unwrap_or_else(|e| vec![format!("Integrity check error: {e}")]);
        let database_integrity_ok = database_integrity_details
            .iter()
            .all(|s| s.eq_ignore_ascii_case("ok"));

        let foreign_key_violation_details = db.foreign_key_check().unwrap_or_default();
        let foreign_key_violations = foreign_key_violation_details.len();

        // 2. Reconcile interrupted active operations (BackingUp/Scanning snapshots and Transferring jobs)
        let (reconciled_snapshots_count, reconciled_transfers_count) =
            db.reconcile_interrupted_operations().unwrap_or((0, 0));

        // 3. Purge orphaned staging files from disk (staging files older than 0 duration on fresh startup)
        let temp_manager = Arc::new(TempPayloadManager::new(paths.temp_dir()));
        let purged_staging_files_count = temp_manager
            .cleanup_stale_staging_files(std::time::Duration::from_secs(0))
            .unwrap_or(0);

        let recovered_at = chrono::Utc::now().to_rfc3339();
        let startup_recovery_report = Arc::new(StartupRecoveryReportDto {
            database_integrity_ok,
            database_integrity_details,
            foreign_key_violations: foreign_key_violations as u32,
            foreign_key_violation_details,
            reconciled_snapshots_count: reconciled_snapshots_count as u32,
            reconciled_transfers_count: reconciled_transfers_count as u32,
            purged_staging_files_count: purged_staging_files_count as u32,
            recovered_at,
        });

        let creds_file = paths.config_dir().join("telegram_credentials.json");
        let initial_provider: Arc<dyn StorageProvider + Send + Sync> = match custom_provider {
            Some(provider) => provider,
            None => {
                if creds_file.exists() {
                    match TelegramCredentials::load_from_file(&creds_file) {
                        Ok(creds) => match HttpTelegramTransport::new(creds.clone()) {
                            Ok(transport) => {
                                let config = TelegramStorageConfig {
                                    target_chat_id: creds.target_chat_id,
                                    chunk_upload_timeout_secs: 300,
                                    max_retries: 3,
                                };
                                Arc::new(TelegramStorageProvider::new(transport, config))
                            }
                            Err(_) => Arc::new(MockStorageProvider::new()),
                        },
                        Err(_) => Arc::new(MockStorageProvider::new()),
                    }
                } else {
                    Arc::new(MockStorageProvider::new())
                }
            }
        };

        let dynamic_storage = Arc::new(DynamicStorageProvider::new(initial_provider));
        let storage_provider: Arc<dyn StorageProvider + Send + Sync> =
            Arc::clone(&dynamic_storage) as Arc<dyn StorageProvider + Send + Sync>;

        let transfer_config = TransferEngineConfig::default();
        let transfer_engine = Arc::new(TransferEngine::new(
            Arc::clone(&storage_provider),
            transfer_config,
            None,
        ));

        let backup_engine = Arc::new(BackupEngine::new(
            Arc::clone(&db),
            Arc::clone(&transfer_engine),
            Arc::clone(&temp_manager),
        ));

        let restore_engine = Arc::new(RestoreEngine::new(
            Arc::clone(&db),
            Arc::clone(&transfer_engine),
            Arc::clone(&temp_manager),
        ));

        let checker = Arc::new(BackupChecker::new(Arc::clone(&db)));
        let scheduler_service = Arc::new(televault_scheduler::SchedulerService::new(
            Arc::clone(&db),
            Arc::clone(&backup_engine),
            None,
        ));
        let retention_engine = Arc::new(RetentionEngine::new(Arc::clone(&db)));
        let verification_engine = Arc::new(VerificationEngine::new(
            Arc::clone(&db),
            Arc::clone(&storage_provider),
            Arc::clone(&temp_manager),
        ));
        let repair_engine = Arc::new(RepairEngine::new(
            Arc::clone(&db),
            Arc::clone(&transfer_engine),
            Arc::clone(&temp_manager),
            Arc::clone(&verification_engine),
        ));

        let session_file = paths.config_dir().join("telegram_session.json");
        let is_mock_env = std::env::var("TELEVAULT_MOCK_TELEGRAM")
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false);

        let mtproto_auth = if is_mock_env {
            Arc::new(MtprotoAuthManager::new_mock(session_file))
        } else {
            let (api_id, api_hash) = if let Some(creds) =
                televault_telegram::TelegramApiCredentials::load_from_env_or_file(
                    &paths.config_dir(),
                ) {
                (creds.api_id, creds.api_hash)
            } else {
                (0, String::new())
            };
            Arc::new(MtprotoAuthManager::new_live(session_file, api_id, api_hash))
        };

        Ok(Self {
            db,
            transfer_engine,
            backup_engine,
            restore_engine,
            checker,
            paths,
            temp_manager,
            dynamic_storage,
            storage_provider,
            scheduler_service,
            retention_engine,
            verification_engine,
            repair_engine,
            cancellation_registry: Arc::new(Mutex::new(HashMap::new())),
            telegram_status_cache: Arc::new(Mutex::new(None)),
            startup_recovery_report,
            mtproto_auth,
        })
    }

    /// Creates an ephemeral, in-memory state suitable for testing and deterministic validation.
    pub fn new_in_memory() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("televault_desktop_test_{nanos}"));
        let paths = Arc::new(PathManager::new(temp_dir));
        let _ = std::fs::create_dir_all(paths.temp_dir());

        let db = Arc::new(Database::open_in_memory().expect("open in-memory db"));
        let dynamic_storage = Arc::new(DynamicStorageProvider::new(Arc::new(
            MockStorageProvider::new(),
        )));
        let storage_provider: Arc<dyn StorageProvider + Send + Sync> =
            Arc::clone(&dynamic_storage) as Arc<dyn StorageProvider + Send + Sync>;
        let temp_manager = Arc::new(TempPayloadManager::new(paths.temp_dir()));
        let transfer_config = TransferEngineConfig::default();
        let transfer_engine = Arc::new(TransferEngine::new(
            Arc::clone(&storage_provider),
            transfer_config,
            None,
        ));

        let backup_engine = Arc::new(BackupEngine::new(
            Arc::clone(&db),
            Arc::clone(&transfer_engine),
            Arc::clone(&temp_manager),
        ));

        let restore_engine = Arc::new(RestoreEngine::new(
            Arc::clone(&db),
            Arc::clone(&transfer_engine),
            Arc::clone(&temp_manager),
        ));

        let checker = Arc::new(BackupChecker::new(Arc::clone(&db)));
        let scheduler_service = Arc::new(televault_scheduler::SchedulerService::new(
            Arc::clone(&db),
            Arc::clone(&backup_engine),
            None,
        ));
        let retention_engine = Arc::new(RetentionEngine::new(Arc::clone(&db)));
        let verification_engine = Arc::new(VerificationEngine::new(
            Arc::clone(&db),
            Arc::clone(&storage_provider),
            Arc::clone(&temp_manager),
        ));
        let repair_engine = Arc::new(RepairEngine::new(
            Arc::clone(&db),
            Arc::clone(&transfer_engine),
            Arc::clone(&temp_manager),
            Arc::clone(&verification_engine),
        ));

        let session_file = paths.config_dir().join("telegram_session.json");
        let mtproto_auth = Arc::new(MtprotoAuthManager::new_mock_ready(session_file));

        Self {
            db,
            transfer_engine,
            backup_engine,
            restore_engine,
            checker,
            paths,
            temp_manager,
            dynamic_storage,
            storage_provider,
            scheduler_service,
            retention_engine,
            verification_engine,
            repair_engine,
            cancellation_registry: Arc::new(Mutex::new(HashMap::new())),
            telegram_status_cache: Arc::new(Mutex::new(None)),
            startup_recovery_report: Arc::new(StartupRecoveryReportDto {
                database_integrity_ok: true,
                database_integrity_details: vec!["ok".into()],
                foreign_key_violations: 0,
                foreign_key_violation_details: Vec::new(),
                reconciled_snapshots_count: 0,
                reconciled_transfers_count: 0,
                purged_staging_files_count: 0,
                recovered_at: chrono::Utc::now().to_rfc3339(),
            }),
            mtproto_auth,
        }
    }

    /// Initializes application state with a mock Telegram account and verified channel in Ready state.
    pub fn new_mock_ready(
        base_dir: PathBuf,
        custom_provider: Option<Arc<dyn StorageProvider + Send + Sync>>,
    ) -> Result<Self, AppError> {
        let mut state = Self::new(base_dir, custom_provider)?;
        let session_file = state.paths.config_dir().join("telegram_session.json");
        state.mtproto_auth = Arc::new(MtprotoAuthManager::new_mock_ready(session_file));
        Ok(state)
    }

    /// Path to the secure local Telegram credentials file.
    pub fn telegram_credentials_path(&self) -> PathBuf {
        self.paths.config_dir().join("telegram_credentials.json")
    }

    /// Queries the current safe Telegram configuration and health status.
    pub fn get_telegram_status(&self) -> TelegramStatusDto {
        let creds_path = self.telegram_credentials_path();
        let active_backend = self.dynamic_storage.active_name().to_string();

        if !creds_path.exists() {
            return TelegramStatusDto {
                is_configured: false,
                status: TelegramConnectionStatus::NotConfigured,
                target_chat_id: None,
                bot_username: None,
                last_tested_at: None,
                last_error: None,
                active_backend,
            };
        }

        let creds = match TelegramCredentials::load_from_file(&creds_path) {
            Ok(c) => c,
            Err(e) => {
                return TelegramStatusDto {
                    is_configured: false,
                    status: TelegramConnectionStatus::ConnectionFailed,
                    target_chat_id: None,
                    bot_username: None,
                    last_tested_at: None,
                    last_error: Some(format!("Failed to load credentials: {e}")),
                    active_backend,
                };
            }
        };

        let cached = self.telegram_status_cache.lock().unwrap().clone();
        let (status, bot_username, last_error) = match cached {
            Some(c) => (c.status, c.bot_username, c.last_error),
            None => (TelegramConnectionStatus::Configured, None, None),
        };

        TelegramStatusDto {
            is_configured: true,
            status,
            target_chat_id: Some(creds.target_chat_id),
            bot_username,
            last_tested_at: None,
            last_error,
            active_backend,
        }
    }

    /// Validates, saves credentials, and reconfigures the active storage backend.
    pub fn save_telegram_config(
        &self,
        req: SaveTelegramConfigDto,
    ) -> Result<TelegramStatusDto, AppError> {
        let creds = TelegramCredentials::new(req.bot_token, req.target_chat_id, req.api_endpoint)
            .map_err(|e| AppError::validation("telegram", e.to_string()))?;

        let transport = HttpTelegramTransport::new(creds.clone())
            .map_err(|e| AppError::validation("telegram", e.to_string()))?;

        // Securely persist to disk
        creds
            .save_to_file(&self.telegram_credentials_path())
            .map_err(|e| AppError::Io(std::io::Error::other(e.to_string())))?;

        // Reconfigure active backend seamlessly
        let config = TelegramStorageConfig {
            target_chat_id: creds.target_chat_id,
            chunk_upload_timeout_secs: 300,
            max_retries: 3,
        };
        self.dynamic_storage
            .set_active(Arc::new(TelegramStorageProvider::new(transport, config)));

        *self.telegram_status_cache.lock().unwrap() = Some(TelegramTestCache {
            status: TelegramConnectionStatus::Configured,
            bot_username: None,
            last_error: None,
        });

        Ok(self.get_telegram_status())
    }

    /// Executes an explicit connection test without modifying backup metadata or payloads.
    pub fn test_telegram_connection(&self) -> Result<TelegramConnectionTestResultDto, AppError> {
        let creds_path = self.telegram_credentials_path();
        if !creds_path.exists() {
            return Ok(TelegramConnectionTestResultDto {
                success: false,
                bot_username: None,
                bot_id: None,
                chat_title: None,
                error_message: Some("Telegram credentials are not configured".into()),
            });
        }

        let creds = TelegramCredentials::load_from_file(&creds_path).map_err(|e| {
            AppError::validation("telegram", format!("Failed to load credentials: {e}"))
        })?;

        let transport = HttpTelegramTransport::new(creds.clone()).map_err(|e| {
            AppError::validation("telegram", format!("Failed to initialize transport: {e}"))
        })?;

        match transport.test_connection(creds.target_chat_id) {
            Ok(info) => {
                let config = TelegramStorageConfig {
                    target_chat_id: creds.target_chat_id,
                    chunk_upload_timeout_secs: 300,
                    max_retries: 3,
                };
                self.dynamic_storage
                    .set_active(Arc::new(TelegramStorageProvider::new(transport, config)));

                *self.telegram_status_cache.lock().unwrap() = Some(TelegramTestCache {
                    status: TelegramConnectionStatus::Connected,
                    bot_username: info.bot_username.clone(),
                    last_error: None,
                });

                Ok(TelegramConnectionTestResultDto {
                    success: true,
                    bot_username: info.bot_username,
                    bot_id: Some(info.bot_id),
                    chat_title: info.chat_title,
                    error_message: None,
                })
            }
            Err(e) => {
                let err_msg = e.to_string();
                *self.telegram_status_cache.lock().unwrap() = Some(TelegramTestCache {
                    status: TelegramConnectionStatus::ConnectionFailed,
                    bot_username: None,
                    last_error: Some(err_msg.clone()),
                });

                Ok(TelegramConnectionTestResultDto {
                    success: false,
                    bot_username: None,
                    bot_id: None,
                    chat_title: None,
                    error_message: Some(err_msg),
                })
            }
        }
    }

    /// Disconnects Telegram credentials and falls back to mock storage.
    pub fn disconnect_telegram(&self) -> Result<TelegramStatusDto, AppError> {
        let creds_path = self.telegram_credentials_path();
        TelegramCredentials::remove_file(&creds_path)
            .map_err(|e| AppError::Io(std::io::Error::other(e.to_string())))?;

        self.dynamic_storage
            .set_active(Arc::new(MockStorageProvider::new()));

        *self.telegram_status_cache.lock().unwrap() = None;

        Ok(self.get_telegram_status())
    }

    /// Inspects the in-flight operations registry to identify snapshot IDs currently active.
    pub fn active_snapshots(&self) -> HashSet<televault_core::ids::SnapshotId> {
        let mut set = HashSet::new();
        if let Ok(reg) = self.cancellation_registry.lock() {
            for key in reg.keys() {
                if let Some(sid_str) = key.strip_prefix("snap-") {
                    if let Ok(sid) = televault_core::ids::SnapshotId::new(sid_str) {
                        set.insert(sid);
                    }
                }
            }
        }
        set
    }

    /// Registers a new active cancellation token for an operation ID.
    pub fn register_cancellation(&self, operation_id: &str) -> CancellationToken {
        let token = CancellationToken::new();
        self.cancellation_registry
            .lock()
            .unwrap()
            .insert(operation_id.to_string(), token.clone());
        token
    }

    /// Signals cancellation on an active operation and removes it from the registry.
    pub fn cancel_operation(&self, operation_id: &str) -> bool {
        if let Some(token) = self
            .cancellation_registry
            .lock()
            .unwrap()
            .remove(operation_id)
        {
            token.cancel();
            true
        } else {
            false
        }
    }

    /// Unregisters a completed or failed operation.
    pub fn unregister_cancellation(&self, operation_id: &str) {
        self.cancellation_registry
            .lock()
            .unwrap()
            .remove(operation_id);
    }

    /// Centralized authorization gate enforcing personal Telegram authentication and channel verification.
    pub async fn check_auth_gate(&self) -> Result<(), IpcError> {
        self.mtproto_auth
            .check_auth_gate()
            .await
            .map_err(|e| IpcError::unauthorized(e.to_string()))
    }

    /// Queries safe personal Telegram authentication and dedicated backup channel setup status.
    pub async fn get_telegram_auth_status(&self) -> TelegramAuthStatusDto {
        let status = self.mtproto_auth.status().await;
        if let Some(ref acc) = status.account {
            let now = chrono::Utc::now().to_rfc3339();
            let _ = self
                .db
                .save_telegram_account(&televault_db::TelegramAccountRecord {
                    user_id: acc.user_id,
                    first_name: acc.first_name.clone(),
                    last_name: acc.last_name.clone(),
                    username: acc.username.clone(),
                    phone_redacted: acc.phone_number.clone(),
                    authenticated_at: now.clone(),
                    last_seen_at: now,
                });
        }
        if let (Some(ref acc), Some(ref ch)) = (&status.account, &status.channel) {
            let now = chrono::Utc::now().to_rfc3339();
            let _ = self
                .db
                .save_telegram_channel(&televault_db::TelegramChannelRecord {
                    channel_id: ch.channel_id,
                    user_id: acc.user_id,
                    title: ch.channel_title.clone(),
                    is_private: ch.is_private,
                    verified: ch.verified,
                    created_by_televault: ch.created_by_televault,
                    created_at: now.clone(),
                    verified_at: if ch.verified { Some(now) } else { None },
                });
        }
        let _ = self.db.update_telegram_auth_state(
            &format!("{:?}", status.state).to_lowercase(),
            status.account.as_ref().map(|a| a.user_id),
            status.channel.as_ref().map(|c| c.channel_id),
        );
        status.into()
    }
}

impl From<televault_telegram::AuthState> for AuthStateDto {
    fn from(s: televault_telegram::AuthState) -> Self {
        match s {
            televault_telegram::AuthState::Initializing => AuthStateDto::Initializing,
            televault_telegram::AuthState::AuthenticationRequired => {
                AuthStateDto::AuthenticationRequired
            }
            televault_telegram::AuthState::Authenticating => AuthStateDto::Authenticating,
            televault_telegram::AuthState::AuthenticationFailed => {
                AuthStateDto::AuthenticationFailed
            }
            televault_telegram::AuthState::Authenticated => AuthStateDto::Authenticated,
            televault_telegram::AuthState::ChannelSetupRequired => {
                AuthStateDto::ChannelSetupRequired
            }
            televault_telegram::AuthState::ChannelSetupInProgress => {
                AuthStateDto::ChannelSetupInProgress
            }
            televault_telegram::AuthState::ChannelVerificationFailed => {
                AuthStateDto::ChannelVerificationFailed
            }
            televault_telegram::AuthState::Ready => AuthStateDto::Ready,
            televault_telegram::AuthState::SessionExpired => AuthStateDto::SessionExpired,
            televault_telegram::AuthState::LoggingOut => AuthStateDto::LoggingOut,
            televault_telegram::AuthState::RecoverableError => AuthStateDto::RecoverableError,
        }
    }
}

impl From<televault_telegram::TelegramAccountInfo> for TelegramAccountInfoDto {
    fn from(a: televault_telegram::TelegramAccountInfo) -> Self {
        Self {
            user_id: a.user_id,
            first_name: a.first_name,
            last_name: a.last_name,
            username: a.username,
            phone_number: a.phone_number,
        }
    }
}

impl From<televault_telegram::TelegramChannelInfo> for TelegramChannelInfoDto {
    fn from(c: televault_telegram::TelegramChannelInfo) -> Self {
        Self {
            channel_id: c.channel_id,
            channel_title: c.channel_title,
            is_private: c.is_private,
            verified: c.verified,
            created_by_televault: c.created_by_televault,
        }
    }
}

impl From<televault_telegram::TelegramAuthStatus> for TelegramAuthStatusDto {
    fn from(s: televault_telegram::TelegramAuthStatus) -> Self {
        Self {
            state: s.state.into(),
            account: s.account.map(Into::into),
            channel: s.channel.map(Into::into),
            requires_password: s.requires_password,
            error_message: s.error_message,
        }
    }
}
