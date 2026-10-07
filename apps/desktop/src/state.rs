//! Shared Desktop application state managed across Tauri IPC commands.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use televault_backup::checker::BackupChecker;
use televault_backup::engine::BackupEngine;
use televault_backup::restore::RestoreEngine;
use televault_core::error::AppError;
use televault_core::paths::PathManager;
use televault_db::Database;
use televault_storage::mock::MockStorageProvider;
use televault_storage::temp::TempPayloadManager;
use televault_storage::StorageProvider;
use televault_transfer::cancellation::CancellationToken;
use televault_transfer::engine::{TransferEngine, TransferEngineConfig};

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
    /// Remote storage abstraction provider.
    pub storage_provider: Arc<dyn StorageProvider + Send + Sync>,
    /// Production background scheduler service coordinating automated backups.
    pub scheduler_service: Arc<televault_scheduler::SchedulerService>,
    /// Registry mapping in-flight operation IDs to cooperative cancellation tokens.
    pub cancellation_registry: Arc<Mutex<HashMap<String, CancellationToken>>>,
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

        let storage_provider: Arc<dyn StorageProvider + Send + Sync> = match custom_provider {
            Some(provider) => provider,
            None => Arc::new(MockStorageProvider::new()),
        };

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

        Ok(Self {
            db,
            transfer_engine,
            backup_engine,
            restore_engine,
            checker,
            paths,
            temp_manager,
            storage_provider,
            scheduler_service,
            cancellation_registry: Arc::new(Mutex::new(HashMap::new())),
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
        let storage_provider: Arc<dyn StorageProvider + Send + Sync> =
            Arc::new(MockStorageProvider::new());
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

        Self {
            db,
            transfer_engine,
            backup_engine,
            restore_engine,
            checker,
            paths,
            temp_manager,
            storage_provider,
            scheduler_service,
            cancellation_registry: Arc::new(Mutex::new(HashMap::new())),
        }
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
}
