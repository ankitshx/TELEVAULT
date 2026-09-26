use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};
use telecloud_core::{
    Breadcrumb, FileChunk, Folder, IntegrityRecord, LogicalFile, RecentAction, RecentItem, Result,
    SearchFilter, TeleCloudError, TransferDirection, TransferRecord, TransferState,
};
use uuid::Uuid;

pub struct DatabaseRepository {
    conn: Arc<Mutex<Connection>>,
}

impl DatabaseRepository {
    pub fn new(mut conn: Connection) -> Result<Self> {
        crate::migrations::run_migrations(&mut conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| TeleCloudError::Database {
            operation: "open_in_memory".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;
        Self::new(conn)
    }

    pub fn open_at_path(path: &std::path::Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| TeleCloudError::Database {
            operation: "open_at_path".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;
        Self::new(conn)
    }

    // -------------------------------------------------------------
    // FOLDER OPERATIONS
    // -------------------------------------------------------------

    pub fn insert_folder(&self, folder: &Folder) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO folders (id, parent_id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                folder.id.to_string(),
                folder.parent_id.map(|id| id.to_string()),
                folder.name,
                folder.created_at.to_rfc3339(),
                folder.updated_at.to_rfc3339(),
            ],
        ).map_err(|e| TeleCloudError::Database {
            operation: "insert_folder".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;
        Ok(())
    }

    pub fn get_folder(&self, id: Uuid) -> Result<Option<Folder>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, parent_id, name, created_at, updated_at FROM folders WHERE id = ?1")
            .map_err(|e| TeleCloudError::Database {
                operation: "prepare_get_folder".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;

        let mut rows = stmt
            .query_map(params![id.to_string()], Self::map_folder)
            .map_err(|e| TeleCloudError::Database {
                operation: "query_get_folder".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;

        if let Some(row) = rows.next() {
            row.map(Some).map_err(|e| TeleCloudError::Database {
                operation: "map_get_folder".to_string(),
                message: e.to_string(),
                recoverable: false,
            })
        } else {
            Ok(None)
        }
    }

    pub fn list_folders(&self, parent_id: Option<Uuid>) -> Result<Vec<Folder>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = if parent_id.is_some() {
            conn.prepare(
                "SELECT id, parent_id, name, created_at, updated_at FROM folders WHERE parent_id = ?1 ORDER BY name ASC",
            )
        } else {
            conn.prepare(
                "SELECT id, parent_id, name, created_at, updated_at FROM folders WHERE parent_id IS NULL ORDER BY name ASC",
            )
        }.map_err(|e| TeleCloudError::Database {
            operation: "prepare_list_folders".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        let rows = if let Some(pid) = parent_id {
            stmt.query_map(params![pid.to_string()], Self::map_folder)
        } else {
            stmt.query_map([], Self::map_folder)
        }
        .map_err(|e| TeleCloudError::Database {
            operation: "query_folders".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        let mut folders = Vec::new();
        for r in rows {
            if let Ok(f) = r {
                folders.push(f);
            }
        }
        Ok(folders)
    }

    pub fn rename_folder(&self, id: Uuid, new_name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE folders SET name = ?1, updated_at = ?2 WHERE id = ?3",
            params![new_name, Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(|e| TeleCloudError::Database {
            operation: "rename_folder".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;
        Ok(())
    }

    pub fn delete_folder(&self, id: Uuid) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM folders WHERE id = ?1", params![id.to_string()])
            .map_err(|e| TeleCloudError::Database {
                operation: "delete_folder".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;
        Ok(())
    }

    pub fn get_breadcrumbs(&self, folder_id: Option<Uuid>) -> Result<Vec<Breadcrumb>> {
        let mut crumbs = Vec::new();
        let mut current_id = folder_id;

        while let Some(fid) = current_id {
            if let Some(folder) = self.get_folder(fid)? {
                crumbs.push(Breadcrumb {
                    id: Some(folder.id),
                    name: folder.name,
                });
                current_id = folder.parent_id;
            } else {
                break;
            }
        }

        crumbs.reverse();
        crumbs.insert(
            0,
            Breadcrumb {
                id: None,
                name: "My Drive".to_string(),
            },
        );

        Ok(crumbs)
    }

    // -------------------------------------------------------------
    // FILE OPERATIONS
    // -------------------------------------------------------------

    pub fn insert_file(&self, file: &LogicalFile) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO files (id, folder_id, name, size, sha256, mime_type, is_favorite, is_encrypted, manifest_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET
                folder_id = excluded.folder_id,
                name = excluded.name,
                size = excluded.size,
                sha256 = excluded.sha256,
                mime_type = excluded.mime_type,
                is_favorite = excluded.is_favorite,
                is_encrypted = excluded.is_encrypted,
                manifest_id = excluded.manifest_id,
                updated_at = excluded.updated_at",
            params![
                file.id.to_string(),
                file.folder_id.map(|id| id.to_string()),
                file.name,
                file.size as i64,
                file.sha256,
                file.mime_type,
                if file.is_favorite { 1 } else { 0 },
                if file.is_encrypted { 1 } else { 0 },
                file.manifest_id.map(|id| id.to_string()),
                file.created_at.to_rfc3339(),
                file.updated_at.to_rfc3339(),
            ],
        ).map_err(|e| TeleCloudError::Database {
            operation: "insert_file".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        if file.is_favorite {
            conn.execute(
                "INSERT OR IGNORE INTO favorites (file_id, created_at) VALUES (?1, ?2)",
                params![file.id.to_string(), file.created_at.to_rfc3339()],
            ).map_err(|e| TeleCloudError::Database {
                operation: "insert_favorite_on_insert_file".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;
        }
        Ok(())
    }

    pub fn get_file(&self, id: Uuid) -> Result<Option<LogicalFile>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, folder_id, name, size, sha256, mime_type, is_favorite, is_encrypted, manifest_id, created_at, updated_at
             FROM files WHERE id = ?1",
        ).map_err(|e| TeleCloudError::Database {
            operation: "prepare_get_file".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        let mut rows = stmt
            .query_map(params![id.to_string()], Self::map_file)
            .map_err(|e| TeleCloudError::Database {
                operation: "query_get_file".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;

        if let Some(row) = rows.next() {
            row.map(Some).map_err(|e| TeleCloudError::Database {
                operation: "map_get_file".to_string(),
                message: e.to_string(),
                recoverable: false,
            })
        } else {
            Ok(None)
        }
    }

    pub fn list_files(&self, folder_id: Option<Uuid>) -> Result<Vec<LogicalFile>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = if folder_id.is_some() {
            conn.prepare(
                "SELECT id, folder_id, name, size, sha256, mime_type, is_favorite, is_encrypted, manifest_id, created_at, updated_at
                 FROM files WHERE folder_id = ?1 ORDER BY name ASC",
            )
        } else {
            conn.prepare(
                "SELECT id, folder_id, name, size, sha256, mime_type, is_favorite, is_encrypted, manifest_id, created_at, updated_at
                 FROM files WHERE folder_id IS NULL ORDER BY name ASC",
            )
        }.map_err(|e| TeleCloudError::Database {
            operation: "prepare_list_files".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        let rows = if let Some(fid) = folder_id {
            stmt.query_map(params![fid.to_string()], Self::map_file)
        } else {
            stmt.query_map([], Self::map_file)
        }
        .map_err(|e| TeleCloudError::Database {
            operation: "query_files".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        let mut files = Vec::new();
        for r in rows {
            if let Ok(f) = r {
                files.push(f);
            }
        }
        Ok(files)
    }

    pub fn rename_file(&self, id: Uuid, new_name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE files SET name = ?1, updated_at = ?2 WHERE id = ?3",
            params![new_name, Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(|e| TeleCloudError::Database {
            operation: "rename_file".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;
        Ok(())
    }

    pub fn move_file(&self, id: Uuid, target_folder_id: Option<Uuid>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE files SET folder_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![
                target_folder_id.map(|f| f.to_string()),
                Utc::now().to_rfc3339(),
                id.to_string()
            ],
        )
        .map_err(|e| TeleCloudError::Database {
            operation: "move_file".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;
        Ok(())
    }

    pub fn copy_file(&self, id: Uuid, target_folder_id: Option<Uuid>, new_name: Option<String>) -> Result<LogicalFile> {
        let orig = self.get_file(id)?.ok_or_else(|| TeleCloudError::Database {
            operation: "copy_file".to_string(),
            message: format!("Original file {} not found", id),
            recoverable: false,
        })?;

        let copy_id = Uuid::new_v4();
        let file_copy = LogicalFile {
            id: copy_id,
            folder_id: target_folder_id,
            name: new_name.unwrap_or_else(|| format!("Copy of {}", orig.name)),
            size: orig.size,
            sha256: orig.sha256.clone(),
            mime_type: orig.mime_type.clone(),
            is_favorite: false,
            is_encrypted: orig.is_encrypted,
            manifest_id: orig.manifest_id,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.insert_file(&file_copy)?;

        // Also copy physical chunks pointers
        let chunks = self.get_file_chunks(id)?;
        if !chunks.is_empty() {
            let copied_chunks: Vec<FileChunk> = chunks
                .into_iter()
                .map(|mut c| {
                    c.id = Uuid::new_v4();
                    c.file_id = copy_id;
                    c
                })
                .collect();
            self.save_file_chunks(&copied_chunks)?;
        }

        Ok(file_copy)
    }

    pub fn delete_file(&self, id: Uuid) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM files WHERE id = ?1", params![id.to_string()])
            .map_err(|e| TeleCloudError::Database {
                operation: "delete_file".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;
        Ok(())
    }

    // -------------------------------------------------------------
    // FTS5 SEARCH
    // -------------------------------------------------------------

    pub fn search_files(&self, filter: &SearchFilter) -> Result<Vec<LogicalFile>> {
        let conn = self.conn.lock().unwrap();

        // If query is present, use FTS5 virtual table combined with LIKE fallback
        if !filter.query.trim().is_empty() {
            let clean_alphanum = filter
                .query
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { ' ' })
                .collect::<String>();
            let words: Vec<&str> = clean_alphanum.split_whitespace().collect();
            let fts_query = if words.is_empty() {
                "\"\"".to_string()
            } else {
                words.iter().map(|w| format!("{}*", w)).collect::<Vec<_>>().join(" ")
            };
            let like_query = format!("%{}%", filter.query.trim());
            let mut sql = String::from(
                "SELECT f.id, f.folder_id, f.name, f.size, f.sha256, f.mime_type, f.is_favorite, f.is_encrypted, f.manifest_id, f.created_at, f.updated_at
                 FROM files f
                 WHERE (f.id IN (SELECT id FROM files_fts WHERE files_fts MATCH ?1) OR f.name LIKE ?2)",
            );

            if filter.folder_id.is_some() {
                sql.push_str(" AND f.folder_id = ?3");
            }
            if filter.is_favorite.unwrap_or(false) {
                sql.push_str(" AND f.is_favorite = 1");
            }
            if let Some(min) = filter.min_size {
                sql.push_str(&format!(" AND f.size >= {}", min));
            }
            if let Some(max) = filter.max_size {
                sql.push_str(&format!(" AND f.size <= {}", max));
            }

            sql.push_str(" ORDER BY f.name ASC");
            if let Some(limit) = filter.limit {
                sql.push_str(&format!(" LIMIT {}", limit));
            }

            let mut stmt = conn.prepare(&sql).map_err(|e| TeleCloudError::Database {
                operation: "prepare_fts_search".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;

            let rows = if let Some(fid) = filter.folder_id {
                stmt.query_map(params![fts_query, like_query, fid.to_string()], Self::map_file)
            } else {
                stmt.query_map(params![fts_query, like_query], Self::map_file)
            }
            .map_err(|e| TeleCloudError::Database {
                operation: "query_fts_search".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;

            let mut results = Vec::new();
            for r in rows {
                let f = r.map_err(|e| TeleCloudError::Database {
                    operation: "map_fts_result".to_string(),
                    message: e.to_string(),
                    recoverable: false,
                })?;
                if let Some(ref ext) = filter.extension {
                    if !f.name.to_lowercase().ends_with(&ext.to_lowercase()) {
                        continue;
                    }
                }
                results.push(f);
            }
            Ok(results)
        } else {
            // No full-text query, perform filtered table scan
            let mut sql = String::from(
                "SELECT id, folder_id, name, size, sha256, mime_type, is_favorite, is_encrypted, manifest_id, created_at, updated_at
                 FROM files WHERE 1=1",
            );

            if filter.folder_id.is_some() {
                sql.push_str(" AND folder_id = ?1");
            }
            if filter.is_favorite.unwrap_or(false) {
                sql.push_str(" AND is_favorite = 1");
            }
            if let Some(min) = filter.min_size {
                sql.push_str(&format!(" AND size >= {}", min));
            }
            if let Some(max) = filter.max_size {
                sql.push_str(&format!(" AND size <= {}", max));
            }

            sql.push_str(" ORDER BY name ASC");
            if let Some(limit) = filter.limit {
                sql.push_str(&format!(" LIMIT {}", limit));
            }

            let mut stmt = conn.prepare(&sql).map_err(|e| TeleCloudError::Database {
                operation: "prepare_scan_search".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;

            let rows = if let Some(fid) = filter.folder_id {
                stmt.query_map(params![fid.to_string()], Self::map_file)
            } else {
                stmt.query_map([], Self::map_file)
            }
            .map_err(|e| TeleCloudError::Database {
                operation: "query_scan_search".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;

            let mut results = Vec::new();
            for r in rows {
                if let Ok(f) = r {
                    if let Some(ref ext) = filter.extension {
                        if !f.name.to_lowercase().ends_with(&ext.to_lowercase()) {
                            continue;
                        }
                    }
                    results.push(f);
                }
            }
            Ok(results)
        }
    }

    // -------------------------------------------------------------
    // FAVORITES & RECENTS
    // -------------------------------------------------------------

    pub fn toggle_favorite(&self, file_id: Uuid) -> Result<bool> {
        let file = self.get_file(file_id)?.ok_or_else(|| TeleCloudError::Database {
            operation: "toggle_favorite".to_string(),
            message: format!("File {} not found", file_id),
            recoverable: false,
        })?;

        let new_fav = !file.is_favorite;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE files SET is_favorite = ?1, updated_at = ?2 WHERE id = ?3",
            params![if new_fav { 1 } else { 0 }, Utc::now().to_rfc3339(), file_id.to_string()],
        ).map_err(|e| TeleCloudError::Database {
            operation: "update_file_favorite".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        if new_fav {
            conn.execute(
                "INSERT OR IGNORE INTO favorites (file_id, created_at) VALUES (?1, ?2)",
                params![file_id.to_string(), Utc::now().to_rfc3339()],
            ).map_err(|e| TeleCloudError::Database {
                operation: "insert_favorite".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;
        } else {
            conn.execute(
                "DELETE FROM favorites WHERE file_id = ?1",
                params![file_id.to_string()],
            ).map_err(|e| TeleCloudError::Database {
                operation: "delete_favorite".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;
        }

        Ok(new_fav)
    }

    pub fn list_favorites(&self) -> Result<Vec<LogicalFile>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT f.id, f.folder_id, f.name, f.size, f.sha256, f.mime_type, f.is_favorite, f.is_encrypted, f.manifest_id, f.created_at, f.updated_at
             FROM files f
             JOIN favorites fav ON f.id = fav.file_id
             ORDER BY fav.created_at DESC",
        ).map_err(|e| TeleCloudError::Database {
            operation: "prepare_list_favorites".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        let rows = stmt.query_map([], Self::map_file).map_err(|e| TeleCloudError::Database {
            operation: "query_list_favorites".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        let mut files = Vec::new();
        for r in rows {
            if let Ok(f) = r {
                files.push(f);
            }
        }
        Ok(files)
    }

    pub fn record_recent_access(&self, file_id: Uuid, action: RecentAction) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let action_str = match action {
            RecentAction::Uploaded => "uploaded",
            RecentAction::Downloaded => "downloaded",
            RecentAction::Opened => "opened",
            RecentAction::Modified => "modified",
        };

        conn.execute(
            "INSERT INTO recent_items (id, file_id, action, accessed_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                Uuid::new_v4().to_string(),
                file_id.to_string(),
                action_str,
                Utc::now().to_rfc3339()
            ],
        ).map_err(|e| TeleCloudError::Database {
            operation: "record_recent_access".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;
        Ok(())
    }

    pub fn list_recent_items(&self, limit: usize) -> Result<Vec<RecentItem>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT f.id, f.folder_id, f.name, f.size, f.sha256, f.mime_type, f.is_favorite, f.is_encrypted, f.manifest_id, f.created_at, f.updated_at,
                    r.action, r.accessed_at
             FROM recent_items r
             JOIN files f ON r.file_id = f.id
             ORDER BY r.accessed_at DESC
             LIMIT ?1",
        ).map_err(|e| TeleCloudError::Database {
            operation: "prepare_list_recent".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let file = Self::map_file(row)?;
            let action_str: String = row.get(11)?;
            let accessed_str: String = row.get(12)?;

            let action = match action_str.as_str() {
                "downloaded" => RecentAction::Downloaded,
                "opened" => RecentAction::Opened,
                "modified" => RecentAction::Modified,
                _ => RecentAction::Uploaded,
            };

            let accessed_at = DateTime::parse_from_rfc3339(&accessed_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(RecentItem {
                file,
                action,
                accessed_at,
            })
        }).map_err(|e| TeleCloudError::Database {
            operation: "query_list_recent".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        let mut items = Vec::new();
        for r in rows {
            if let Ok(item) = r {
                items.push(item);
            }
        }
        Ok(items)
    }

    // -------------------------------------------------------------
    // CHUNKS & MANIFESTS
    // -------------------------------------------------------------

    pub fn save_file_chunks(&self, chunks: &[FileChunk]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "INSERT INTO file_chunks (id, file_id, chunk_index, byte_offset, size, sha256, storage_provider, remote_chat_id, remote_message_id, is_primary, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO NOTHING",
        ).map_err(|e| TeleCloudError::Database {
            operation: "prepare_save_chunks".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        for chunk in chunks {
            stmt.execute(params![
                chunk.id.to_string(),
                chunk.file_id.to_string(),
                chunk.chunk_index,
                chunk.byte_offset as i64,
                chunk.size as i64,
                chunk.sha256,
                chunk.storage_provider,
                chunk.remote_chat_id,
                chunk.remote_message_id,
                if chunk.is_primary { 1 } else { 0 },
                chunk.created_at.to_rfc3339(),
            ])
            .map_err(|e| TeleCloudError::Database {
                operation: "execute_save_chunk".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;
        }
        Ok(())
    }

    pub fn get_file_chunks(&self, file_id: Uuid) -> Result<Vec<FileChunk>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, file_id, chunk_index, byte_offset, size, sha256, storage_provider, remote_chat_id, remote_message_id, is_primary, created_at
             FROM file_chunks WHERE file_id = ?1 ORDER BY chunk_index ASC",
        ).map_err(|e| TeleCloudError::Database {
            operation: "prepare_get_chunks".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        let rows = stmt.query_map(params![file_id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let f_id_str: String = row.get(1)?;
            let c_idx: u32 = row.get(2)?;
            let offset: i64 = row.get(3)?;
            let size: i64 = row.get(4)?;
            let sha: String = row.get(5)?;
            let prov: String = row.get(6)?;
            let chat_id: i64 = row.get(7)?;
            let msg_id: i64 = row.get(8)?;
            let primary: i32 = row.get(9)?;
            let c_str: String = row.get(10)?;

            Ok(FileChunk {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                file_id: Uuid::parse_str(&f_id_str).unwrap_or_default(),
                chunk_index: c_idx,
                byte_offset: offset as u64,
                size: size as u64,
                sha256: sha,
                storage_provider: prov,
                remote_chat_id: chat_id,
                remote_message_id: msg_id,
                is_primary: primary == 1,
                created_at: DateTime::parse_from_rfc3339(&c_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        }).map_err(|e| TeleCloudError::Database {
            operation: "query_chunks".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        let mut chunks = Vec::new();
        for r in rows {
            if let Ok(c) = r {
                chunks.push(c);
            }
        }
        Ok(chunks)
    }

    pub fn save_manifest(&self, file_id: Uuid, version: u32, manifest_json: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO manifests (file_id, version, manifest_json, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(file_id) DO UPDATE SET
                version = excluded.version,
                manifest_json = excluded.manifest_json",
            params![
                file_id.to_string(),
                version,
                manifest_json,
                Utc::now().to_rfc3339()
            ],
        ).map_err(|e| TeleCloudError::Database {
            operation: "save_manifest".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;
        Ok(())
    }

    pub fn get_manifest(&self, file_id: Uuid) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT manifest_json FROM manifests WHERE file_id = ?1")
            .map_err(|e| TeleCloudError::Database {
                operation: "prepare_get_manifest".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;

        let mut rows = stmt
            .query_map(params![file_id.to_string()], |row| row.get(0))
            .map_err(|e| TeleCloudError::Database {
                operation: "query_get_manifest".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;

        if let Some(row) = rows.next() {
            row.map(Some).map_err(|e| TeleCloudError::Database {
                operation: "map_get_manifest".to_string(),
                message: e.to_string(),
                recoverable: false,
            })
        } else {
            Ok(None)
        }
    }

    // -------------------------------------------------------------
    // INTEGRITY RECORDS
    // -------------------------------------------------------------

    pub fn record_integrity_check(&self, record: &IntegrityRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO integrity_records (file_id, last_verified_at, is_valid, details)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(file_id) DO UPDATE SET
                last_verified_at = excluded.last_verified_at,
                is_valid = excluded.is_valid,
                details = excluded.details",
            params![
                record.file_id.to_string(),
                record.last_verified_at.to_rfc3339(),
                if record.is_valid { 1 } else { 0 },
                record.details,
            ],
        ).map_err(|e| TeleCloudError::Database {
            operation: "record_integrity_check".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;
        Ok(())
    }

    pub fn get_integrity_record(&self, file_id: Uuid) -> Result<Option<IntegrityRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT file_id, last_verified_at, is_valid, details FROM integrity_records WHERE file_id = ?1",
        ).map_err(|e| TeleCloudError::Database {
            operation: "prepare_get_integrity".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        let mut rows = stmt.query_map(params![file_id.to_string()], |row| {
            let f_id_str: String = row.get(0)?;
            let v_str: String = row.get(1)?;
            let valid: i32 = row.get(2)?;
            let det: Option<String> = row.get(3)?;

            Ok(IntegrityRecord {
                file_id: Uuid::parse_str(&f_id_str).unwrap_or_default(),
                last_verified_at: DateTime::parse_from_rfc3339(&v_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                is_valid: valid == 1,
                details: det,
            })
        }).map_err(|e| TeleCloudError::Database {
            operation: "query_get_integrity".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        if let Some(row) = rows.next() {
            row.map(Some).map_err(|e| TeleCloudError::Database {
                operation: "map_get_integrity".to_string(),
                message: e.to_string(),
                recoverable: false,
            })
        } else {
            Ok(None)
        }
    }

    // -------------------------------------------------------------
    // SETTINGS & SESSIONS
    // -------------------------------------------------------------

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, Utc::now().to_rfc3339()],
        ).map_err(|e| TeleCloudError::Database {
            operation: "set_setting".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT value FROM settings WHERE key = ?1")
            .map_err(|e| TeleCloudError::Database {
                operation: "prepare_get_setting".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;

        let mut rows = stmt
            .query_map(params![key], |row| row.get(0))
            .map_err(|e| TeleCloudError::Database {
                operation: "query_get_setting".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;

        if let Some(row) = rows.next() {
            row.map(Some).map_err(|e| TeleCloudError::Database {
                operation: "map_get_setting".to_string(),
                message: e.to_string(),
                recoverable: false,
            })
        } else {
            Ok(None)
        }
    }

    pub fn set_session(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sessions (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, Utc::now().to_rfc3339()],
        ).map_err(|e| TeleCloudError::Database {
            operation: "set_session".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;
        Ok(())
    }

    pub fn get_session(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT value FROM sessions WHERE key = ?1")
            .map_err(|e| TeleCloudError::Database {
                operation: "prepare_get_session".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;

        let mut rows = stmt
            .query_map(params![key], |row| row.get(0))
            .map_err(|e| TeleCloudError::Database {
                operation: "query_get_session".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;

        if let Some(row) = rows.next() {
            row.map(Some).map_err(|e| TeleCloudError::Database {
                operation: "map_get_session".to_string(),
                message: e.to_string(),
                recoverable: false,
            })
        } else {
            Ok(None)
        }
    }

    // -------------------------------------------------------------
    // TRANSFERS
    // -------------------------------------------------------------

    pub fn upsert_transfer(&self, transfer: &TransferRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let state_str = serde_json::to_string(&transfer.state).unwrap_or_default();
        let state_clean = state_str.trim_matches('"');
        let dir_str = match transfer.direction {
            TransferDirection::Upload => "UPLOAD",
            TransferDirection::Download => "DOWNLOAD",
        };

        conn.execute(
            "INSERT INTO transfers (id, file_id, direction, local_path, file_name, total_bytes, transferred_bytes, state, speed_bytes_per_sec, eta_seconds, active_chunk_index, total_chunks, error_message, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(id) DO UPDATE SET
                transferred_bytes = excluded.transferred_bytes,
                state = excluded.state,
                speed_bytes_per_sec = excluded.speed_bytes_per_sec,
                eta_seconds = excluded.eta_seconds,
                active_chunk_index = excluded.active_chunk_index,
                error_message = excluded.error_message,
                updated_at = excluded.updated_at",
            params![
                transfer.id.to_string(),
                transfer.file_id.map(|id| id.to_string()),
                dir_str,
                transfer.local_path,
                transfer.file_name,
                transfer.total_bytes as i64,
                transfer.transferred_bytes as i64,
                state_clean,
                transfer.speed_bytes_per_sec as i64,
                transfer.eta_seconds.map(|s| s as i64),
                transfer.active_chunk_index,
                transfer.total_chunks,
                transfer.error_message,
                transfer.created_at.to_rfc3339(),
                transfer.updated_at.to_rfc3339(),
            ],
        ).map_err(|e| TeleCloudError::Database {
            operation: "upsert_transfer".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;
        Ok(())
    }

    pub fn get_transfer(&self, id: Uuid) -> Result<Option<TransferRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, file_id, direction, local_path, file_name, total_bytes, transferred_bytes, state, speed_bytes_per_sec, eta_seconds, active_chunk_index, total_chunks, error_message, created_at, updated_at
             FROM transfers WHERE id = ?1",
        ).map_err(|e| TeleCloudError::Database {
            operation: "prepare_get_transfer".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        let mut rows = stmt
            .query_map(params![id.to_string()], Self::map_transfer)
            .map_err(|e| TeleCloudError::Database {
                operation: "query_get_transfer".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;

        if let Some(row) = rows.next() {
            row.map(Some).map_err(|e| TeleCloudError::Database {
                operation: "map_get_transfer".to_string(),
                message: e.to_string(),
                recoverable: false,
            })
        } else {
            Ok(None)
        }
    }

    pub fn list_transfers(&self) -> Result<Vec<TransferRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, file_id, direction, local_path, file_name, total_bytes, transferred_bytes, state, speed_bytes_per_sec, eta_seconds, active_chunk_index, total_chunks, error_message, created_at, updated_at
             FROM transfers
             ORDER BY created_at DESC",
        ).map_err(|e| TeleCloudError::Database {
            operation: "prepare_list_transfers".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        let rows = stmt.query_map([], Self::map_transfer).map_err(|e| TeleCloudError::Database {
            operation: "query_list_transfers".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        let mut transfers = Vec::new();
        for r in rows {
            if let Ok(t) = r {
                transfers.push(t);
            }
        }
        Ok(transfers)
    }

    pub fn list_active_transfers(&self) -> Result<Vec<TransferRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, file_id, direction, local_path, file_name, total_bytes, transferred_bytes, state, speed_bytes_per_sec, eta_seconds, active_chunk_index, total_chunks, error_message, created_at, updated_at
             FROM transfers WHERE state IN ('QUEUED', 'PREPARING', 'UPLOADING', 'DOWNLOADING', 'VERIFYING', 'RETRYING')
             ORDER BY created_at ASC",
        ).map_err(|e| TeleCloudError::Database {
            operation: "prepare_active_transfers".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        let rows = stmt.query_map([], Self::map_transfer).map_err(|e| TeleCloudError::Database {
            operation: "query_active_transfers".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        let mut transfers = Vec::new();
        for r in rows {
            if let Ok(t) = r {
                transfers.push(t);
            }
        }
        Ok(transfers)
    }

    fn map_folder(row: &rusqlite::Row) -> rusqlite::Result<Folder> {
        let id_str: String = row.get(0)?;
        let parent_str: Option<String> = row.get(1)?;
        let name: String = row.get(2)?;
        let c_str: String = row.get(3)?;
        let u_str: String = row.get(4)?;

        Ok(Folder {
            id: Uuid::parse_str(&id_str).unwrap_or_default(),
            parent_id: parent_str.and_then(|s| Uuid::parse_str(&s).ok()),
            name,
            created_at: DateTime::parse_from_rfc3339(&c_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&u_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }

    fn map_file(row: &rusqlite::Row) -> rusqlite::Result<LogicalFile> {
        let id_str: String = row.get(0)?;
        let folder_str: Option<String> = row.get(1)?;
        let name: String = row.get(2)?;
        let size: i64 = row.get(3)?;
        let sha256: String = row.get(4)?;
        let mime_type: Option<String> = row.get(5)?;
        let is_fav: i32 = row.get(6)?;
        let is_enc: i32 = row.get(7)?;
        let manifest_str: Option<String> = row.get(8)?;
        let c_str: String = row.get(9)?;
        let u_str: String = row.get(10)?;

        Ok(LogicalFile {
            id: Uuid::parse_str(&id_str).unwrap_or_default(),
            folder_id: folder_str.and_then(|s| Uuid::parse_str(&s).ok()),
            name,
            size: size as u64,
            sha256,
            mime_type,
            is_favorite: is_fav == 1,
            is_encrypted: is_enc == 1,
            manifest_id: manifest_str.and_then(|s| Uuid::parse_str(&s).ok()),
            created_at: DateTime::parse_from_rfc3339(&c_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&u_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }

    fn map_transfer(row: &rusqlite::Row) -> rusqlite::Result<TransferRecord> {
        let id_str: String = row.get(0)?;
        let file_str: Option<String> = row.get(1)?;
        let dir_str: String = row.get(2)?;
        let local_path: String = row.get(3)?;
        let file_name: String = row.get(4)?;
        let total: i64 = row.get(5)?;
        let transferred: i64 = row.get(6)?;
        let state_str: String = row.get(7)?;
        let speed: i64 = row.get(8)?;
        let eta: Option<i64> = row.get(9)?;
        let act_chunk: Option<u32> = row.get(10)?;
        let tot_chunk: Option<u32> = row.get(11)?;
        let err: Option<String> = row.get(12)?;
        let c_str: String = row.get(13)?;
        let u_str: String = row.get(14)?;

        let state = match state_str.as_str() {
            "QUEUED" => TransferState::Queued,
            "PREPARING" => TransferState::Preparing,
            "UPLOADING" => TransferState::Uploading,
            "DOWNLOADING" => TransferState::Downloading,
            "PAUSED" => TransferState::Paused,
            "COMPLETED" => TransferState::Completed,
            "FAILED" => TransferState::Failed,
            "CANCELLED" => TransferState::Cancelled,
            "VERIFYING" => TransferState::Verifying,
            "RETRYING" => TransferState::Retrying,
            _ => TransferState::Queued,
        };

        let direction = match dir_str.as_str() {
            "DOWNLOAD" => TransferDirection::Download,
            _ => TransferDirection::Upload,
        };

        Ok(TransferRecord {
            id: Uuid::parse_str(&id_str).unwrap_or_default(),
            file_id: file_str.and_then(|s| Uuid::parse_str(&s).ok()),
            direction,
            local_path,
            file_name,
            total_bytes: total as u64,
            transferred_bytes: transferred as u64,
            state,
            speed_bytes_per_sec: speed as u64,
            eta_seconds: eta.map(|e| e as u64),
            active_chunk_index: act_chunk,
            total_chunks: tot_chunk,
            error_message: err,
            created_at: DateTime::parse_from_rfc3339(&c_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&u_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }
}
