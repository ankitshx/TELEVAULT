//! Per-profile concurrency and execution coordination guards.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use televault_core::ids::ProfileId;

use crate::error::SchedulerError;

/// RAII lock guard holding exclusive execution rights for a specific profile.
///
/// Automatically removes the profile from the active execution registry upon `Drop`.
pub struct ProfileGuard {
    profile_id: ProfileId,
    active_set: Arc<Mutex<HashSet<ProfileId>>>,
}

impl ProfileGuard {
    /// Returns the profile ID protected by this guard.
    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }
}

impl Drop for ProfileGuard {
    fn drop(&mut self) {
        if let Ok(mut set) = self.active_set.lock() {
            set.remove(&self.profile_id);
        }
    }
}

/// Thread-safe registry ensuring that no two backups (scheduled or manual) run concurrently for the same profile.
#[derive(Debug, Clone, Default)]
pub struct ExecutionGuard {
    active_set: Arc<Mutex<HashSet<ProfileId>>>,
}

impl ExecutionGuard {
    /// Creates a new, empty execution guard registry.
    pub fn new() -> Self {
        Self {
            active_set: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Attempts to acquire an exclusive execution lock for the given profile.
    ///
    /// If a backup for this profile is already running, returns `SchedulerError::ProfileAlreadyRunning`.
    pub fn try_acquire(&self, profile_id: &ProfileId) -> Result<ProfileGuard, SchedulerError> {
        let mut set = self
            .active_set
            .lock()
            .map_err(|e| SchedulerError::Internal(format!("Execution guard lock poisoned: {e}")))?;

        if set.contains(profile_id) {
            return Err(SchedulerError::ProfileAlreadyRunning(
                profile_id.to_string(),
            ));
        }

        set.insert(profile_id.clone());

        Ok(ProfileGuard {
            profile_id: profile_id.clone(),
            active_set: Arc::clone(&self.active_set),
        })
    }

    /// Returns whether a backup operation is currently executing for the specified profile.
    pub fn is_running(&self, profile_id: &ProfileId) -> bool {
        self.active_set
            .lock()
            .map(|set| set.contains(profile_id))
            .unwrap_or(false)
    }

    /// Returns a snapshot list of all currently executing profile IDs.
    pub fn active_profiles(&self) -> Vec<ProfileId> {
        self.active_set
            .lock()
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_guard_acquisition_and_release() {
        let guard = ExecutionGuard::new();
        let pid_a = ProfileId::new("profile-a").unwrap();
        let pid_b = ProfileId::new("profile-b").unwrap();

        // 1. Acquire A
        let lock_a = guard.try_acquire(&pid_a).expect("acquire A");
        assert!(guard.is_running(&pid_a));
        assert!(!guard.is_running(&pid_b));

        // 2. Second acquire for A fails
        assert!(guard.try_acquire(&pid_a).is_err());

        // 3. Acquire B succeeds independently
        let lock_b = guard.try_acquire(&pid_b).expect("acquire B");
        assert!(guard.is_running(&pid_a));
        assert!(guard.is_running(&pid_b));

        // 4. Drop lock_a, A is released while B is still active
        drop(lock_a);
        assert!(!guard.is_running(&pid_a));
        assert!(guard.is_running(&pid_b));

        // 5. A can be acquired again
        let _lock_a2 = guard.try_acquire(&pid_a).expect("re-acquire A");
        assert!(guard.is_running(&pid_a));

        drop(lock_b);
        assert!(!guard.is_running(&pid_b));
    }
}
