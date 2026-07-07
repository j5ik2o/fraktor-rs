//! Membership coordinator error types.

use super::{GossipTransportError, MembershipCoordinatorState, MembershipError};
use crate::extension::{ClusterExtensionConfigValidationError, ClusterProviderError};

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
  /// Cluster configuration validation failure.
  Configuration(ClusterExtensionConfigValidationError),
  /// Gossip transport error.
  Transport(GossipTransportError),
  /// Cluster provider error.
  ClusterProvider(ClusterProviderError),
}
