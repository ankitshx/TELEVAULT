//! Backup orchestration, snapshot management, and retention policies for TELEVAULT.

#![deny(missing_docs)]

pub use televault_core as core;
pub use televault_manifest as manifest;

/// Returns the backup engine status string.
pub fn backup_engine_status() -> &'static str {
    "idle"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_engine_status() {
        assert_eq!(backup_engine_status(), "idle");
    }
}
