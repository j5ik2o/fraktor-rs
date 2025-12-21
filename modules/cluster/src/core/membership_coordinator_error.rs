//! Membership coordinator error types.

use crate::core::{
  gossip_transport_error::GossipTransportError, membership_coordinator_state::MembershipCoordinatorState,
  membership_error::MembershipError,
};

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
