//! Membership coordinator runtime state.

/// Coordinator runtime state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MembershipCoordinatorState {
  /// Coordinator is stopped.
  Stopped,
  /// Coordinator is running in member mode.
  Member,
  /// Coordinator is running in client mode.
  Client,
}
