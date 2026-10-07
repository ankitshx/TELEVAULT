//! Time provider abstractions enabling deterministic testing without wall-clock sleep.

use chrono::{DateTime, Local, TimeZone, Utc};
use std::sync::{Arc, RwLock};

/// Abstraction for querying current system or test time.
pub trait Clock: Send + Sync {
    /// Returns the current UTC timestamp.
    fn now_utc(&self) -> DateTime<Utc>;

    /// Returns the current local timestamp.
    fn now_local(&self) -> DateTime<Local> {
        Local::now()
    }
}

/// Standard production clock backed by system time.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl SystemClock {
    /// Creates a new system clock instance.
    pub fn new() -> Self {
        Self
    }
}

impl Clock for SystemClock {
    fn now_utc(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn now_local(&self) -> DateTime<Local> {
        Local::now()
    }
}

/// Mock clock for deterministic tests allowing simulated time advancement.
#[derive(Debug, Clone)]
pub struct MockClock {
    current_utc: Arc<RwLock<DateTime<Utc>>>,
}

impl MockClock {
    /// Creates a mock clock initialized to the specified UTC timestamp.
    pub fn new(initial_utc: DateTime<Utc>) -> Self {
        Self {
            current_utc: Arc::new(RwLock::new(initial_utc)),
        }
    }

    /// Sets the mock clock to a specific UTC timestamp.
    pub fn set(&self, new_utc: DateTime<Utc>) {
        let mut lock = self.current_utc.write().expect("mock clock lock poisoned");
        *lock = new_utc;
    }

    /// Advances the mock clock by the specified duration.
    pub fn advance(&self, duration: chrono::Duration) {
        let mut lock = self.current_utc.write().expect("mock clock lock poisoned");
        *lock += duration;
    }
}

impl Clock for MockClock {
    fn now_utc(&self) -> DateTime<Utc> {
        *self.current_utc.read().expect("mock clock lock poisoned")
    }

    fn now_local(&self) -> DateTime<Local> {
        let utc = self.now_utc();
        Local
            .timestamp_opt(utc.timestamp(), utc.timestamp_subsec_nanos())
            .unwrap()
    }
}
