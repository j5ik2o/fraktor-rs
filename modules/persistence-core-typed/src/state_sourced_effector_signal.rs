//! Public state-sourced effector signals.

#[cfg(test)]
#[path = "state_sourced_effector_signal_test.rs"]
mod tests;

use fraktor_persistence_core_kernel_rs::state::DurableStateError;

use crate::state_sourced_effector_signal_auth::StateSourcedEffectorSignalAuth;

/// Stable signal delivered to the aggregate actor through its private message type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateSourcedEffectorSignal<S> {
  /// Durable state recovery completed.
  #[non_exhaustive]
  RecoveryCompleted {
    #[doc(hidden)]
    auth:     StateSourcedEffectorSignalAuth,
    /// Recovered state, when the durable state store had one.
    state:    Option<S>,
    /// Recovered revision.
    revision: u64,
  },
  /// Durable state recovery failed.
  #[non_exhaustive]
  RecoveryFailed {
    #[doc(hidden)]
    auth:  StateSourcedEffectorSignalAuth,
    /// Durable state error reported by the persistence kernel.
    error: DurableStateError,
  },
  /// Durable state was persisted.
  #[non_exhaustive]
  StatePersisted {
    #[doc(hidden)]
    auth:     StateSourcedEffectorSignalAuth,
    /// Persisted state.
    state:    S,
    /// Persisted revision.
    revision: u64,
  },
  /// Durable state was deleted.
  #[non_exhaustive]
  StateDeleted {
    #[doc(hidden)]
    auth:     StateSourcedEffectorSignalAuth,
    /// Deleted revision.
    revision: u64,
  },
  /// Durable state persistence failed.
  #[non_exhaustive]
  PersistenceFailed {
    #[doc(hidden)]
    auth:  StateSourcedEffectorSignalAuth,
    /// Durable state error reported by the persistence kernel.
    error: DurableStateError,
  },
}
