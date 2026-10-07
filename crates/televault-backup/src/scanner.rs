//! Safe and deterministic filesystem discovery and directory scanner.

use crate::error::{BackupError, Result};
use crate::profile::BackupProfile;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Discovered logical file metadata on the local filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedFile {
    /// Relative path within the backup root (forward-slash normalized).
    pub relative_path: String,
    /// Absolute path on the host filesystem.
    pub absolute_path: PathBuf,
    /// File base name.
    pub file_name: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Last modified timestamp in milliseconds since UNIX epoch.
    pub modified_at_epoch_ms: u64,
    /// Formatted modified timestamp string.
    pub modified_at: String,
    /// Whether the file is currently accessible for reading.
    pub is_accessible: bool,
}

/// Discovered filesystem scan results with graceful error logging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanResult {
    /// Discovered files matching filter criteria, sorted deterministically by relative path.
    pub files: Vec<ScannedFile>,
    /// Inaccessible files or directories encountered during scan with error reasons.
    pub inaccessible: Vec<(PathBuf, String)>,
    /// Total aggregate size of all accessible scanned files in bytes.
    pub total_bytes: u64,
    /// Total count of accessible scanned files.
    pub total_count: usize,
}

/// Filesystem scanner with path traversal guards and deterministic sorting.
pub struct FileScanner;

impl FileScanner {
    /// Scans a directory rooted at `source_root` matching rules from `profile`.
    pub fn scan_directory(source_root: &Path, profile: &BackupProfile) -> Result<ScanResult> {
        if !source_root.exists() {
            return Err(BackupError::ScanError(format!(
                "Source directory does not exist: {}",
                source_root.display()
            )));
        }

        if !source_root.is_dir() {
            return Err(BackupError::ScanError(format!(
                "Source path is not a directory: {}",
                source_root.display()
            )));
        }

        let mut files = Vec::new();
        let mut inaccessible = Vec::new();

        Self::crawl_dir(
            source_root,
            source_root,
            profile,
            &mut files,
            &mut inaccessible,
        )?;

        // Enforce deterministic sorting by relative path
        files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

        let total_bytes = files.iter().map(|f| f.size_bytes).sum();
        let total_count = files.len();

        Ok(ScanResult {
            files,
            inaccessible,
            total_bytes,
            total_count,
        })
    }

    /// Recursive directory crawler handling permission errors gracefully.
    fn crawl_dir(
        current_dir: &Path,
        root: &Path,
        profile: &BackupProfile,
        files: &mut Vec<ScannedFile>,
        inaccessible: &mut Vec<(PathBuf, String)>,
    ) -> Result<()> {
        let entries = match fs::read_dir(current_dir) {
            Ok(iter) => iter,
            Err(e) => {
                inaccessible.push((current_dir.to_path_buf(), e.to_string()));
                return Ok(());
            }
        };

        for entry_res in entries {
            let entry = match entry_res {
                Ok(e) => e,
                Err(e) => {
                    inaccessible.push((current_dir.to_path_buf(), e.to_string()));
                    continue;
                }
            };

            let path = entry.path();
            let metadata = match fs::metadata(&path) {
                Ok(m) => m,
                Err(e) => {
                    inaccessible.push((path, e.to_string()));
                    continue;
                }
            };

            if metadata.is_dir() {
                Self::crawl_dir(&path, root, profile, files, inaccessible)?;
            } else if metadata.is_file() {
                let relative = match path.strip_prefix(root) {
                    Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
                    Err(e) => {
                        inaccessible.push((path, e.to_string()));
                        continue;
                    }
                };

                // Path traversal check
                if let Err(e) =
                    televault_core::validation::validate_safe_relative_path(Path::new(&relative))
                {
                    inaccessible.push((path, format!("Unsafe relative path: {e}")));
                    continue;
                }

                // Check profile filter rules
                if !profile.is_path_included(&relative) {
                    continue;
                }

                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| relative.clone());

                let size_bytes = metadata.len();

                let (epoch_ms, formatted_ts) = metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| {
                        let ms = d.as_millis() as u64;
                        (ms, format!("epoch-ms:{}", ms))
                    })
                    .unwrap_or((0, "1970-01-01T00:00:00Z".into()));

                // Verify file read accessibility
                let is_accessible = fs::File::open(&path).is_ok();

                files.push(ScannedFile {
                    relative_path: relative,
                    absolute_path: path,
                    file_name,
                    size_bytes,
                    modified_at_epoch_ms: epoch_ms,
                    modified_at: formatted_ts,
                    is_accessible,
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use televault_core::ids::ProfileId;

    #[test]
    fn test_file_scanner_discovery_and_determinism() {
        let temp_dir = std::env::temp_dir().join("televault_test_scanner");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("sub")).unwrap();

        fs::write(temp_dir.join("b_file.txt"), b"File B content").unwrap();
        fs::write(temp_dir.join("a_file.txt"), b"File A").unwrap();
        fs::write(temp_dir.join("sub").join("nested.pdf"), b"PDF bytes").unwrap();
        fs::write(temp_dir.join("ignored.tmp"), b"Temp file").unwrap();

        let profile = BackupProfile::new(
            ProfileId::new("p-scan").unwrap(),
            "Scanner Test",
            temp_dir.clone(),
        )
        .with_exclude_patterns(vec!["*.tmp".into()]);

        let res = FileScanner::scan_directory(&temp_dir, &profile).unwrap();

        assert_eq!(res.files.len(), 3);
        // Deterministic sort: a_file.txt, b_file.txt, sub/nested.pdf
        assert_eq!(res.files[0].relative_path, "a_file.txt");
        assert_eq!(res.files[1].relative_path, "b_file.txt");
        assert_eq!(res.files[2].relative_path, "sub/nested.pdf");
        assert_eq!(res.total_bytes, 6 + 14 + 9);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
