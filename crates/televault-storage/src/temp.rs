//! Temporary payload lifecycle management and cleanup policies for staging buffers.

use crate::error::{Result, StorageError};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use televault_core::paths::PathManager;

/// Managed temporary payload file with RAII cleanup semantics.
///
/// Ensures temporary upload or download buffers are never leaked on disk
/// if a transfer is cancelled, fails, or panics.
#[derive(Debug)]
pub struct TempPayloadFile {
    path: PathBuf,
    active: bool,
}

impl TempPayloadFile {
    /// Creates a new [`TempPayloadFile`] tracking an active file on disk.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            active: true,
        }
    }

    /// Returns the filesystem path of the temporary staging payload.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Explicitly removes the temporary file from disk upon successful remote completion.
    pub fn cleanup(mut self) -> Result<()> {
        self.active = false;
        if self.path.exists() {
            fs::remove_file(&self.path).map_err(|e| {
                StorageError::TempStorage(format!(
                    "Failed to remove staging file '{}': {e}",
                    self.path.display()
                ))
            })?;
        }
        Ok(())
    }

    /// Disables RAII automatic deletion if the file is intentionally handed over.
    pub fn disown(mut self) -> PathBuf {
        self.active = false;
        self.path.clone()
    }
}

impl Drop for TempPayloadFile {
    fn drop(&mut self) {
        if self.active && self.path.exists() {
            // Defensive cleanup on drop/panic/early return
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Manager enforcing TELEVAULT temporary storage policy.
///
/// Rules:
/// - Staging files strictly reside within `PathManager::temp_dir()`.
/// - Temporary files are transient and must NEVER be treated as permanent backups.
/// - Interrupted or crashed operations leave stale files that are purged upon startup.
#[derive(Debug, Clone)]
pub struct TempPayloadManager {
    temp_dir: PathBuf,
}

impl TempPayloadManager {
    /// Creates a new [`TempPayloadManager`] bound to the application's temporary directory.
    pub fn new(temp_dir: impl Into<PathBuf>) -> Self {
        Self {
            temp_dir: temp_dir.into(),
        }
    }

    /// Creates a manager configured directly from a [`PathManager`].
    pub fn from_path_manager(paths: &PathManager) -> Self {
        Self::new(paths.temp_dir())
    }

    /// Returns the root temporary directory path.
    pub fn temp_dir(&self) -> &Path {
        &self.temp_dir
    }

    /// Ensures the staging directory exists on disk.
    pub fn ensure_temp_dir(&self) -> Result<()> {
        if !self.temp_dir.exists() {
            fs::create_dir_all(&self.temp_dir).map_err(|e| {
                StorageError::TempStorage(format!(
                    "Failed to create temp directory '{}': {e}",
                    self.temp_dir.display()
                ))
            })?;
        }
        Ok(())
    }

    /// Allocates and creates a new empty temporary payload staging file.
    pub fn create_staging_file(&self, prefix: &str) -> Result<TempPayloadFile> {
        self.ensure_temp_dir()?;
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let filename = format!("stage_{prefix}_{timestamp}_{}.tmp", std::process::id());
        let file_path = self.temp_dir.join(filename);

        // Ensure file is created
        File::create(&file_path).map_err(|e| {
            StorageError::TempStorage(format!(
                "Failed to create staging file '{}': {e}",
                file_path.display()
            ))
        })?;

        Ok(TempPayloadFile::new(file_path))
    }

    /// Scans the temporary staging directory and purges stale orphaned files.
    ///
    /// Intended for application startup or post-crash recovery to reclaim disk space.
    pub fn cleanup_stale_staging_files(&self, max_age: Duration) -> Result<usize> {
        if !self.temp_dir.exists() {
            return Ok(0);
        }

        let now = SystemTime::now();
        let mut purged_count = 0;

        let entries = fs::read_dir(&self.temp_dir).map_err(|e| {
            StorageError::TempStorage(format!(
                "Failed to read temp directory '{}': {e}",
                self.temp_dir.display()
            ))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(age) = now.duration_since(modified) {
                            if age >= max_age && fs::remove_file(&path).is_ok() {
                                purged_count += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(purged_count)
    }

    /// Calculates total disk bytes currently occupied by staging files in the temp area.
    pub fn total_staged_bytes(&self) -> Result<u64> {
        if !self.temp_dir.exists() {
            return Ok(0);
        }

        let mut total = 0;
        let entries = fs::read_dir(&self.temp_dir).map_err(|e| {
            StorageError::TempStorage(format!(
                "Failed to read temp directory '{}': {e}",
                self.temp_dir.display()
            ))
        })?;

        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    total += meta.len();
                }
            }
        }

        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_temp_payload_file_explicit_cleanup() {
        let temp_dir = std::env::temp_dir().join("televault_test_temp_explicit");
        let manager = TempPayloadManager::new(&temp_dir);

        let temp_file = manager.create_staging_file("upload").expect("create file");
        let path = temp_file.path().to_path_buf();
        assert!(path.exists());

        // Write some bytes
        {
            let mut f = File::create(&path).unwrap();
            f.write_all(b"temporary staged payload").unwrap();
        }
        assert!(path.exists());

        // Explicit cleanup
        temp_file.cleanup().expect("cleanup");
        assert!(!path.exists(), "File must be deleted after cleanup");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_temp_payload_file_raii_drop_cleanup() {
        let temp_dir = std::env::temp_dir().join("televault_test_temp_drop");
        let manager = TempPayloadManager::new(&temp_dir);

        let path = {
            let temp_file = manager.create_staging_file("drop").expect("create file");
            let p = temp_file.path().to_path_buf();
            assert!(p.exists());
            p
            // temp_file dropped here without calling .cleanup()
        };

        assert!(!path.exists(), "File must be automatically deleted on drop");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_cleanup_stale_staging_files() {
        let temp_dir = std::env::temp_dir().join("televault_test_temp_stale");
        let manager = TempPayloadManager::new(&temp_dir);

        let file = manager.create_staging_file("stale").expect("create");
        let path = file.disown(); // Keep it on disk

        assert!(path.exists());
        // Clean with 0s max_age will purge it immediately
        std::thread::sleep(std::time::Duration::from_millis(50));
        let purged = manager
            .cleanup_stale_staging_files(Duration::from_millis(10))
            .expect("purge");
        assert_eq!(purged, 1);
        assert!(!path.exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
