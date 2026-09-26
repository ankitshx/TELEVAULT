use chrono::Utc;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Manager, State};
use telecloud_core::{
    AppConfig, Breadcrumb, Folder, LogicalFile, RecentItem, SearchFilter, TransferRecord,
};
use telecloud_db::DatabaseRepository;
use telecloud_storage::{MockStorageProvider, StorageProvider, StorageStatus};
use telecloud_transfer::{DownloadParams, TransferManager, UploadParams};
use uuid::Uuid;

pub struct AppState {
    pub db: Arc<DatabaseRepository>,
    pub storage: Arc<dyn StorageProvider>,
    pub transfer_manager: Arc<TransferManager>,
}

fn initialize_db() -> DatabaseRepository {
    let local_app_data = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let db_dir = PathBuf::from(local_app_data).join("TeleCloud");
    let _ = std::fs::create_dir_all(&db_dir);
    let db_path = db_dir.join("telecloud.db");

    let repo = match DatabaseRepository::open_at_path(&db_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to open db at {:?}: {}", db_path, e);
            DatabaseRepository::open_in_memory().expect("in-memory db must succeed")
        }
    };

    // Seed initial folder tree if fresh db
    if let Ok(folders) = repo.list_folders(None) {
        if folders.is_empty() {
            let _ = repo.insert_folder(&Folder {
                id: Uuid::new_v4(),
                parent_id: None,
                name: "Documents".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            });
            let _ = repo.insert_folder(&Folder {
                id: Uuid::new_v4(),
                parent_id: None,
                name: "Backups".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            });
            let _ = repo.insert_folder(&Folder {
                id: Uuid::new_v4(),
                parent_id: None,
                name: "Media".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            });
        }
    }

    repo
}

#[tauri::command]
fn get_app_status() -> serde_json::Value {
    let config = AppConfig::default();
    serde_json::json!({
        "app_name": config.app_name,
        "chunk_size_mb": config.chunk_size / (1024 * 1024),
        "redundancy": config.redundancy,
        "ready": true
    })
}

