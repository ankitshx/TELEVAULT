//! Centralized PathManager for standardized application filesystem locations.

use crate::error::Result;
use crate::validation::validate_safe_relative_path;
use std::path::{Path, PathBuf};

/// Centralized manager for application paths, preventing hardcoded developer paths.
///
/// Categorizes storage locations into isolated, deterministic directories:
/// - Data: Persistent application payloads and local mirrors
/// - Config: Non-sensitive configuration files
/// - Database: Embedded SQLite catalog
/// - Cache: Ephemeral cache files
/// - Logs: Diagnostic logs (sanitized)
/// - Temp: Transient staging and scratch space
/// - Backups: Local backup artifacts and staging
/// - Recovery: Disaster recovery manifests and state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathManager {
    base_dir: PathBuf,
}

impl PathManager {
    /// Creates a new [`PathManager`] rooted at the specified base directory.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Resolves the platform-default base directory for TELEVAULT.
    ///
    /// On Windows, this resolves to `%LOCALAPPDATA%\TELEVAULT`.
    /// On other platforms, it uses `$XDG_DATA_HOME/televault` or `~/.local/share/televault`.
    /// Falls back to a `./.televault` directory in the current working directory if user directories cannot be determined.
    pub fn system_default() -> Self {
        #[cfg(target_os = "windows")]
        {
            if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
                return Self::new(PathBuf::from(local_appdata).join("TELEVAULT"));
            }
            if let Ok(userprofile) = std::env::var("USERPROFILE") {
                return Self::new(
                    PathBuf::from(userprofile)
                        .join("AppData")
                        .join("Local")
                        .join("TELEVAULT"),
                );
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
                return Self::new(PathBuf::from(xdg_data).join("televault"));
            }
            if let Ok(home) = std::env::var("HOME") {
                return Self::new(
                    PathBuf::from(home)
                        .join(".local")
                        .join("share")
                        .join("televault"),
                );
            }
        }

        Self::new(PathBuf::from(".televault"))
    }

    /// Returns the application base directory.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Returns the data storage directory.
    pub fn data_dir(&self) -> PathBuf {
        self.base_dir.join("data")
    }

    /// Returns the configuration directory.
    pub fn config_dir(&self) -> PathBuf {
        self.base_dir.join("config")
    }

    /// Returns the configuration file path (`config/televault.json`).
    pub fn config_file(&self) -> PathBuf {
        self.config_dir().join("televault.json")
    }

    /// Returns the database directory.
    pub fn db_dir(&self) -> PathBuf {
        self.base_dir.join("database")
    }

    /// Returns the SQLite database file path (`database/televault.db`).
    pub fn db_file(&self) -> PathBuf {
        self.db_dir().join("televault.db")
    }

    /// Returns the cache directory.
    pub fn cache_dir(&self) -> PathBuf {
        self.base_dir.join("cache")
    }

    /// Returns the diagnostic logs directory.
    pub fn logs_dir(&self) -> PathBuf {
        self.base_dir.join("logs")
    }

    /// Returns the transient temporary staging directory.
    pub fn temp_dir(&self) -> PathBuf {
        self.base_dir.join("temp")
    }

    /// Returns the local backup state directory.
    pub fn backups_dir(&self) -> PathBuf {
        self.base_dir.join("backups")
    }

    /// Returns the disaster recovery local state directory.
    pub fn recovery_dir(&self) -> PathBuf {
        self.base_dir.join("recovery")
    }

    /// Resolves a relative path safely within the base directory, preventing path traversal.
    pub fn resolve_safe_subpath(&self, relative_path: impl AsRef<Path>) -> Result<PathBuf> {
        let rel = relative_path.as_ref();
        validate_safe_relative_path(rel)?;
        Ok(self.base_dir.join(rel))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_manager_categories() {
        let root = Path::new("/custom/televault_root");
        let pm = PathManager::new(root);

        assert_eq!(pm.base_dir(), root);
        assert_eq!(pm.data_dir(), root.join("data"));
        assert_eq!(pm.config_dir(), root.join("config"));
        assert_eq!(pm.config_file(), root.join("config").join("televault.json"));
        assert_eq!(pm.db_dir(), root.join("database"));
        assert_eq!(pm.db_file(), root.join("database").join("televault.db"));
        assert_eq!(pm.cache_dir(), root.join("cache"));
        assert_eq!(pm.logs_dir(), root.join("logs"));
        assert_eq!(pm.temp_dir(), root.join("temp"));
        assert_eq!(pm.backups_dir(), root.join("backups"));
        assert_eq!(pm.recovery_dir(), root.join("recovery"));
    }

    #[test]
    fn test_system_default_does_not_panic() {
        let default_pm = PathManager::system_default();
        assert!(!default_pm.base_dir().as_os_str().is_empty());
        let db_file = default_pm.db_file();
        assert!(db_file.ends_with(Path::new("database").join("televault.db")));
    }

    #[test]
    fn test_resolve_safe_subpath() {
        let pm = PathManager::new("/base/path");

        // Safe relative path
        let resolved = pm.resolve_safe_subpath("data/chunks/chunk_01.enc").unwrap();
        assert_eq!(
            resolved,
            Path::new("/base/path").join("data/chunks/chunk_01.enc")
        );

        // Path traversal must fail
        assert!(pm.resolve_safe_subpath("../outside.key").is_err());
        assert!(pm.resolve_safe_subpath("data/../../outside.key").is_err());
    }
}
