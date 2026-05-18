//! Public persistence effector signals.

use alloc::vec::Vec;

use fraktor_persistence_core_kernel_rs::error::PersistenceError;

/// Stable signal delivered to the aggregate actor through its private message type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PersistenceEffectorSignal<S, E> {
  /// Recovery completed with the recovered state and latest sequence number.
  RecoveryCompleted {
    /// Recovered state.
    state:       S,
    /// Latest recovered sequence number.
    sequence_nr: u64,
  },
  /// Events were persisted in order.
  PersistedEvents {
    /// Persisted events.
    events:      Vec<E>,
    /// Latest sequence number after the batch.
    sequence_nr: u64,
  },
  /// A snapshot was persisted.
  PersistedSnapshot {
    /// Persisted snapshot state.
    snapshot:    S,
    /// Snapshot sequence number.
    sequence_nr: u64,
  },
  /// Old snapshots were deleted.
  DeletedSnapshots {
    /// Inclusive upper sequence number for deletion.
    to_sequence_nr: u64,
  },
  /// Persistence failed.
  Failed {
    /// Persistence kernel error.
    error: PersistenceError,
  },
}
