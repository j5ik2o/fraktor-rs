//! Public persistence effector signals.

use alloc::vec::Vec;

use fraktor_persistence_core_kernel_rs::error::PersistenceError;

use crate::{EventSourcedSignal, PublishedEvent, persistence_effector_signal_auth::PersistenceEffectorSignalAuth};

/// Stable signal delivered to the aggregate actor through its private message type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PersistenceEffectorSignal<S, E> {
  /// Recovery completed with the recovered state and latest sequence number.
  #[non_exhaustive]
  RecoveryCompleted {
    #[doc(hidden)]
    auth:        PersistenceEffectorSignalAuth,
    /// Recovered state.
    state:       S,
    /// Latest recovered sequence number.
    sequence_nr: u64,
  },
  /// Events were persisted in order.
  #[non_exhaustive]
  PersistedEvents {
    #[doc(hidden)]
    auth:             PersistenceEffectorSignalAuth,
    /// Persisted events.
    events:           Vec<E>,
    #[doc(hidden)]
    published_events: Vec<PublishedEvent<E>>,
    /// Latest sequence number after the batch.
    sequence_nr:      u64,
  },
  /// A snapshot was persisted.
  #[non_exhaustive]
  PersistedSnapshot {
    #[doc(hidden)]
    auth:        PersistenceEffectorSignalAuth,
    /// Persisted snapshot state.
    snapshot:    S,
    /// Snapshot sequence number.
    sequence_nr: u64,
  },
  /// Old snapshots were deleted.
  #[non_exhaustive]
  DeletedSnapshots {
    #[doc(hidden)]
    auth:           PersistenceEffectorSignalAuth,
    /// Inclusive upper sequence number for deletion.
    to_sequence_nr: u64,
  },
  /// Persistence failed.
  #[non_exhaustive]
  Failed {
    #[doc(hidden)]
    auth:  PersistenceEffectorSignalAuth,
    /// Persistence kernel error.
    error: PersistenceError,
  },
  /// Event-sourced behavior signal.
  EventSourced {
    /// Event-sourced signal payload.
    signal: EventSourcedSignal,
  },
}
