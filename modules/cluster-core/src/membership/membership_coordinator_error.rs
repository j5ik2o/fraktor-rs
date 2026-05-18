//! Membership coordinator error types.

use super::{GossipTransportError, MembershipCoordinatorState, MembershipError};

/// Errors returned by the membership coordinator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MembershipCoordinatorError {
  /// Coordinator has not started yet.
  NotStarted,
  /// Coordinator is in a state that rejects the operation.
  InvalidState {
    /// Current coordinator state.
    state: MembershipCoordinatorState,
  },
  /// Membership table error.
  Membership(MembershipError),
  /// Gossip transport error.
  Transport(GossipTransportError),
}
