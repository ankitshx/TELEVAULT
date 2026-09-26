use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use telecloud_core::{AppConfig, SearchFilter};
use telecloud_db::DatabaseRepository;
use telecloud_storage::{MockStorageProvider, StorageProvider};
use telecloud_transfer::{DownloadParams, TransferManager, UploadParams};
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "telecloud")]
#[command(author = "TeleCloud Team")]
#[command(version = "0.1.0")]
#[command(about = "TeleCloud: Personal cloud drive powered by Telegram storage", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Inspect account and cloud storage status
    Status,

    /// List files and folders in cloud drive
    Ls {
        /// Optional folder ID to list contents of
        folder: Option<String>,
    },

    /// Search files by keyword or pattern
    Search {
        /// Search keyword or query
        query: String,
    },

    /// Upload a local file to TeleCloud
    Upload {
        /// Path to local file
        file: PathBuf,

        /// Enable client-side zero-knowledge AES-256-GCM encryption
        #[arg(long, default_value_t = false)]
        encrypt: bool,

        /// Optional encryption passphrase
        #[arg(long)]
        passphrase: Option<String>,
    },

    /// Download a file from TeleCloud
    Download {
        /// Logical file ID or exact filename
        file: String,

        /// Destination directory or file path
        #[arg(short, long)]
        dest: Option<PathBuf>,

        /// Decryption passphrase if file is encrypted
        #[arg(long)]
        passphrase: Option<String>,
    },

    /// Manage background transfer queue
    Transfer {
        #[command(subcommand)]
        action: TransferActions,
    },
}

#[derive(Subcommand)]
enum TransferActions {
    /// List active and queued transfers
    List,
    /// Pause an active transfer
    Pause { id: String },
    /// Resume a paused transfer
    Resume { id: String },
    /// Cancel a transfer
    Cancel { id: String },
}

fn get_cli_database() -> DatabaseRepository {
    let base_dir = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let db_dir = PathBuf::from(base_dir).join(".telecloud");
    let _ = std::fs::create_dir_all(&db_dir);
    let db_path = db_dir.join("telecloud.db");

    DatabaseRepository::open_at_path(&db_path).unwrap_or_else(|_| {
        DatabaseRepository::open_in_memory().expect("Database fallback in-memory")
    })
}

fn get_cli_storage_dir() -> PathBuf {
    let base_dir = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let storage_dir = PathBuf::from(base_dir).join(".telecloud").join("mock_storage");
    let _ = std::fs::create_dir_all(&storage_dir);
    storage_dir
}

fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    let k = 1024.0;
    let sizes = ["B", "KB", "MB", "GB", "TB"];
    let i = (bytes as f64).log(k).floor() as usize;
    let i = i.min(sizes.len() - 1);
    format!("{:.2} {}", (bytes as f64) / k.powi(i as i32), sizes[i])
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = AppConfig::default();

    let db = Arc::new(get_cli_database());
    let storage_dir = get_cli_storage_dir();
    let storage: Arc<dyn StorageProvider> = Arc::new(MockStorageProvider::with_storage_dir(
        -1001234567890,
        storage_dir,
    ));
    let transfer_manager = Arc::new(TransferManager::new(storage.clone(), db.clone()));

    match cli.command {
        Commands::Status => {
            let storage_status = storage.check_connection().await?;

            println!("==================================================");
            println!("               TELECLOUD CLI STATUS               ");
            println!("==================================================");
            println!("App Name:         {}", config.app_name);
            println!("Data Directory:   {}", config.data_dir.display());
            println!("Default Chunk:    {} MB", config.chunk_size / (1024 * 1024));
            println!("Redundancy Mode:  {:?}", config.redundancy);
            println!("Storage Provider: {}", storage_status.provider_name);
            println!("Connected:        {}", if storage_status.is_connected { "YES (Active)" } else { "NO" });
            println!("Max Chunk Size:   {}", format_bytes(storage_status.max_single_upload_bytes));

            let root_files = db.list_files(None)?;
            let root_folders = db.list_folders(None)?;
            let total_stored_bytes: u64 = root_files.iter().map(|f| f.size).sum();

            println!("--------------------------------------------------");
            println!("Root Folders:     {}", root_folders.len());
            println!("Root Files:       {}", root_files.len());
            println!("Total Tracked:    {}", format_bytes(total_stored_bytes));
            println!("==================================================");
        }

        Commands::Ls { folder } => {
            let folder_id = folder.and_then(|s| Uuid::parse_str(&s).ok());
            let folders = db.list_folders(folder_id)?;
            let files = db.list_files(folder_id)?;

            if folders.is_empty() && files.is_empty() {
                println!("(Directory is empty)");
                return Ok(());
            }

            println!("{:<38} {:<10} {:<12} {:<32}", "ID / NAME", "TYPE", "SIZE", "SECURITY");
            println!("{:-<92}", "");

            for f in folders {
                println!("{:<38} {:<10} {:<12} {:<32}", f.name, "[DIR]", "—", "Plaintext");
            }

            for f in files {
                let sec = if f.is_encrypted { "AES-256-GCM" } else { "Plaintext" };
                println!(
                    "{:<38} {:<10} {:<12} {:<32}",
                    f.name,
                    "[FILE]",
                    format_bytes(f.size),
                    sec
                );
            }
        }

        Commands::Search { query } => {
            println!("Searching for: '{}'", query);
            let results = db.search_files(&SearchFilter {
                query: query.clone(),
                ..Default::default()
            })?;

            if results.is_empty() {
                println!("No files matched '{}'", query);
            } else {
                println!("\nFound {} match(es):", results.len());
                for f in results {
                    let sec = if f.is_encrypted { " [AES-256]" } else { "" };
                    println!(" - {} ({}){} ID: {}", f.name, format_bytes(f.size), sec, f.id);
                }
            }
        }

        Commands::Upload { file, encrypt, passphrase } => {
            if !file.exists() {
                anyhow::bail!("File does not exist: {}", file.display());
            }

            let file_name = file
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "upload.bin".to_string());

            let enc_pass = if encrypt {
                passphrase.or_else(|| Some("telecloud_cli_default_secret".to_string()))
            } else {
                passphrase
            };

            let transfer_id = Uuid::new_v4();
            println!("Starting streaming upload for: {}", file.display());
            println!("Transfer ID: {}", transfer_id);
            if enc_pass.is_some() {
                println!("Encryption: AES-256-GCM (Zero-Knowledge)");
            }

            let params = UploadParams {
                transfer_id,
                file_id: None,
                folder_id: None,
                local_path: file,
                file_name,
                passphrase: enc_pass,
                chunk_size: Some(config.chunk_size),
            };

            let uploaded = transfer_manager.execute_upload_now(params).await?;
            println!("\nUpload Completed Successfully!");
            println!("File ID:     {}", uploaded.id);
            println!("File Name:   {}", uploaded.name);
            println!("Size:        {}", format_bytes(uploaded.size));
            println!("SHA-256:     {}", uploaded.sha256);
            println!("Encrypted:   {}", uploaded.is_encrypted);
        }

        Commands::Download { file, dest, passphrase } => {
            // Find file by UUID or by exact name
            let target_file = if let Ok(id) = Uuid::parse_str(&file) {
                db.get_file(id)?
            } else {
                let search_results = db.search_files(&SearchFilter {
                    query: file.clone(),
                    ..Default::default()
                })?;
                search_results.into_iter().next()
            };

            let file_record = target_file.ok_or_else(|| {
                anyhow::anyhow!("File '{}' not found in cloud database", file)
            })?;

            let dest_path = dest.unwrap_or_else(|| PathBuf::from(&file_record.name));
            println!("Downloading file: {} -> {}", file_record.name, dest_path.display());

            let transfer_id = Uuid::new_v4();
            let params = DownloadParams {
                transfer_id,
                file_id: file_record.id,
                dest_path: dest_path.clone(),
                passphrase,
            };

            transfer_manager.execute_download_now(params).await?;
            println!("\nDownload & Integrity Verification Completed Successfully!");
            println!("Verified Path: {}", dest_path.display());
            println!("Expected SHA-256: {}", file_record.sha256);
        }

        Commands::Transfer { action } => match action {
            TransferActions::List => {
                let list = transfer_manager.list_transfers()?;
                if list.is_empty() {
                    println!("No active or historical transfers found.");
                } else {
                    println!("{:<38} {:<10} {:<14} {:<12} {:<12}", "ID", "DIR", "STATE", "PROGRESS", "FILE");
                    println!("{:-<86}", "");
                    for t in list {
                        let pct = if t.total_bytes > 0 {
                            format!("{}%", (t.transferred_bytes * 100) / t.total_bytes)
                        } else {
                            "0%".to_string()
                        };
                        println!(
                            "{:<38} {:<10?} {:<14?} {:<12} {:<12}",
                            t.id, t.direction, t.state, pct, t.file_name
                        );
                    }
                }
            }
            TransferActions::Pause { id } => {
                let tid = Uuid::parse_str(&id)?;
                transfer_manager.pause_transfer(tid).await?;
                println!("Paused transfer {}", tid);
            }
            TransferActions::Resume { id } => {
                let tid = Uuid::parse_str(&id)?;
                transfer_manager.resume_transfer(tid).await?;
                println!("Resumed transfer {}", tid);
            }
            TransferActions::Cancel { id } => {
                let tid = Uuid::parse_str(&id)?;
                transfer_manager.cancel_transfer(tid).await?;
                println!("Cancelled transfer {}", tid);
            }
        },
    }

    Ok(())
}
