//! Transfer state machine and valid transition rules.

use crate::error::{Result, TransferError};
use serde::{Deserialize, Serialize};
use std::fmt;
use televault_core::TransferStatus;

/// Fine-grained state machine for an individual transfer work unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferState {
    /// Queued in the transfer engine, awaiting worker assignment.
    Pending,
    /// Worker assigned, initializing source stream and staging payload.
    Preparing,
    /// Streaming data to/from the storage provider over the wire.
    Transferring,
    /// Remote hash, size, or reference verification in progress.
    Verifying,
    /// Transfer successfully uploaded/downloaded, verified, and temporary payload cleaned.
    Completed,
    /// Transfer encountered an unrecoverable failure or retries were exhausted.
    Failed,
    /// Transient failure encountered; backoff timer active before next retry attempt.
    Retrying,
    /// Transfer was cooperatively cancelled by user or scheduler.
    Cancelled,
}

impl TransferState {
    /// Validates whether a state transition from `self` to `next` is permissible.
    pub fn can_transition_to(&self, next: TransferState) -> bool {
        match (self, next) {
            // Identity is a no-op
            (current, target) if *current == target => true,

            // Pending can move to Preparing, or be Cancelled/Failed directly
            (Self::Pending, Self::Preparing | Self::Cancelled | Self::Failed) => true,

            // Preparing can move to Transferring, or fail/cancel
            (Self::Preparing, Self::Transferring | Self::Failed | Self::Cancelled) => true,

            // Transferring can move to Verifying, Completed (direct downloads), Retrying, Failed, or Cancelled
            (
                Self::Transferring,
                Self::Verifying | Self::Completed | Self::Retrying | Self::Failed | Self::Cancelled,
            ) => true,

            // Verifying can complete, retry (if remote verify failed retryably), fail, or cancel
            (
                Self::Verifying,
                Self::Completed | Self::Retrying | Self::Failed | Self::Cancelled,
            ) => true,

            // Retrying can transition back to Preparing or Transferring, or fail/cancel
            (
                Self::Retrying,
                Self::Preparing | Self::Transferring | Self::Failed | Self::Cancelled,
            ) => true,

            // Terminal states cannot transition to any other active state
            (Self::Completed, _) => false,
            (Self::Failed, _) => false,
            (Self::Cancelled, _) => false,

            // All other transitions are invalid
            _ => false,
        }
    }

    /// Attempts to mutate the state to `next`, returning an error if the transition is invalid.
    pub fn transition_to(&mut self, next: TransferState) -> Result<()> {
        if !self.can_transition_to(next) {
            return Err(TransferError::InvalidStateTransition {
                from: self.to_string(),
                to: next.to_string(),
            });
        }
        *self = next;
        Ok(())
    }

    /// Returns `true` if this state represents a terminal completion state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Returns `true` if this state represents an actively running worker phase.
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Preparing | Self::Transferring | Self::Verifying | Self::Retrying
        )
    }

    /// Maps the fine-grained `TransferState` to the shared `televault_core::TransferStatus` enum.
    pub fn to_transfer_status(&self) -> TransferStatus {
        match self {
            Self::Pending | Self::Preparing => TransferStatus::Pending,
            Self::Transferring | Self::Verifying => TransferStatus::Transferring,
            Self::Retrying => TransferStatus::Paused,
            Self::Completed => TransferStatus::Completed,
            Self::Failed => TransferStatus::Failed,
            Self::Cancelled => TransferStatus::Cancelled,
        }
    }
}

impl fmt::Display for TransferState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Preparing => write!(f, "preparing"),
            Self::Transferring => write!(f, "transferring"),
            Self::Verifying => write!(f, "verifying"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Retrying => write!(f, "retrying"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_state_transitions() {
        let mut state = TransferState::Pending;
        assert!(state.transition_to(TransferState::Preparing).is_ok());
        assert!(state.transition_to(TransferState::Transferring).is_ok());
        assert!(state.transition_to(TransferState::Verifying).is_ok());
        assert!(state.transition_to(TransferState::Completed).is_ok());
        assert!(state.is_terminal());
    }

    #[test]
    fn test_retry_cycle_transitions() {
        let mut state = TransferState::Pending;
        state.transition_to(TransferState::Preparing).unwrap();
        state.transition_to(TransferState::Transferring).unwrap();
        state.transition_to(TransferState::Retrying).unwrap();
        state.transition_to(TransferState::Transferring).unwrap();
        state.transition_to(TransferState::Verifying).unwrap();
        state.transition_to(TransferState::Completed).unwrap();
        assert_eq!(state, TransferState::Completed);
    }

    #[test]
    fn test_invalid_terminal_transitions() {
        let mut state = TransferState::Completed;
        let err = state.transition_to(TransferState::Transferring);
        assert!(err.is_err());
        match err.unwrap_err() {
            TransferError::InvalidStateTransition { from, to } => {
                assert_eq!(from, "completed");
                assert_eq!(to, "transferring");
            }
            other => panic!("Unexpected error: {other:?}"),
        }

        let mut cancelled = TransferState::Cancelled;
        assert!(cancelled.transition_to(TransferState::Completed).is_err());

        let mut failed = TransferState::Failed;
        assert!(failed.transition_to(TransferState::Verifying).is_err());
    }

    #[test]
    fn test_mapping_to_core_transfer_status() {
        assert_eq!(
            TransferState::Pending.to_transfer_status(),
            TransferStatus::Pending
        );
        assert_eq!(
            TransferState::Preparing.to_transfer_status(),
            TransferStatus::Pending
        );
        assert_eq!(
            TransferState::Transferring.to_transfer_status(),
            TransferStatus::Transferring
        );
        assert_eq!(
            TransferState::Verifying.to_transfer_status(),
            TransferStatus::Transferring
        );
        assert_eq!(
            TransferState::Retrying.to_transfer_status(),
            TransferStatus::Paused
        );
        assert_eq!(
            TransferState::Completed.to_transfer_status(),
            TransferStatus::Completed
        );
        assert_eq!(
            TransferState::Failed.to_transfer_status(),
            TransferStatus::Failed
        );
        assert_eq!(
            TransferState::Cancelled.to_transfer_status(),
            TransferStatus::Cancelled
        );
    }
}
