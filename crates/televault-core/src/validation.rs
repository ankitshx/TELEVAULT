//! Validation rules and invariants for domain data.

use crate::error::{AppError, Result};
use std::path::Path;

/// Ensures a string slice is non-empty after trimming whitespace.
pub fn validate_non_empty(field: &'static str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(AppError::validation(field, "value cannot be empty"));
    }
    Ok(())
}

/// Validates that an identifier string matches allowed character patterns:
/// alphanumeric plus hyphens and underscores, between 1 and 64 characters.
pub fn validate_identifier_string(entity: &'static str, value: &str) -> Result<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidId {
            entity,
            reason: "identifier cannot be empty".into(),
        });
    }
    if trimmed.len() > 64 {
        return Err(AppError::InvalidId {
            entity,
            reason: format!(
                "identifier exceeds maximum length of 64 characters (length: {})",
                trimmed.len()
            ),
        });
    }
    for ch in trimmed.chars() {
        if !ch.is_ascii_alphanumeric() && ch != '-' && ch != '_' {
            return Err(AppError::InvalidId {
                entity,
                reason: format!(
                    "identifier contains illegal character '{ch}'; only alphanumeric, '-', and '_' are permitted"
                ),
            });
        }
    }
    Ok(())
}

/// Validates that a path is relative and safe from path traversal attacks.
pub fn validate_safe_relative_path(path: &Path) -> Result<()> {
    if path.is_absolute() {
        return Err(AppError::Path("path must be relative, not absolute".into()));
    }
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                return Err(AppError::Path(
                    "path traversal ('..') is strictly prohibited".into(),
                ));
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err(AppError::Path(
                    "absolute prefixes or root elements are not permitted in relative paths".into(),
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_non_empty() {
        assert!(validate_non_empty("name", "valid-name").is_ok());
        assert!(validate_non_empty("name", "   ").is_err());
        assert!(validate_non_empty("name", "").is_err());
    }

    #[test]
    fn test_validate_identifier_string() {
        assert!(validate_identifier_string("Profile", "profile-01_a").is_ok());
        assert!(validate_identifier_string("Profile", "abc123XYZ").is_ok());

        // Empty
        assert!(validate_identifier_string("Profile", "").is_err());
        assert!(validate_identifier_string("Profile", "   ").is_err());

        // Illegal characters
        assert!(validate_identifier_string("Profile", "prof/01").is_err());
        assert!(validate_identifier_string("Profile", "prof\\01").is_err());
        assert!(validate_identifier_string("Profile", "prof.01").is_err());
        assert!(validate_identifier_string("Profile", "prof 01").is_err());
        assert!(validate_identifier_string("Profile", "prof@01").is_err());

        // Max length boundary
        let exact_64 = "a".repeat(64);
        assert!(validate_identifier_string("Profile", &exact_64).is_ok());

        let too_long = "a".repeat(65);
        assert!(validate_identifier_string("Profile", &too_long).is_err());
    }

    #[test]
    fn test_validate_safe_relative_path() {
        assert!(validate_safe_relative_path(Path::new("sub/folder/file.txt")).is_ok());
        assert!(validate_safe_relative_path(Path::new("file.txt")).is_ok());
        assert!(validate_safe_relative_path(Path::new("a/b/c/d")).is_ok());

        // Traversal attempts
        assert!(validate_safe_relative_path(Path::new("../escape.txt")).is_err());
        assert!(validate_safe_relative_path(Path::new("sub/../../escape.txt")).is_err());
        assert!(validate_safe_relative_path(Path::new("sub/../folder")).is_err());

        // Absolute paths
        #[cfg(windows)]
        {
            assert!(validate_safe_relative_path(Path::new("C:\\Windows\\System32")).is_err());
            assert!(validate_safe_relative_path(Path::new("\\Windows")).is_err());
        }
        #[cfg(not(windows))]
        {
            assert!(validate_safe_relative_path(Path::new("/etc/passwd")).is_err());
        }
    }
}
