#[cfg(test)]
#[path = "connection_snapshot_test.rs"]
mod tests;

use crate::snapshot::{ConnectionState, LogicSnapshot};

/// Diagnostic snapshot of a single connection between two stage logics.
///
/// Corresponds to Pekko `ConnectionSnapshotImpl(id: Int, in: LogicSnapshot,
/// out: LogicSnapshot, state: ConnectionState)`. The upstream accessor is
/// renamed to `in_logic` because `in` is a reserved Rust keyword.
#[derive(Debug, Clone)]
pub struct ConnectionSnapshot {
  id:       u32,
  in_logic: LogicSnapshot,
  out:      LogicSnapshot,
  state:    ConnectionState,
}

impl ConnectionSnapshot {
  /// Creates a new connection snapshot.
  #[must_use]
  pub const fn new(id: u32, in_logic: LogicSnapshot, out: LogicSnapshot, state: ConnectionState) -> Self {
    Self { id, in_logic, out, state }
  }

  /// Returns the connection identifier.
  #[must_use]
  pub const fn id(&self) -> u32 {
    self.id
  }

  /// Returns the upstream logic (Pekko `in`, renamed to avoid the Rust keyword).
  #[must_use]
  pub const fn in_logic(&self) -> &LogicSnapshot {
    &self.in_logic
  }

  /// Returns the downstream logic.
  #[must_use]
  pub const fn out(&self) -> &LogicSnapshot {
    &self.out
  }

  /// Returns the runtime state of the connection.
  #[must_use]
  pub const fn state(&self) -> ConnectionState {
    self.state
  }
}
