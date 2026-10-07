//! Recurring backup scheduling, next-run calculations, and automated execution coordination for TELEVAULT.

#![deny(missing_docs)]

pub mod clock;
pub mod error;
pub mod expression;
pub mod guard;
pub mod service;
pub mod types;

pub use clock::{Clock, MockClock, SystemClock};
pub use error::{Result, SchedulerError};
pub use expression::{calculate_next_run, validate_expression};
pub use guard::{ExecutionGuard, ProfileGuard};
pub use service::SchedulerService;
pub use types::*;

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