#[tauri::command]
async fn list_files(
    state: State<'_, AppState>,
    folder_id: Option<String>,
) -> Result<Vec<LogicalFile>, String> {
    let fid = folder_id.and_then(|s| Uuid::parse_str(&s).ok());
    state.db.list_files(fid).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_folders(
    state: State<'_, AppState>,
    parent_id: Option<String>,
) -> Result<Vec<Folder>, String> {
    let pid = parent_id.and_then(|s| Uuid::parse_str(&s).ok());
    state.db.list_folders(pid).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_breadcrumbs(
    state: State<'_, AppState>,
    folder_id: Option<String>,
) -> Result<Vec<Breadcrumb>, String> {
    let fid = folder_id.and_then(|s| Uuid::parse_str(&s).ok());
    state.db.get_breadcrumbs(fid).map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_folder(
    state: State<'_, AppState>,
    name: String,
    parent_id: Option<String>,
) -> Result<Folder, String> {
    let pid = parent_id.and_then(|s| Uuid::parse_str(&s).ok());
    let folder = Folder {
        id: Uuid::new_v4(),
        parent_id: pid,
        name,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    state.db.insert_folder(&folder).map_err(|e| e.to_string())?;
    Ok(folder)
}

#[tauri::command]
async fn rename_folder(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<Folder, String> {
    let fid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state.db.rename_folder(fid, &name).map_err(|e| e.to_string())?;
    state
        .db
        .get_folder(fid)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Folder not found".to_string())
}

#[tauri::command]
async fn delete_folder(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let fid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state.db.delete_folder(fid).map_err(|e| e.to_string())
}

#[tauri::command]
async fn rename_file(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<LogicalFile, String> {
    let fid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state.db.rename_file(fid, &name).map_err(|e| e.to_string())?;
    state
        .db
        .get_file(fid)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "File not found".to_string())
}

#[tauri::command]
async fn delete_file(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let fid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state.db.delete_file(fid).map_err(|e| e.to_string())
}

#[tauri::command]
async fn copy_file(
    state: State<'_, AppState>,
    id: String,
    target_folder_id: Option<String>,
) -> Result<LogicalFile, String> {
    let fid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let tfid = target_folder_id.and_then(|s| Uuid::parse_str(&s).ok());
    state.db.copy_file(fid, tfid, None).map_err(|e| e.to_string())
}

#[tauri::command]
async fn move_file(
    state: State<'_, AppState>,
    id: String,
    target_folder_id: Option<String>,
) -> Result<(), String> {
    let fid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let tfid = target_folder_id.and_then(|s| Uuid::parse_str(&s).ok());
    state.db.move_file(fid, tfid).map_err(|e| e.to_string())
}

#[tauri::command]
async fn toggle_favorite(state: State<'_, AppState>, file_id: String) -> Result<bool, String> {
    let fid = Uuid::parse_str(&file_id).map_err(|e| e.to_string())?;
    state.db.toggle_favorite(fid).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_favorites(state: State<'_, AppState>) -> Result<Vec<LogicalFile>, String> {
    state.db.list_favorites().map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_recent_items(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<RecentItem>, String> {
    state
        .db
        .list_recent_items(limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn search_files(
    state: State<'_, AppState>,
    filter: SearchFilter,
) -> Result<Vec<LogicalFile>, String> {
    state.db.search_files(&filter).map_err(|e| e.to_string())
}

#[tauri::command]
async fn enqueue_upload(
    state: State<'_, AppState>,
    local_path: String,
    file_name: String,
    folder_id: Option<String>,
    passphrase: Option<String>,
) -> Result<String, String> {
    let fid = folder_id.and_then(|s| Uuid::parse_str(&s).ok());
    let params = UploadParams {
        transfer_id: Uuid::new_v4(),
        file_id: None,
        folder_id: fid,
        local_path: PathBuf::from(local_path),
        file_name,
        passphrase,
        chunk_size: None,
    };
    state
        .transfer_manager
        .enqueue_upload(params)
        .await
        .map(|id| id.to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn enqueue_download(
    state: State<'_, AppState>,
    file_id: String,
    dest_path: String,
    passphrase: Option<String>,
) -> Result<String, String> {
    let fid = Uuid::parse_str(&file_id).map_err(|e| e.to_string())?;
    let params = DownloadParams {
        transfer_id: Uuid::new_v4(),
        file_id: fid,
        dest_path: PathBuf::from(dest_path),
        passphrase,
    };
    state
        .transfer_manager
        .enqueue_download(params)
        .await
        .map(|id| id.to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn pause_transfer(state: State<'_, AppState>, transfer_id: String) -> Result<(), String> {
    let tid = Uuid::parse_str(&transfer_id).map_err(|e| e.to_string())?;
    state.transfer_manager.pause_transfer(tid).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn resume_transfer(state: State<'_, AppState>, transfer_id: String) -> Result<(), String> {
    let tid = Uuid::parse_str(&transfer_id).map_err(|e| e.to_string())?;
    state.transfer_manager.resume_transfer(tid).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cancel_transfer(state: State<'_, AppState>, transfer_id: String) -> Result<(), String> {
    let tid = Uuid::parse_str(&transfer_id).map_err(|e| e.to_string())?;
    state.transfer_manager.cancel_transfer(tid).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_transfers(state: State<'_, AppState>) -> Result<Vec<TransferRecord>, String> {
    state.transfer_manager.list_transfers().map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_active_transfers(state: State<'_, AppState>) -> Result<Vec<TransferRecord>, String> {
    state.transfer_manager.list_active_transfers().map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_storage_status(state: State<'_, AppState>) -> Result<StorageStatus, String> {
    state.storage.check_connection().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn select_file() -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(|| {
        let output = std::process::Command::new("powershell")
            .args(&[
                "-NoProfile",
                "-Command",
                "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.OpenFileDialog; $d.Title = 'Select File for TeleCloud Upload'; if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { Write-Output $d.FileName }",
            ])
            .output()
            .map_err(|e| e.to_string())?;
        let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if selected.is_empty() {
            Ok(None)
        } else {
            Ok(Some(selected))
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

fn ensure_backend_running() {
    std::thread::spawn(|| {
        if std::net::TcpStream::connect("127.0.0.1:8000").is_err() {
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .unwrap_or_else(|| PathBuf::from("."));
            let televault_exe = exe_dir.join("televault.exe");

            #[cfg(target_os = "windows")]
            const CREATE_NO_WINDOW: u32 = 0x08000000;

            if televault_exe.exists() {
                let mut cmd = std::process::Command::new(&televault_exe);
                cmd.args(&["web", "--no-browser"]);
                #[cfg(target_os = "windows")]
                cmd.creation_flags(CREATE_NO_WINDOW);
                let _ = cmd.spawn();
            } else {
                let mut cmd = std::process::Command::new("python");
                cmd.args(&["-m", "televault.presentation.cli", "web", "--no-browser"]);
                #[cfg(target_os = "windows")]
                cmd.creation_flags(CREATE_NO_WINDOW);
                let _ = cmd.spawn();
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let local_app_data = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let telecloud_dir = PathBuf::from(&local_app_data).join("TeleCloud");
    let _ = std::fs::create_dir_all(&telecloud_dir);

    // Global panic hook to capture crash diagnostics
    let panic_log_path = telecloud_dir.join("panic.log");
    std::panic::set_hook(Box::new(move |info| {
        let timestamp = chrono::Utc::now().to_rfc3339();
        let msg = format!("[{}] PANIC: {}\n", timestamp, info);
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&panic_log_path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, msg.as_bytes()));
        eprintln!("{}", msg);
    }));

    let storage_dir = telecloud_dir.join("mock_storage");
    let _ = std::fs::create_dir_all(&storage_dir);

    let db = Arc::new(initialize_db());
    let storage: Arc<dyn StorageProvider> = Arc::new(MockStorageProvider::with_storage_dir(
        -1001234567890,
        storage_dir,
    ));
    let transfer_manager = Arc::new(TransferManager::new(storage.clone(), db.clone()));

    let app_state = AppState {
        db,
        storage,
        transfer_manager,
    };

    tauri::Builder::default()
        .manage(app_state)
        .setup(|app| {
            ensure_backend_running();
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_status,
            list_files,
            list_folders,
            get_breadcrumbs,
            create_folder,
            rename_folder,
            delete_folder,
            rename_file,
            delete_file,
            copy_file,
            move_file,
            toggle_favorite,
            list_favorites,
            list_recent_items,
            search_files,
            enqueue_upload,
            enqueue_download,
            pause_transfer,
            resume_transfer,
            cancel_transfer,
            get_transfers,
            get_active_transfers,
            get_storage_status,
            select_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running telecloud application");
}

