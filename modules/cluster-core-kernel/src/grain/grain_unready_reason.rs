//! Reason a grain runtime is not ready to accept traffic.

use alloc::string::String;

use crate::{activation::PlacementCoordinatorState, membership::NodeStatus};

/// Identifies a single unmet condition for grain readiness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GrainUnreadyReason {
  /// Self node is absent from membership or not in an accepting status.
  SelfNodeNotUp {
    /// Observed self node status, or `None` when absent from membership.
    status: Option<NodeStatus>,
  },
  /// Placement coordination cannot resolve placements.
  PlacementNotReady {
    /// Observed placement coordinator state.
    state: PlacementCoordinatorState,
  },
  /// An expected kind is not registered.
  KindNotRegistered {
    /// Name of the expected kind that is not registered.
    kind: String,
  },
}
