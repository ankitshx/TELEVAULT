//! Transfer queue, chunking pipelines, and background worker interfaces for TELEVAULT.

#![deny(missing_docs)]

pub use televault_core as core;
pub use televault_storage as storage;

/// Returns the transfer subsystem status string.
pub fn transfer_status() -> &'static str {
    "ready"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer_status() {
        assert_eq!(transfer_status(), "ready");
    }
}
