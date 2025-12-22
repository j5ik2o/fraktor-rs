//! Placement coordinator state definition.

/// State of the placement coordinator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementCoordinatorState {
  /// Coordinator is stopped.
  Stopped,
  /// Coordinator is running as a member.
  Member,
  /// Coordinator is running as a client.
  Client,
  /// Coordinator is not ready to resolve.
  NotReady,
}
