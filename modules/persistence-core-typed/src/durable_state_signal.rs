//! Typed durable state lifecycle signals.

use fraktor_persistence_core_kernel_rs::state::DurableStateError;

/// Signals emitted by future typed durable state behavior integration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DurableStateSignal<S> {
  /// Durable state recovery completed.
  RecoveryCompleted {
    /// Recovered state, when the durable state store had one.
    state:    Option<S>,
    /// Recovered revision.
    revision: u64,
  },
  /// Durable state recovery failed.
  RecoveryFailed {
    /// Durable state error reported by the persistence kernel.
    error: DurableStateError,
  },
  /// Durable state was persisted.
  StatePersisted {
    /// Persisted state.
    state:    S,
    /// Persisted revision.
    revision: u64,
  },
  /// Durable state was deleted.
  StateDeleted {
    /// Deleted revision.
    revision: u64,
  },
  /// Durable state persistence failed.
  PersistenceFailed {
    /// Durable state error reported by the persistence kernel.
    error: DurableStateError,
  },
}
