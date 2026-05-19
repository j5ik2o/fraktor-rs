//! Persistence mode.

/// Selects how persistence operations store state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PersistenceMode {
  /// Store events and snapshots through the configured persistence kernel.
  #[default]
  Persisted,
  /// Store events and snapshots in actor-system scoped memory.
  Ephemeral,
  /// Skip storage and run callbacks immediately.
  Deferred,
}
