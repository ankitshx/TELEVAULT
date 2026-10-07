//! Deterministic destination collision handling policies and resolution.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Policy governing behavior when a restored file destination already exists on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CollisionPolicy {
    /// Overwrite the existing file with the restored file.
    #[default]
    Overwrite,
    /// Skip restoring the file; leave the existing file untouched.
    Skip,
    /// Keep both files by writing the restored content to a deterministic alternate name.
    ///
    /// For example: `document.txt` -> `document (1).txt` -> `document (2).txt`.
    KeepBoth,
}

/// Result of evaluating a collision against local filesystem state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollisionResolution {
    /// Proceed with restore writing to the specified target path.
    Apply(PathBuf),
    /// Skip restore without modifying disk.
    Skip,
}

/// Resolves a collision policy against a target destination path.
///
/// If the file does not exist, [`CollisionResolution::Apply(target_path)`] is returned immediately.
/// If the file exists, the policy determines whether to overwrite, skip, or generate a deterministic alternate.
pub fn resolve_collision(target_path: &Path, policy: CollisionPolicy) -> CollisionResolution {
    if !target_path.exists() {
        return CollisionResolution::Apply(target_path.to_path_buf());
    }

    match policy {
        CollisionPolicy::Overwrite => CollisionResolution::Apply(target_path.to_path_buf()),
        CollisionPolicy::Skip => CollisionResolution::Skip,
        CollisionPolicy::KeepBoth => {
            let alternate = generate_alternate_path(target_path);
            CollisionResolution::Apply(alternate)
        }
    }
}

/// Generates a deterministic safe alternate filename when `target_path` already exists.
///
/// Follows standard desktop convention:
/// - `report.pdf` -> `report (1).pdf` -> `report (2).pdf`
/// - `archive` (no ext) -> `archive (1)` -> `archive (2)`
pub fn generate_alternate_path(target_path: &Path) -> PathBuf {
    let parent = target_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = target_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("restored");
    let ext = target_path.extension().and_then(|e| e.to_str());

    for i in 1..=10_000 {
        let candidate_filename = match ext {
            Some(e) => format!("{stem} ({i}).{e}"),
            None => format!("{stem} ({i})"),
        };
        let candidate = parent.join(candidate_filename);
        if !candidate.exists() {
            return candidate;
        }
    }

    // Fallback if 10,000 collisions exist: nanosecond timestamp
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let fallback_filename = match ext {
        Some(e) => format!("{stem} (collision_{ts}).{e}"),
        None => format!("{stem} (collision_{ts})"),
    };
    parent.join(fallback_filename)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_collision_non_existent_file() {
        let temp_dir = std::env::temp_dir().join("televault_test_collision_nonexistent");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let target = temp_dir.join("new_file.txt");
        assert_eq!(
            resolve_collision(&target, CollisionPolicy::Overwrite),
            CollisionResolution::Apply(target.clone())
        );
        assert_eq!(
            resolve_collision(&target, CollisionPolicy::Skip),
            CollisionResolution::Apply(target.clone())
        );
        assert_eq!(
            resolve_collision(&target, CollisionPolicy::KeepBoth),
            CollisionResolution::Apply(target)
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_collision_overwrite_and_skip() {
        let temp_dir = std::env::temp_dir().join("televault_test_collision_overwrite_skip");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let target = temp_dir.join("existing.txt");
        fs::write(&target, b"existing content").unwrap();

        assert_eq!(
            resolve_collision(&target, CollisionPolicy::Overwrite),
            CollisionResolution::Apply(target.clone())
        );
        assert_eq!(
            resolve_collision(&target, CollisionPolicy::Skip),
            CollisionResolution::Skip
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_collision_keep_both_sequence() {
        let temp_dir = std::env::temp_dir().join("televault_test_collision_keep_both");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let target = temp_dir.join("document.txt");
        fs::write(&target, b"version 0").unwrap();

        // First collision -> document (1).txt
        let alt1 = generate_alternate_path(&target);
        assert_eq!(alt1, temp_dir.join("document (1).txt"));
        fs::write(&alt1, b"version 1").unwrap();

        // Second collision -> document (2).txt
        let alt2 = generate_alternate_path(&target);
        assert_eq!(alt2, temp_dir.join("document (2).txt"));
        fs::write(&alt2, b"version 2").unwrap();

        // Third collision -> document (3).txt
        let alt3 = generate_alternate_path(&target);
        assert_eq!(alt3, temp_dir.join("document (3).txt"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_collision_keep_both_no_extension() {
        let temp_dir = std::env::temp_dir().join("televault_test_collision_no_ext");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let target = temp_dir.join("binary_file");
        fs::write(&target, b"data").unwrap();

        let alt1 = generate_alternate_path(&target);
        assert_eq!(alt1, temp_dir.join("binary_file (1)"));

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
