//! Typed durable state lifecycle signals.

use fraktor_persistence_core_kernel_rs::state::DurableStateError;

use crate::durable_state_signal_auth::DurableStateSignalAuth;

/// Signals emitted by future typed durable state behavior integration.
///
/// Durable state signals are produced by the persistence runtime and delivered
/// through user private message types. External crates can wrap a received
/// signal, but cannot construct trusted signals directly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DurableStateSignal<S> {
  /// Durable state recovery completed.
  #[non_exhaustive]
  RecoveryCompleted {
    #[doc(hidden)]
    auth:     DurableStateSignalAuth,
    /// Recovered state, when the durable state store had one.
    state:    Option<S>,
    /// Recovered revision.
    revision: u64,
  },
  /// Durable state recovery failed.
  #[non_exhaustive]
  RecoveryFailed {
    #[doc(hidden)]
    auth:  DurableStateSignalAuth,
    /// Durable state error reported by the persistence kernel.
    error: DurableStateError,
  },
  /// Durable state was persisted.
  #[non_exhaustive]
  StatePersisted {
    #[doc(hidden)]
    auth:     DurableStateSignalAuth,
    /// Persisted state.
    state:    S,
    /// Persisted revision.
    revision: u64,
  },
  /// Durable state was deleted.
  #[non_exhaustive]
  StateDeleted {
    #[doc(hidden)]
    auth:     DurableStateSignalAuth,
    /// Deleted revision.
    revision: u64,
  },
  /// Durable state persistence failed.
  #[non_exhaustive]
  PersistenceFailed {
    #[doc(hidden)]
    auth:  DurableStateSignalAuth,
    /// Durable state error reported by the persistence kernel.
    error: DurableStateError,
  },
}
