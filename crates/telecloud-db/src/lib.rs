pub mod migrations;
pub mod repository;

pub use migrations::run_migrations;
pub use repository::DatabaseRepository;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use telecloud_core::{Folder, LogicalFile, TransferDirection, TransferRecord, TransferState};
    use uuid::Uuid;

    #[test]
    fn test_db_folder_and_file_crud() {
        let repo = DatabaseRepository::open_in_memory().unwrap();

        let folder_id = Uuid::new_v4();
        let folder = Folder {
            id: folder_id,
            parent_id: None,
            name: "Documents".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        repo.insert_folder(&folder).unwrap();

        let root_folders = repo.list_folders(None).unwrap();
        assert_eq!(root_folders.len(), 1);
        assert_eq!(root_folders[0].name, "Documents");

        let file = LogicalFile {
            id: Uuid::new_v4(),
            folder_id: Some(folder_id),
            name: "report.pdf".to_string(),
            size: 1024,
            sha256: "deadbeef".to_string(),
            mime_type: Some("application/pdf".to_string()),
            is_favorite: true,
            is_encrypted: false,
            manifest_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        repo.insert_file(&file).unwrap();

        let files = repo.list_files(Some(folder_id)).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "report.pdf");
        assert!(files[0].is_favorite);
    }

    #[test]
    fn test_db_transfer_persistence() {
        let repo = DatabaseRepository::open_in_memory().unwrap();
        let transfer_id = Uuid::new_v4();

        let transfer = TransferRecord {
            id: transfer_id,
            file_id: None,
            direction: TransferDirection::Upload,
            local_path: "C:\\test\\large.iso".to_string(),
            file_name: "large.iso".to_string(),
            total_bytes: 5_000_000_000,
            transferred_bytes: 1_000_000_000,
            state: TransferState::Uploading,
            speed_bytes_per_sec: 25_000_000,
            eta_seconds: Some(160),
            active_chunk_index: Some(1),
            total_chunks: Some(5),
            error_message: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        repo.upsert_transfer(&transfer).unwrap();

        let active = repo.list_active_transfers().unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, transfer_id);
        assert_eq!(active[0].transferred_bytes, 1_000_000_000);
        assert_eq!(active[0].state, TransferState::Uploading);

        let single = repo.get_transfer(transfer_id).unwrap();
        assert!(single.is_some());
        assert_eq!(single.unwrap().file_name, "large.iso");

        let all = repo.list_transfers().unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn test_db_fts5_search_and_filter() {
        let repo = DatabaseRepository::open_in_memory().unwrap();

        let f1 = LogicalFile {
            id: Uuid::new_v4(),
            folder_id: None,
            name: "Quarterly_Financial_Report_2026.xlsx".to_string(),
            size: 204800,
            sha256: "hash1".to_string(),
            mime_type: None,
            is_favorite: true,
            is_encrypted: false,
            manifest_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let f2 = LogicalFile {
            id: Uuid::new_v4(),
            folder_id: None,
            name: "Vacation_Photo.jpg".to_string(),
            size: 512000,
            sha256: "hash2".to_string(),
            mime_type: None,
            is_favorite: false,
            is_encrypted: true,
            manifest_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        repo.insert_file(&f1).unwrap();
        repo.insert_file(&f2).unwrap();

        // Search by word
        let filter = telecloud_core::SearchFilter {
            query: "Financial".to_string(),
            ..Default::default()
        };
        let results = repo.search_files(&filter).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, f1.id);

        // Search by favorite
        let favs = repo.list_favorites().unwrap();
        assert_eq!(favs.len(), 1);
        assert_eq!(favs[0].name, "Quarterly_Financial_Report_2026.xlsx");

        // Toggle favorite
        let is_fav = repo.toggle_favorite(f1.id).unwrap();
        assert!(!is_fav);
        assert_eq!(repo.list_favorites().unwrap().len(), 0);
    }

    #[test]
    fn test_db_breadcrumbs() {
        let repo = DatabaseRepository::open_in_memory().unwrap();

        let root_dir = Folder {
            id: Uuid::new_v4(),
            parent_id: None,
            name: "Projects".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let sub_dir = Folder {
            id: Uuid::new_v4(),
            parent_id: Some(root_dir.id),
            name: "Rust".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        repo.insert_folder(&root_dir).unwrap();
        repo.insert_folder(&sub_dir).unwrap();

        let crumbs = repo.get_breadcrumbs(Some(sub_dir.id)).unwrap();
        assert_eq!(crumbs.len(), 3);
        assert_eq!(crumbs[0].name, "My Drive");
        assert_eq!(crumbs[1].name, "Projects");
        assert_eq!(crumbs[2].name, "Rust");
    }

    #[test]
    fn test_db_chunks_and_manifests() {
        let repo = DatabaseRepository::open_in_memory().unwrap();
        let file_id = Uuid::new_v4();

        let file = LogicalFile {
            id: file_id,
            folder_id: None,
            name: "test_archive.tar".to_string(),
            size: 1048576,
            sha256: "fullhash0".to_string(),
            mime_type: None,
            is_favorite: false,
            is_encrypted: false,
            manifest_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        repo.insert_file(&file).unwrap();

        let chunk = telecloud_core::FileChunk {
            id: Uuid::new_v4(),
            file_id,
            chunk_index: 0,
            byte_offset: 0,
            size: 1048576,
            sha256: "chunkhash0".to_string(),
            storage_provider: "telegram".to_string(),
            remote_chat_id: -1001234567,
            remote_message_id: 42,
            is_primary: true,
            created_at: Utc::now(),
        };

        repo.save_file_chunks(&[chunk]).unwrap();

        let loaded_chunks = repo.get_file_chunks(file_id).unwrap();
        assert_eq!(loaded_chunks.len(), 1);
        assert_eq!(loaded_chunks[0].chunk_index, 0);
        assert_eq!(loaded_chunks[0].remote_message_id, 42);

        repo.save_manifest(file_id, 1, r#"{"version":1}"#).unwrap();
        let manifest = repo.get_manifest(file_id).unwrap();
        assert!(manifest.is_some());
        assert_eq!(manifest.unwrap(), r#"{"version":1}"#);
    }
}
