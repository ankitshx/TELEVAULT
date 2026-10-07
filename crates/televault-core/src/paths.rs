//! PathManager foundation for standardized filesystem access across platforms.

use std::path::{Path, PathBuf};

/// Centralized manager for application paths, preventing hardcoded developer paths.
#[derive(Debug, Clone)]
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

    /// Returns the base directory.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Returns the data storage directory.
    pub fn data_dir(&self) -> PathBuf {
        self.base_dir.join("data")
    }

    /// Returns the cache directory.
    pub fn cache_dir(&self) -> PathBuf {
        self.base_dir.join("cache")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_manager_paths() {
        let pm = PathManager::new("/test/vault");
        assert_eq!(pm.base_dir(), Path::new("/test/vault"));
        assert_eq!(pm.data_dir(), Path::new("/test/vault/data"));
        assert_eq!(pm.cache_dir(), Path::new("/test/vault/cache"));
    }
}
