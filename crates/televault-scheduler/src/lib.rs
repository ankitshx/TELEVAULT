//! Recurring backup scheduling, next-run calculations, and missed-run handling for TELEVAULT.

#![deny(missing_docs)]

pub use televault_core as core;

/// Returns the scheduler subsystem state.
pub fn scheduler_state() -> &'static str {
    "stopped"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_state() {
        assert_eq!(scheduler_state(), "stopped");
    }
}
