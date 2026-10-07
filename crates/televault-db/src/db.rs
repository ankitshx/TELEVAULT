//! Database connection and high-level persistence engine for TELEVAULT.

use crate::error::{DbError, Result};
use crate::migrations::run_migrations;
use crate::models::{
    ChunkRecord, FileRecord, HealthStatus, ManifestRecord, ProfileRecord, ScheduleHistoryRecord,
    ScheduleRecord, SearchResult, SnapshotRecord, TransferJobRecord, VersionRecord,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::path::Path;
use std::sync::{Arc, Mutex};
use televault_core::ids::{ChunkId, FileId, JobId, ProfileId, ScheduleId, SnapshotId, VersionId};
use televault_core::models::{BackupStatus, TransferDirection, TransferStatus};
use televault_core::paths::PathManager;
use televault_manifest::ManifestV1;

/// Embedded SQLite database instance for TELEVAULT.
///
/// Thread-safe and cloneable, encapsulating SQLite connection management,
/// pragmas, schema migrations, and typed repository operations.
#[derive(Debug, Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Opens or creates a database at the specified filesystem path and executes migrations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    DbError::Connection(format!(
                        "Failed to create database parent directory '{}': {e}",
                        parent.display()
                    ))
                })?;
            }
        }

        let mut conn = Connection::open(path).map_err(|e| {
            DbError::Connection(format!(
                "Failed to open database file '{}': {e}",
                path.display()
            ))
        })?;

        Self::configure_pragmas(&mut conn, true)?;
        run_migrations(&mut conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Opens an in-memory SQLite database, configures pragmas, and executes migrations.
    pub fn open_in_memory() -> Result<Self> {
        let mut conn = Connection::open_in_memory()
            .map_err(|e| DbError::Connection(format!("Failed to open in-memory database: {e}")))?;

        Self::configure_pragmas(&mut conn, false)?;
        run_migrations(&mut conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Opens the default production database location managed by [`PathManager`].
    pub fn open_default(paths: &PathManager) -> Result<Self> {
        Self::open(paths.db_file())
    }

    /// Configures essential SQLite pragmas for safety, durability, and concurrency.
    fn configure_pragmas(conn: &mut Connection, is_file: bool) -> Result<()> {
        // Enforce referential integrity
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        // 5 second busy timeout to handle concurrent locking
        conn.execute_batch("PRAGMA busy_timeout = 5000;")?;

        // Write-Ahead Logging for file-backed storage
        if is_file {
            conn.execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;",
            )?;
        }

        Ok(())
    }

    /// Executes a closure with exclusive access to the raw SQLite connection.
    pub fn with_connection<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut Connection) -> Result<T>,
    {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::LockPoisoned(e.to_string()))?;
        f(&mut conn)
    }

    /// Executes a closure inside an atomic SQLite transaction.
    ///
    /// Automatically rolls back if the closure returns an error or panics.
    pub fn with_transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Transaction) -> Result<T>,
    {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| DbError::LockPoisoned(e.to_string()))?;
        let tx = conn
            .transaction()
            .map_err(|e| DbError::Transaction(e.to_string()))?;
        let result = f(&tx)?;
        tx.commit()
            .map_err(|e| DbError::Transaction(e.to_string()))?;
        Ok(result)
    }

    // =========================================================================
    // Diagnostic / Health Checks
    // =========================================================================

    /// Verifies database integrity, foreign-key enforcement, and table existence.
    pub fn health_check(&self) -> Result<HealthStatus> {
        self.with_connection(|conn| {
            // Check foreign keys status
            let fk_enabled: i64 = conn.query_row("PRAGMA foreign_keys;", [], |r| r.get(0))?;

            // Quick check
            let integrity: String = conn.query_row("PRAGMA quick_check;", [], |r| r.get(0))?;

            // Count migrations
            let applied_migrations: i64 = conn
                .query_row("SELECT COUNT(*) FROM refinery_schema_history;", [], |r| {
                    r.get(0)
                })
                .unwrap_or(0);

            // Entity counts
            let total_profiles: i64 =
                conn.query_row("SELECT COUNT(*) FROM profiles;", [], |r| r.get(0))?;
            let total_files: i64 =
                conn.query_row("SELECT COUNT(*) FROM files;", [], |r| r.get(0))?;
            let total_chunks: i64 =
                conn.query_row("SELECT COUNT(*) FROM chunks;", [], |r| r.get(0))?;

            let is_healthy = integrity.eq_ignore_ascii_case("ok") && fk_enabled == 1;

            Ok(HealthStatus {
                is_healthy,
                applied_migrations: applied_migrations as usize,
                total_profiles: total_profiles as usize,
                total_files: total_files as usize,
                total_chunks: total_chunks as usize,
                integrity_check: integrity,
                foreign_keys_enabled: fk_enabled == 1,
            })
        })
    }

    // =========================================================================
    // Backup Profiles
    // =========================================================================

    /// Inserts a new backup profile.
    pub fn create_profile(&self, profile: &ProfileRecord) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO profiles (profile_id, name, description, source_path, enabled, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);",
                params![
                    profile.profile_id.as_str(),
                    profile.name,
                    profile.description,
                    profile.source_path,
                    if profile.enabled { 1 } else { 0 },
                    profile.created_at,
                    profile.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    /// Retrieves a profile by its unique ID.
    pub fn get_profile(&self, id: &ProfileId) -> Result<Option<ProfileRecord>> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT profile_id, name, description, source_path, enabled, created_at, updated_at
                 FROM profiles WHERE profile_id = ?1;",
                params![id.as_str()],
                |r| {
                    let pid_str: String = r.get(0)?;
                    let enabled_num: i64 = r.get(4)?;
                    Ok((
                        pid_str,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, String>(3)?,
                        enabled_num != 0,
                        r.get::<_, String>(5)?,
                        r.get::<_, String>(6)?,
                    ))
                },
            )
            .optional()?
            .map(|(pid_str, name, desc, src, enabled, cat, uat)| {
                let pid =
                    ProfileId::new(pid_str).map_err(|e| DbError::InvalidData(e.to_string()))?;
                Ok(ProfileRecord {
                    profile_id: pid,
                    name,
                    description: desc,
                    source_path: src,
                    enabled,
                    created_at: cat,
                    updated_at: uat,
                })
            })
            .transpose()
        })
    }

    /// Retrieves all profiles.
    pub fn list_profiles(&self) -> Result<Vec<ProfileRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT profile_id, name, description, source_path, enabled, created_at, updated_at
                 FROM profiles ORDER BY name ASC;",
            )?;
            let rows = stmt.query_map([], |r| {
                let pid_str: String = r.get(0)?;
                let enabled_num: i64 = r.get(4)?;
                Ok((
                    pid_str,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                    enabled_num != 0,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                ))
            })?;

            let mut profiles = Vec::new();
            for item in rows {
                let (pid_str, name, desc, src, enabled, cat, uat) = item?;
                let pid =
                    ProfileId::new(pid_str).map_err(|e| DbError::InvalidData(e.to_string()))?;
                profiles.push(ProfileRecord {
                    profile_id: pid,
                    name,
                    description: desc,
                    source_path: src,
                    enabled,
                    created_at: cat,
                    updated_at: uat,
                });
            }
            Ok(profiles)
        })
    }

    /// Updates an existing profile's mutable fields.
    pub fn update_profile(&self, profile: &ProfileRecord) -> Result<()> {
        self.with_connection(|conn| {
            let affected = conn.execute(
                "UPDATE profiles
                 SET name = ?2, description = ?3, source_path = ?4, enabled = ?5, updated_at = ?6
                 WHERE profile_id = ?1;",
                params![
                    profile.profile_id.as_str(),
                    profile.name,
                    profile.description,
                    profile.source_path,
                    if profile.enabled { 1 } else { 0 },
                    profile.updated_at,
                ],
            )?;
            if affected == 0 {
                return Err(DbError::NotFound {
                    entity: "Profile",
                    id: profile.profile_id.to_string(),
                });
            }
            Ok(())
        })
    }

    /// Enables or disables a profile.
    pub fn set_profile_enabled(&self, id: &ProfileId, enabled: bool) -> Result<()> {
        self.with_connection(|conn| {
            let affected = conn.execute(
                "UPDATE profiles SET enabled = ?2 WHERE profile_id = ?1;",
                params![id.as_str(), if enabled { 1 } else { 0 }],
            )?;
            if affected == 0 {
                return Err(DbError::NotFound {
                    entity: "Profile",
                    id: id.to_string(),
                });
            }
            Ok(())
        })
    }

    /// Deletes a profile.
    pub fn delete_profile(&self, id: &ProfileId) -> Result<()> {
        self.with_connection(|conn| {
            let affected = conn.execute(
                "DELETE FROM profiles WHERE profile_id = ?1;",
                params![id.as_str()],
            )?;
            if affected == 0 {
                return Err(DbError::NotFound {
                    entity: "Profile",
                    id: id.to_string(),
                });
            }
            Ok(())
        })
    }

    // =========================================================================
    // Logical Files
    // =========================================================================

    /// Inserts a new logical file record.
    pub fn create_file(&self, file: &FileRecord) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO files (
                    file_id, profile_id, file_name, relative_path, original_size,
                    mime_type, status, logical_file_hash, created_at, modified_at,
                    created_timestamp, updated_timestamp
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12);",
                params![
                    file.file_id.as_str(),
                    file.profile_id.as_ref().map(|p| p.as_str()),
                    file.file_name,
                    file.relative_path,
                    file.original_size as i64,
                    file.mime_type,
                    file.status,
                    file.logical_file_hash,
                    file.created_at,
                    file.modified_at,
                    file.created_timestamp,
                    file.updated_timestamp,
                ],
            )?;
            Ok(())
        })
    }

    /// Retrieves a logical file record by ID.
    pub fn get_file(&self, id: &FileId) -> Result<Option<FileRecord>> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT file_id, profile_id, file_name, relative_path, original_size,
                        mime_type, status, logical_file_hash, created_at, modified_at,
                        created_timestamp, updated_timestamp
                 FROM files WHERE file_id = ?1;",
                params![id.as_str()],
                |r| {
                    let fid_str: String = r.get(0)?;
                    let pid_opt: Option<String> = r.get(1)?;
                    let size_i64: i64 = r.get(4)?;
                    Ok((
                        fid_str,
                        pid_opt,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        size_i64 as u64,
                        r.get::<_, Option<String>>(5)?,
                        r.get::<_, String>(6)?,
                        r.get::<_, Option<String>>(7)?,
                        r.get::<_, String>(8)?,
                        r.get::<_, Option<String>>(9)?,
                        r.get::<_, String>(10)?,
                        r.get::<_, String>(11)?,
                    ))
                },
            )
            .optional()?
            .map(
                |(
                    fid_str,
                    pid_opt,
                    fname,
                    rpath,
                    size,
                    mime,
                    status,
                    fhash,
                    cat,
                    mat,
                    ctime,
                    utime,
                )| {
                    let fid =
                        FileId::new(fid_str).map_err(|e| DbError::InvalidData(e.to_string()))?;
                    let pid = match pid_opt {
                        Some(s) => Some(
                            ProfileId::new(s).map_err(|e| DbError::InvalidData(e.to_string()))?,
                        ),
                        None => None,
                    };
                    Ok(FileRecord {
                        file_id: fid,
                        profile_id: pid,
                        file_name: fname,
                        relative_path: rpath,
                        original_size: size,
                        mime_type: mime,
                        status,
                        logical_file_hash: fhash,
                        created_at: cat,
                        modified_at: mat,
                        created_timestamp: ctime,
                        updated_timestamp: utime,
                    })
                },
            )
            .transpose()
        })
    }

    /// Lists logical files optionally filtered by profile ID.
    pub fn list_files_by_profile(&self, profile_id: Option<&ProfileId>) -> Result<Vec<FileRecord>> {
        self.with_connection(|conn| {
            let mut records = Vec::new();
            if let Some(pid) = profile_id {
                let mut stmt = conn.prepare(
                    "SELECT file_id, profile_id, file_name, relative_path, original_size,
                            mime_type, status, logical_file_hash, created_at, modified_at,
                            created_timestamp, updated_timestamp
                     FROM files WHERE profile_id = ?1 ORDER BY relative_path ASC;",
                )?;
                let rows = stmt.query_map(params![pid.as_str()], Self::map_file_row)?;
                for row in rows {
                    records.push(row?);
                }
            } else {
                let mut stmt = conn.prepare(
                    "SELECT file_id, profile_id, file_name, relative_path, original_size,
                            mime_type, status, logical_file_hash, created_at, modified_at,
                            created_timestamp, updated_timestamp
                     FROM files ORDER BY file_name ASC;",
                )?;
                let rows = stmt.query_map([], Self::map_file_row)?;
                for row in rows {
                    records.push(row?);
                }
            }
            Ok(records)
        })
    }

    /// Retrieves a file record by profile ID and relative path.
    pub fn get_file_by_relative_path(
        &self,
        profile_id: &ProfileId,
        relative_path: &str,
    ) -> Result<Option<FileRecord>> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT file_id, profile_id, file_name, relative_path, original_size,
                        mime_type, status, logical_file_hash, created_at, modified_at,
                        created_timestamp, updated_timestamp
                 FROM files WHERE profile_id = ?1 AND relative_path = ?2;",
                params![profile_id.as_str(), relative_path],
                Self::map_file_row,
            )
            .optional()
            .map_err(DbError::from)
        })
    }

    /// Helper for mapping a file row to [`FileRecord`].
    fn map_file_row(r: &rusqlite::Row) -> rusqlite::Result<FileRecord> {
        let fid_str: String = r.get(0)?;
        let pid_opt: Option<String> = r.get(1)?;
        let size_i64: i64 = r.get(4)?;

        let fid = FileId::new(fid_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let pid = match pid_opt {
            Some(s) => Some(ProfileId::new(s).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?),
            None => None,
        };

        Ok(FileRecord {
            file_id: fid,
            profile_id: pid,
            file_name: r.get(2)?,
            relative_path: r.get(3)?,
            original_size: size_i64 as u64,
            mime_type: r.get(5)?,
            status: r.get(6)?,
            logical_file_hash: r.get(7)?,
            created_at: r.get(8)?,
            modified_at: r.get(9)?,
            created_timestamp: r.get(10)?,
            updated_timestamp: r.get(11)?,
        })
    }

    /// Updates mutable fields of a logical file record.
    pub fn update_file(&self, file: &FileRecord) -> Result<()> {
        self.with_connection(|conn| {
            let affected = conn.execute(
                "UPDATE files
                 SET file_name = ?2, relative_path = ?3, original_size = ?4, mime_type = ?5,
                     status = ?6, logical_file_hash = ?7, modified_at = ?8, updated_timestamp = ?9
                 WHERE file_id = ?1;",
                params![
                    file.file_id.as_str(),
                    file.file_name,
                    file.relative_path,
                    file.original_size as i64,
                    file.mime_type,
                    file.status,
                    file.logical_file_hash,
                    file.modified_at,
                    file.updated_timestamp,
                ],
            )?;
            if affected == 0 {
                return Err(DbError::NotFound {
                    entity: "File",
                    id: file.file_id.to_string(),
                });
            }
            Ok(())
        })
    }

    /// Deletes a logical file record by ID.
    pub fn delete_file(&self, id: &FileId) -> Result<()> {
        self.with_connection(|conn| {
            let affected = conn.execute(
                "DELETE FROM files WHERE file_id = ?1;",
                params![id.as_str()],
            )?;
            if affected == 0 {
                return Err(DbError::NotFound {
                    entity: "File",
                    id: id.to_string(),
                });
            }
            Ok(())
        })
    }

    /// Returns the total number of logical files stored in the database.
    pub fn count_files(&self) -> Result<usize> {
        self.with_connection(|conn| {
            let count: i64 = conn.query_row("SELECT COUNT(*) FROM files;", [], |r| r.get(0))?;
            Ok(count as usize)
        })
    }

    // =========================================================================
    // Manifests
    // =========================================================================

    /// Persists a [`ManifestV1`] instance as the authoritative file manifest.
    pub fn save_manifest(&self, manifest: &ManifestV1) -> Result<()> {
        let serialized = manifest.to_json()?;
        let manifest_id = manifest.manifest_id.clone();
        let file_id = manifest.logical_file.file_id.clone();
        let version_str = manifest.manifest_version.to_string();
        let now = manifest
            .logical_file
            .created_at
            .map(|ts| ts.to_string())
            .unwrap_or_else(|| "1970-01-01T00:00:00Z".into());

        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO manifests (manifest_id, file_id, manifest_version, serialized_manifest, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(manifest_id) DO UPDATE SET
                    serialized_manifest = excluded.serialized_manifest,
                    updated_at = excluded.updated_at;",
                params![
                    manifest_id,
                    file_id.as_str(),
                    version_str,
                    serialized,
                    now,
                    now,
                ],
            )?;
            Ok(())
        })
    }

    /// Inserts a raw manifest record.
    pub fn create_manifest(&self, manifest: &ManifestRecord) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO manifests (manifest_id, file_id, manifest_version, serialized_manifest, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
                params![
                    manifest.manifest_id,
                    manifest.file_id.as_str(),
                    manifest.manifest_version,
                    manifest.serialized_manifest,
                    manifest.created_at,
                    manifest.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    /// Retrieves and deserializes a [`ManifestV1`] by its manifest ID.
    pub fn get_manifest(&self, manifest_id: &str) -> Result<Option<ManifestV1>> {
        self.with_connection(|conn| {
            let raw: Option<String> = conn
                .query_row(
                    "SELECT serialized_manifest FROM manifests WHERE manifest_id = ?1;",
                    params![manifest_id],
                    |r| r.get(0),
                )
                .optional()?;

            match raw {
                Some(json) => {
                    let manifest = ManifestV1::from_json(&json)
                        .map_err(|e| DbError::InvalidData(e.to_string()))?;
                    Ok(Some(manifest))
                }
                None => Ok(None),
            }
        })
    }

    /// Retrieves and deserializes a [`ManifestV1`] by associated logical file ID.
    pub fn get_manifest_by_file_id(&self, file_id: &FileId) -> Result<Option<ManifestV1>> {
        self.with_connection(|conn| {
            let raw: Option<String> = conn
                .query_row(
                    "SELECT serialized_manifest FROM manifests WHERE file_id = ?1 ORDER BY created_at DESC LIMIT 1;",
                    params![file_id.as_str()],
                    |r| r.get(0),
                )
                .optional()?;

            match raw {
                Some(json) => {
                    let manifest = ManifestV1::from_json(&json)
                        .map_err(|e| DbError::InvalidData(e.to_string()))?;
                    Ok(Some(manifest))
                }
                None => Ok(None),
            }
        })
    }

    /// Retrieves raw manifest record metadata.
    pub fn get_manifest_record(&self, manifest_id: &str) -> Result<Option<ManifestRecord>> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT manifest_id, file_id, manifest_version, serialized_manifest, created_at, updated_at
                 FROM manifests WHERE manifest_id = ?1;",
                params![manifest_id],
                |r| {
                    let mid: String = r.get(0)?;
                    let fid_str: String = r.get(1)?;
                    let mver: String = r.get(2)?;
                    let sman: String = r.get(3)?;
                    let cat: String = r.get(4)?;
                    let uat: String = r.get(5)?;
                    Ok((mid, fid_str, mver, sman, cat, uat))
                },
            )
            .optional()?
            .map(|(mid, fid_str, mver, sman, cat, uat)| {
                let fid = FileId::new(fid_str)
                    .map_err(|e| DbError::InvalidData(e.to_string()))?;
                Ok(ManifestRecord {
                    manifest_id: mid,
                    file_id: fid,
                    manifest_version: mver,
                    serialized_manifest: sman,
                    created_at: cat,
                    updated_at: uat,
                })
            })
            .transpose()
        })
    }

    /// Deletes a manifest record by ID.
    pub fn delete_manifest(&self, manifest_id: &str) -> Result<()> {
        self.with_connection(|conn| {
            let affected = conn.execute(
                "DELETE FROM manifests WHERE manifest_id = ?1;",
                params![manifest_id],
            )?;
            if affected == 0 {
                return Err(DbError::NotFound {
                    entity: "Manifest",
                    id: manifest_id.to_string(),
                });
            }
            Ok(())
        })
    }

    // =========================================================================
    // Physical Chunks
    // =========================================================================

    /// Inserts a physical chunk record.
    pub fn create_chunk(&self, chunk: &ChunkRecord) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO chunks (
                    chunk_id, file_id, manifest_id, chunk_index, plaintext_size,
                    stored_size, integrity_hash, storage_reference, status,
                    created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11);",
                params![
                    chunk.chunk_id.as_str(),
                    chunk.file_id.as_str(),
                    chunk.manifest_id,
                    chunk.chunk_index as i64,
                    chunk.plaintext_size as i64,
                    chunk.stored_size as i64,
                    chunk.integrity_hash,
                    chunk.storage_reference,
                    chunk.status,
                    chunk.created_at,
                    chunk.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    /// Inserts multiple chunks in a single atomic batch.
    pub fn create_chunks(&self, chunks: &[ChunkRecord]) -> Result<()> {
        self.with_transaction(|tx| {
            for chunk in chunks {
                tx.execute(
                    "INSERT INTO chunks (
                        chunk_id, file_id, manifest_id, chunk_index, plaintext_size,
                        stored_size, integrity_hash, storage_reference, status,
                        created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11);",
                    params![
                        chunk.chunk_id.as_str(),
                        chunk.file_id.as_str(),
                        chunk.manifest_id,
                        chunk.chunk_index as i64,
                        chunk.plaintext_size as i64,
                        chunk.stored_size as i64,
                        chunk.integrity_hash,
                        chunk.storage_reference,
                        chunk.status,
                        chunk.created_at,
                        chunk.updated_at,
                    ],
                )?;
            }
            Ok(())
        })
    }

    /// Retrieves a chunk record by its chunk ID.
    pub fn get_chunk(&self, chunk_id: &ChunkId) -> Result<Option<ChunkRecord>> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT chunk_id, file_id, manifest_id, chunk_index, plaintext_size,
                        stored_size, integrity_hash, storage_reference, status,
                        created_at, updated_at
                 FROM chunks WHERE chunk_id = ?1;",
                params![chunk_id.as_str()],
                Self::map_chunk_row,
            )
            .optional()
            .map_err(DbError::from)
        })
    }

    /// Lists all chunks for a logical file, guaranteed strictly ordered by `chunk_index ASC`.
    pub fn list_chunks_by_file(&self, file_id: &FileId) -> Result<Vec<ChunkRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT chunk_id, file_id, manifest_id, chunk_index, plaintext_size,
                        stored_size, integrity_hash, storage_reference, status,
                        created_at, updated_at
                 FROM chunks WHERE file_id = ?1 ORDER BY chunk_index ASC;",
            )?;
            let rows = stmt.query_map(params![file_id.as_str()], Self::map_chunk_row)?;
            let mut chunks = Vec::new();
            for r in rows {
                chunks.push(r?);
            }
            Ok(chunks)
        })
    }

    /// Lists all chunks for a manifest, guaranteed strictly ordered by `chunk_index ASC`.
    pub fn list_chunks_by_manifest(&self, manifest_id: &str) -> Result<Vec<ChunkRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT chunk_id, file_id, manifest_id, chunk_index, plaintext_size,
                        stored_size, integrity_hash, storage_reference, status,
                        created_at, updated_at
                 FROM chunks WHERE manifest_id = ?1 ORDER BY chunk_index ASC;",
            )?;
            let rows = stmt.query_map(params![manifest_id], Self::map_chunk_row)?;
            let mut chunks = Vec::new();
            for r in rows {
                chunks.push(r?);
            }
            Ok(chunks)
        })
    }

    /// Helper for mapping a chunk row to [`ChunkRecord`].
    fn map_chunk_row(r: &rusqlite::Row) -> rusqlite::Result<ChunkRecord> {
        let cid_str: String = r.get(0)?;
        let fid_str: String = r.get(1)?;
        let mid: String = r.get(2)?;
        let cidx: i64 = r.get(3)?;
        let psize: i64 = r.get(4)?;
        let ssize: i64 = r.get(5)?;
        let hash: String = r.get(6)?;
        let sref: String = r.get(7)?;
        let status: String = r.get(8)?;
        let cat: String = r.get(9)?;
        let uat: String = r.get(10)?;

        let cid = ChunkId::new(cid_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let fid = FileId::new(fid_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?;

        Ok(ChunkRecord {
            chunk_id: cid,
            file_id: fid,
            manifest_id: mid,
            chunk_index: cidx as u32,
            plaintext_size: psize as u64,
            stored_size: ssize as u64,
            integrity_hash: hash,
            storage_reference: sref,
            status,
            created_at: cat,
            updated_at: uat,
        })
    }

    /// Updates status of a physical chunk.
    pub fn update_chunk_status(&self, chunk_id: &ChunkId, status: &str) -> Result<()> {
        self.with_connection(|conn| {
            let affected = conn.execute(
                "UPDATE chunks SET status = ?2 WHERE chunk_id = ?1;",
                params![chunk_id.as_str(), status],
            )?;
            if affected == 0 {
                return Err(DbError::NotFound {
                    entity: "Chunk",
                    id: chunk_id.to_string(),
                });
            }
            Ok(())
        })
    }

    /// Returns chunk count for a logical file.
    pub fn count_chunks_for_file(&self, file_id: &FileId) -> Result<usize> {
        self.with_connection(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM chunks WHERE file_id = ?1;",
                params![file_id.as_str()],
                |r| r.get(0),
            )?;
            Ok(count as usize)
        })
    }

    /// Deletes all physical chunks associated with a logical file.
    pub fn delete_chunks_by_file(&self, file_id: &FileId) -> Result<usize> {
        self.with_connection(|conn| {
            let affected = conn.execute(
                "DELETE FROM chunks WHERE file_id = ?1;",
                params![file_id.as_str()],
            )?;
            Ok(affected)
        })
    }

    // =========================================================================
    // Snapshots
    // =========================================================================

    /// Inserts a backup snapshot.
    pub fn create_snapshot(&self, snapshot: &SnapshotRecord) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO snapshots (snapshot_id, profile_id, status, metadata, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5);",
                params![
                    snapshot.snapshot_id.as_str(),
                    snapshot.profile_id.as_str(),
                    snapshot.status.to_string(),
                    snapshot.metadata,
                    snapshot.created_at,
                ],
            )?;
            Ok(())
        })
    }

    /// Retrieves a snapshot by ID.
    pub fn get_snapshot(&self, id: &SnapshotId) -> Result<Option<SnapshotRecord>> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT snapshot_id, profile_id, status, metadata, created_at
                 FROM snapshots WHERE snapshot_id = ?1;",
                params![id.as_str()],
                Self::map_snapshot_row,
            )
            .optional()
            .map_err(DbError::from)
        })
    }

    /// Lists all snapshots for a profile.
    pub fn list_snapshots_by_profile(&self, profile_id: &ProfileId) -> Result<Vec<SnapshotRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT snapshot_id, profile_id, status, metadata, created_at
                 FROM snapshots WHERE profile_id = ?1 ORDER BY created_at DESC;",
            )?;
            let rows = stmt.query_map(params![profile_id.as_str()], Self::map_snapshot_row)?;
            let mut snapshots = Vec::new();
            for r in rows {
                snapshots.push(r?);
            }
            Ok(snapshots)
        })
    }

    /// Retrieves the most recent snapshot for a profile.
    pub fn get_latest_snapshot(&self, profile_id: &ProfileId) -> Result<Option<SnapshotRecord>> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT snapshot_id, profile_id, status, metadata, created_at
                 FROM snapshots WHERE profile_id = ?1 ORDER BY created_at DESC LIMIT 1;",
                params![profile_id.as_str()],
                Self::map_snapshot_row,
            )
            .optional()
            .map_err(DbError::from)
        })
    }

    /// Lists all file records and their versions associated with a snapshot.
    pub fn list_files_by_snapshot(
        &self,
        snapshot_id: &SnapshotId,
    ) -> Result<Vec<(FileRecord, VersionRecord)>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT f.file_id, f.profile_id, f.file_name, f.relative_path, f.original_size,
                        f.mime_type, f.status, f.logical_file_hash, f.created_at, f.modified_at,
                        f.created_timestamp, f.updated_timestamp,
                        v.version_id, v.file_id, v.snapshot_id, v.manifest_id, v.status, v.created_at
                 FROM versions v
                 INNER JOIN files f ON v.file_id = f.file_id
                 WHERE v.snapshot_id = ?1
                 ORDER BY f.relative_path ASC;",
            )?;
            let rows = stmt.query_map(params![snapshot_id.as_str()], |row| {
                let file = Self::map_file_row(row)?;
                let vid_str: String = row.get(12)?;
                let fid_str: String = row.get(13)?;
                let sid_str: String = row.get(14)?;
                let mid: String = row.get(15)?;
                let vstatus: String = row.get(16)?;
                let vcat: String = row.get(17)?;

                let vid = VersionId::new(vid_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        12,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
                let fid = FileId::new(fid_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        13,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
                let sid = SnapshotId::new(sid_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        14,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

                let version = VersionRecord {
                    version_id: vid,
                    file_id: fid,
                    snapshot_id: sid,
                    manifest_id: mid,
                    status: vstatus,
                    created_at: vcat,
                };

                Ok((file, version))
            })?;

            let mut results = Vec::new();
            for r in rows {
                results.push(r?);
            }
            Ok(results)
        })
    }

    /// Helper for mapping a snapshot row to [`SnapshotRecord`].
    fn map_snapshot_row(r: &rusqlite::Row) -> rusqlite::Result<SnapshotRecord> {
        let sid_str: String = r.get(0)?;
        let pid_str: String = r.get(1)?;
        let status_str: String = r.get(2)?;
        let meta: Option<String> = r.get(3)?;
        let cat: String = r.get(4)?;

        let sid = SnapshotId::new(sid_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let pid = ProfileId::new(pid_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let status = match status_str.as_str() {
            "idle" => BackupStatus::Idle,
            "scanning" => BackupStatus::Scanning,
            "backing_up" | "backingup" => BackupStatus::BackingUp,
            "completed" => BackupStatus::Completed,
            "failed" => BackupStatus::Failed,
            "cancelled" => BackupStatus::Cancelled,
            other => {
                return Err(rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    format!("unknown BackupStatus: '{other}'").into(),
                ));
            }
        };

        Ok(SnapshotRecord {
            snapshot_id: sid,
            profile_id: pid,
            status,
            metadata: meta,
            created_at: cat,
        })
    }

    /// Updates status of a snapshot.
    pub fn update_snapshot_status(&self, id: &SnapshotId, status: BackupStatus) -> Result<()> {
        self.update_snapshot_status_and_metadata(id, status, None)
    }

    /// Updates status and metadata of a snapshot.
    pub fn update_snapshot_status_and_metadata(
        &self,
        id: &SnapshotId,
        status: BackupStatus,
        metadata: Option<&str>,
    ) -> Result<()> {
        self.with_connection(|conn| {
            let affected = conn.execute(
                "UPDATE snapshots SET status = ?2, metadata = ?3 WHERE snapshot_id = ?1;",
                params![id.as_str(), status.to_string(), metadata],
            )?;
            if affected == 0 {
                return Err(DbError::NotFound {
                    entity: "Snapshot",
                    id: id.to_string(),
                });
            }
            Ok(())
        })
    }

    // =========================================================================
    // File Versions
    // =========================================================================

    /// Inserts a file version record.
    pub fn create_version(&self, version: &VersionRecord) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO versions (version_id, file_id, snapshot_id, manifest_id, status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
                params![
                    version.version_id.as_str(),
                    version.file_id.as_str(),
                    version.snapshot_id.as_str(),
                    version.manifest_id,
                    version.status,
                    version.created_at,
                ],
            )?;
            Ok(())
        })
    }

    /// Retrieves a file version by ID.
    pub fn get_version(&self, id: &VersionId) -> Result<Option<VersionRecord>> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT version_id, file_id, snapshot_id, manifest_id, status, created_at
                 FROM versions WHERE version_id = ?1;",
                params![id.as_str()],
                Self::map_version_row,
            )
            .optional()
            .map_err(DbError::from)
        })
    }

    /// Lists all file versions associated with a snapshot.
    pub fn list_versions_by_snapshot(
        &self,
        snapshot_id: &SnapshotId,
    ) -> Result<Vec<VersionRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT version_id, file_id, snapshot_id, manifest_id, status, created_at
                 FROM versions WHERE snapshot_id = ?1 ORDER BY created_at ASC;",
            )?;
            let rows = stmt.query_map(params![snapshot_id.as_str()], Self::map_version_row)?;
            let mut versions = Vec::new();
            for r in rows {
                versions.push(r?);
            }
            Ok(versions)
        })
    }

    /// Lists all versions of a specific logical file.
    pub fn list_versions_by_file(&self, file_id: &FileId) -> Result<Vec<VersionRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT version_id, file_id, snapshot_id, manifest_id, status, created_at
                 FROM versions WHERE file_id = ?1 ORDER BY created_at DESC;",
            )?;
            let rows = stmt.query_map(params![file_id.as_str()], Self::map_version_row)?;
            let mut versions = Vec::new();
            for r in rows {
                versions.push(r?);
            }
            Ok(versions)
        })
    }

    /// Helper for mapping a version row to [`VersionRecord`].
    fn map_version_row(r: &rusqlite::Row) -> rusqlite::Result<VersionRecord> {
        let vid_str: String = r.get(0)?;
        let fid_str: String = r.get(1)?;
        let sid_str: String = r.get(2)?;
        let mid: String = r.get(3)?;
        let status: String = r.get(4)?;
        let cat: String = r.get(5)?;

        let vid = VersionId::new(vid_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let fid = FileId::new(fid_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let sid = SnapshotId::new(sid_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
        })?;

        Ok(VersionRecord {
            version_id: vid,
            file_id: fid,
            snapshot_id: sid,
            manifest_id: mid,
            status,
            created_at: cat,
        })
    }

    // =========================================================================
    // Transfer Jobs
    // =========================================================================

    /// Inserts a new transfer job.
    pub fn create_transfer_job(&self, job: &TransferJobRecord) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO transfer_jobs (
                    job_id, file_id, chunk_id, direction, status, progress,
                    retry_count, error_message, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10);",
                params![
                    job.job_id.as_str(),
                    job.file_id.as_str(),
                    job.chunk_id.as_ref().map(|c| c.as_str()),
                    job.direction.to_string(),
                    job.status.to_string(),
                    job.progress as i64,
                    job.retry_count as i64,
                    job.error_message,
                    job.created_at,
                    job.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    /// Inserts or updates an existing transfer job record by its primary key.
    pub fn upsert_transfer_job(&self, job: &TransferJobRecord) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO transfer_jobs (
                    job_id, file_id, chunk_id, direction, status, progress,
                    retry_count, error_message, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(job_id) DO UPDATE SET
                    status = excluded.status,
                    progress = excluded.progress,
                    retry_count = excluded.retry_count,
                    error_message = excluded.error_message,
                    updated_at = excluded.updated_at;",
                params![
                    job.job_id.as_str(),
                    job.file_id.as_str(),
                    job.chunk_id.as_ref().map(|c| c.as_str()),
                    job.direction.to_string(),
                    job.status.to_string(),
                    job.progress as i64,
                    job.retry_count as i64,
                    job.error_message,
                    job.created_at,
                    job.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    /// Retrieves a transfer job by ID.
    pub fn get_transfer_job(&self, id: &JobId) -> Result<Option<TransferJobRecord>> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT job_id, file_id, chunk_id, direction, status, progress,
                        retry_count, error_message, created_at, updated_at
                 FROM transfer_jobs WHERE job_id = ?1;",
                params![id.as_str()],
                Self::map_transfer_job_row,
            )
            .optional()
            .map_err(DbError::from)
        })
    }

    /// Lists transfer jobs associated with a file.
    pub fn list_transfer_jobs_by_file(&self, file_id: &FileId) -> Result<Vec<TransferJobRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT job_id, file_id, chunk_id, direction, status, progress,
                        retry_count, error_message, created_at, updated_at
                 FROM transfer_jobs WHERE file_id = ?1 ORDER BY created_at ASC;",
            )?;
            let rows = stmt.query_map(params![file_id.as_str()], Self::map_transfer_job_row)?;
            let mut jobs = Vec::new();
            for r in rows {
                jobs.push(r?);
            }
            Ok(jobs)
        })
    }

    /// Lists transfer jobs filtered by status.
    pub fn list_transfer_jobs_by_status(
        &self,
        status: TransferStatus,
    ) -> Result<Vec<TransferJobRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT job_id, file_id, chunk_id, direction, status, progress,
                        retry_count, error_message, created_at, updated_at
                 FROM transfer_jobs WHERE status = ?1 ORDER BY created_at ASC;",
            )?;
            let rows = stmt.query_map(params![status.to_string()], Self::map_transfer_job_row)?;
            let mut jobs = Vec::new();
            for r in rows {
                jobs.push(r?);
            }
            Ok(jobs)
        })
    }

    /// Lists the most recent transfer jobs up to `limit`.
    pub fn list_recent_transfer_jobs(&self, limit: usize) -> Result<Vec<TransferJobRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT job_id, file_id, chunk_id, direction, status, progress,
                        retry_count, error_message, created_at, updated_at
                 FROM transfer_jobs ORDER BY created_at DESC LIMIT ?1;",
            )?;
            let rows = stmt.query_map(params![limit as i64], Self::map_transfer_job_row)?;
            let mut jobs = Vec::new();
            for r in rows {
                jobs.push(r?);
            }
            Ok(jobs)
        })
    }

    /// Updates status and error information of a transfer job.
    pub fn update_transfer_job_status(
        &self,
        id: &JobId,
        status: TransferStatus,
        error: Option<&str>,
        updated_at: &str,
    ) -> Result<()> {
        self.with_connection(|conn| {
            let affected = conn.execute(
                "UPDATE transfer_jobs
                 SET status = ?2, error_message = ?3, updated_at = ?4
                 WHERE job_id = ?1;",
                params![id.as_str(), status.to_string(), error, updated_at],
            )?;
            if affected == 0 {
                return Err(DbError::NotFound {
                    entity: "TransferJob",
                    id: id.to_string(),
                });
            }
            Ok(())
        })
    }

    /// Updates byte/unit progress of a transfer job.
    pub fn update_transfer_job_progress(
        &self,
        id: &JobId,
        progress: u64,
        updated_at: &str,
    ) -> Result<()> {
        self.with_connection(|conn| {
            let affected = conn.execute(
                "UPDATE transfer_jobs
                 SET progress = ?2, updated_at = ?3
                 WHERE job_id = ?1;",
                params![id.as_str(), progress as i64, updated_at],
            )?;
            if affected == 0 {
                return Err(DbError::NotFound {
                    entity: "TransferJob",
                    id: id.to_string(),
                });
            }
            Ok(())
        })
    }

    /// Increments retry counter of a transfer job.
    pub fn increment_transfer_job_retry(&self, id: &JobId, updated_at: &str) -> Result<()> {
        self.with_connection(|conn| {
            let affected = conn.execute(
                "UPDATE transfer_jobs
                 SET retry_count = retry_count + 1, updated_at = ?2
                 WHERE job_id = ?1;",
                params![id.as_str(), updated_at],
            )?;
            if affected == 0 {
                return Err(DbError::NotFound {
                    entity: "TransferJob",
                    id: id.to_string(),
                });
            }
            Ok(())
        })
    }

    /// Helper for mapping a transfer job row to [`TransferJobRecord`].
    fn map_transfer_job_row(r: &rusqlite::Row) -> rusqlite::Result<TransferJobRecord> {
        let jid_str: String = r.get(0)?;
        let fid_str: String = r.get(1)?;
        let cid_opt: Option<String> = r.get(2)?;
        let dir_str: String = r.get(3)?;
        let status_str: String = r.get(4)?;
        let prog_i64: i64 = r.get(5)?;
        let retries_i64: i64 = r.get(6)?;
        let err: Option<String> = r.get(7)?;
        let cat: String = r.get(8)?;
        let uat: String = r.get(9)?;

        let jid = JobId::new(jid_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let fid = FileId::new(fid_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let cid = match cid_opt {
            Some(s) => Some(ChunkId::new(s).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?),
            None => None,
        };

        let direction = match dir_str.as_str() {
            "upload" => TransferDirection::Upload,
            "download" => TransferDirection::Download,
            other => {
                return Err(rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    format!("unknown TransferDirection: '{other}'").into(),
                ));
            }
        };

        let status = match status_str.as_str() {
            "pending" => TransferStatus::Pending,
            "transferring" => TransferStatus::Transferring,
            "paused" => TransferStatus::Paused,
            "completed" => TransferStatus::Completed,
            "failed" => TransferStatus::Failed,
            "cancelled" => TransferStatus::Cancelled,
            other => {
                return Err(rusqlite::Error::FromSqlConversionFailure(
                    4,
                    rusqlite::types::Type::Text,
                    format!("unknown TransferStatus: '{other}'").into(),
                ));
            }
        };

        Ok(TransferJobRecord {
            job_id: jid,
            file_id: fid,
            chunk_id: cid,
            direction,
            status,
            progress: prog_i64 as u64,
            retry_count: retries_i64 as u32,
            error_message: err,
            created_at: cat,
            updated_at: uat,
        })
    }

    // =========================================================================
    // FTS5 Full-Text Search
    // =========================================================================

    /// Performs full-text search across filenames and relative paths using FTS5.
    pub fn search_files(&self, query: &str) -> Result<Vec<SearchResult>> {
        let sanitized = sanitize_fts5_query(query);
        if sanitized.is_empty() {
            return Ok(Vec::new());
        }

        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT file_id, file_name, relative_path
                 FROM files_fts
                 WHERE files_fts MATCH ?1
                 ORDER BY rank;",
            )?;

            let rows = stmt.query_map(params![sanitized], |r| {
                let fid_str: String = r.get(0)?;
                let fname: String = r.get(1)?;
                let rpath: String = r.get(2)?;
                let fid = FileId::new(fid_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
                Ok(SearchResult {
                    file_id: fid,
                    file_name: fname,
                    relative_path: rpath,
                })
            })?;

            let mut results = Vec::new();
            for r in rows {
                results.push(r?);
            }
            Ok(results)
        })
    }

    // =========================================================================
    // Recurring Schedules & Execution History
    // =========================================================================

    /// Inserts a new recurring backup schedule.
    pub fn create_schedule(&self, schedule: &ScheduleRecord) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO schedules (
                    schedule_id, profile_id, schedule_type, expression, timezone,
                    enabled, next_run_at, last_run_at, last_status, last_error_code,
                    created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12);",
                params![
                    schedule.schedule_id.as_str(),
                    schedule.profile_id.as_str(),
                    schedule.schedule_type,
                    schedule.expression,
                    schedule.timezone,
                    if schedule.enabled { 1 } else { 0 },
                    schedule.next_run_at,
                    schedule.last_run_at,
                    schedule.last_status,
                    schedule.last_error_code,
                    schedule.created_at,
                    schedule.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    /// Retrieves a schedule by its unique ID.
    pub fn get_schedule(&self, id: &ScheduleId) -> Result<Option<ScheduleRecord>> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT schedule_id, profile_id, schedule_type, expression, timezone,
                        enabled, next_run_at, last_run_at, last_status, last_error_code,
                        created_at, updated_at
                 FROM schedules WHERE schedule_id = ?1;",
                params![id.as_str()],
                map_schedule_row,
            )
            .optional()
            .map_err(DbError::from)
        })
    }

    /// Lists all configured backup schedules.
    pub fn list_schedules(&self) -> Result<Vec<ScheduleRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT schedule_id, profile_id, schedule_type, expression, timezone,
                        enabled, next_run_at, last_run_at, last_status, last_error_code,
                        created_at, updated_at
                 FROM schedules ORDER BY created_at ASC;",
            )?;
            let rows = stmt.query_map([], map_schedule_row)?;
            let mut schedules = Vec::new();
            for r in rows {
                schedules.push(r?);
            }
            Ok(schedules)
        })
    }

    /// Lists all schedules configured for a specific profile.
    pub fn list_schedules_for_profile(
        &self,
        profile_id: &ProfileId,
    ) -> Result<Vec<ScheduleRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT schedule_id, profile_id, schedule_type, expression, timezone,
                        enabled, next_run_at, last_run_at, last_status, last_error_code,
                        created_at, updated_at
                 FROM schedules WHERE profile_id = ?1 ORDER BY created_at ASC;",
            )?;
            let rows = stmt.query_map(params![profile_id.as_str()], map_schedule_row)?;
            let mut schedules = Vec::new();
            for r in rows {
                schedules.push(r?);
            }
            Ok(schedules)
        })
    }

    /// Lists all enabled schedules that are due to execute at or before `max_timestamp`.
    pub fn list_due_schedules(&self, max_timestamp: &str) -> Result<Vec<ScheduleRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT schedule_id, profile_id, schedule_type, expression, timezone,
                        enabled, next_run_at, last_run_at, last_status, last_error_code,
                        created_at, updated_at
                 FROM schedules
                 WHERE enabled = 1 AND next_run_at IS NOT NULL AND next_run_at <= ?1
                 ORDER BY next_run_at ASC;",
            )?;
            let rows = stmt.query_map(params![max_timestamp], map_schedule_row)?;
            let mut schedules = Vec::new();
            for r in rows {
                schedules.push(r?);
            }
            Ok(schedules)
        })
    }

    /// Updates an existing schedule's configuration and properties.
    pub fn update_schedule(&self, schedule: &ScheduleRecord) -> Result<()> {
        self.with_connection(|conn| {
            let rows_affected = conn.execute(
                "UPDATE schedules
                 SET profile_id = ?2, schedule_type = ?3, expression = ?4, timezone = ?5,
                     enabled = ?6, next_run_at = ?7, last_run_at = ?8, last_status = ?9,
                     last_error_code = ?10, updated_at = ?11
                 WHERE schedule_id = ?1;",
                params![
                    schedule.schedule_id.as_str(),
                    schedule.profile_id.as_str(),
                    schedule.schedule_type,
                    schedule.expression,
                    schedule.timezone,
                    if schedule.enabled { 1 } else { 0 },
                    schedule.next_run_at,
                    schedule.last_run_at,
                    schedule.last_status,
                    schedule.last_error_code,
                    schedule.updated_at,
                ],
            )?;
            if rows_affected == 0 {
                return Err(DbError::NotFound {
                    entity: "Schedule",
                    id: schedule.schedule_id.to_string(),
                });
            }
            Ok(())
        })
    }

    /// Updates execution status and timing for a schedule.
    pub fn update_schedule_status(
        &self,
        id: &ScheduleId,
        next_run_at: Option<&str>,
        last_run_at: Option<&str>,
        last_status: Option<&str>,
        last_error_code: Option<&str>,
    ) -> Result<()> {
        self.with_connection(|conn| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .to_string();
            let rows_affected = conn.execute(
                "UPDATE schedules
                 SET next_run_at = ?2, last_run_at = ?3, last_status = ?4,
                     last_error_code = ?5, updated_at = ?6
                 WHERE schedule_id = ?1;",
                params![
                    id.as_str(),
                    next_run_at,
                    last_run_at,
                    last_status,
                    last_error_code,
                    now
                ],
            )?;
            if rows_affected == 0 {
                return Err(DbError::NotFound {
                    entity: "Schedule",
                    id: id.to_string(),
                });
            }
            Ok(())
        })
    }

    /// Deletes a schedule by its unique ID. Returns true if a record was removed.
    pub fn delete_schedule(&self, id: &ScheduleId) -> Result<bool> {
        self.with_connection(|conn| {
            let affected = conn.execute(
                "DELETE FROM schedules WHERE schedule_id = ?1;",
                params![id.as_str()],
            )?;
            Ok(affected > 0)
        })
    }

    /// Records an execution history entry for a schedule run.
    pub fn record_schedule_history(&self, record: &ScheduleHistoryRecord) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO schedule_history (
                    history_id, schedule_id, profile_id, started_at, completed_at,
                    status, snapshot_id, files_processed, bytes_transferred,
                    error_code, error_message
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11);",
                params![
                    record.history_id,
                    record.schedule_id.as_str(),
                    record.profile_id.as_str(),
                    record.started_at,
                    record.completed_at,
                    record.status,
                    record.snapshot_id.as_ref().map(|s| s.as_str()),
                    record.files_processed as i64,
                    record.bytes_transferred as i64,
                    record.error_code,
                    record.error_message,
                ],
            )?;
            Ok(())
        })
    }

    /// Lists execution history for a schedule, ordered by descending start time.
    pub fn list_schedule_history(
        &self,
        schedule_id: &ScheduleId,
        limit: usize,
    ) -> Result<Vec<ScheduleHistoryRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT history_id, schedule_id, profile_id, started_at, completed_at,
                        status, snapshot_id, files_processed, bytes_transferred,
                        error_code, error_message
                 FROM schedule_history
                 WHERE schedule_id = ?1
                 ORDER BY started_at DESC
                 LIMIT ?2;",
            )?;
            let rows = stmt.query_map(
                params![schedule_id.as_str(), limit as i64],
                map_schedule_history_row,
            )?;
            let mut history = Vec::new();
            for r in rows {
                history.push(r?);
            }
            Ok(history)
        })
    }
}

fn map_schedule_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ScheduleRecord> {
    let sid_str: String = r.get(0)?;
    let pid_str: String = r.get(1)?;
    let schedule_type: String = r.get(2)?;
    let expression: String = r.get(3)?;
    let timezone: String = r.get(4)?;
    let enabled_num: i64 = r.get(5)?;
    let next_run_at: Option<String> = r.get(6)?;
    let last_run_at: Option<String> = r.get(7)?;
    let last_status: Option<String> = r.get(8)?;
    let last_error_code: Option<String> = r.get(9)?;
    let created_at: String = r.get(10)?;
    let updated_at: String = r.get(11)?;

    let schedule_id = ScheduleId::new(sid_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let profile_id = ProfileId::new(pid_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
    })?;

    Ok(ScheduleRecord {
        schedule_id,
        profile_id,
        schedule_type,
        expression,
        timezone,
        enabled: enabled_num != 0,
        next_run_at,
        last_run_at,
        last_status,
        last_error_code,
        created_at,
        updated_at,
    })
}

fn map_schedule_history_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ScheduleHistoryRecord> {
    let history_id: String = r.get(0)?;
    let sid_str: String = r.get(1)?;
    let pid_str: String = r.get(2)?;
    let started_at: String = r.get(3)?;
    let completed_at: Option<String> = r.get(4)?;
    let status: String = r.get(5)?;
    let snap_str: Option<String> = r.get(6)?;
    let files_processed: i64 = r.get(7)?;
    let bytes_transferred: i64 = r.get(8)?;
    let error_code: Option<String> = r.get(9)?;
    let error_message: Option<String> = r.get(10)?;

    let schedule_id = ScheduleId::new(sid_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let profile_id = ProfileId::new(pid_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let snapshot_id = if let Some(s) = snap_str {
        Some(SnapshotId::new(s).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e))
        })?)
    } else {
        None
    };

    Ok(ScheduleHistoryRecord {
        history_id,
        schedule_id,
        profile_id,
        started_at,
        completed_at,
        status,
        snapshot_id,
        files_processed: files_processed as u64,
        bytes_transferred: bytes_transferred as u64,
        error_code,
        error_message,
    })
}

/// Sanitizes user queries for safe execution inside SQLite FTS5 MATCH expressions.
///
/// Strips characters that have syntactic meaning in FTS5 boolean grammar unless quoted.
fn sanitize_fts5_query(query: &str) -> String {
    let mut tokens = Vec::new();
    for word in query.split_whitespace() {
        let clean: String = word
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
            .collect();
        if !clean.is_empty() {
            // Suffix with wildcard for prefix matching (e.g., "sec" matches "secret")
            tokens.push(format!("\"{clean}\"*"));
        }
    }
    tokens.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_fts5_query() {
        assert_eq!(sanitize_fts5_query("hello world"), "\"hello\"* \"world\"*");
        assert_eq!(
            sanitize_fts5_query("secret*^ OR (DROP)"),
            "\"secret\"* \"OR\"* \"DROP\"*"
        );
        assert_eq!(sanitize_fts5_query("   "), "");
    }
}
