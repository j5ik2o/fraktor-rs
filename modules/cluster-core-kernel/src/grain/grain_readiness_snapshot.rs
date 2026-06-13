//! Snapshot of grain readiness inputs and the pure derivation query.

#[cfg(test)]
#[path = "grain_readiness_snapshot_test.rs"]
mod tests;

use alloc::{string::String, vec::Vec};

use super::{GrainReadiness, GrainUnreadyReason};
use crate::{activation::PlacementCoordinatorState, membership::NodeStatus};

/// Captures the runtime inputs for grain readiness derivation at a point in time.
///
/// The snapshot is a value object: [`Self::readiness`] derives the outcome with no
/// side effects and no host dependencies, so the same snapshot and the same expected
/// kinds always yield the same result. Snapshot freshness is fixed at construction
/// time; continuous monitoring is the caller's responsibility.
pub struct GrainReadinessSnapshot {
  self_status:      Option<NodeStatus>,
  placement_state:  PlacementCoordinatorState,
  registered_kinds: Vec<String>,
}

impl GrainReadinessSnapshot {
  /// Creates a snapshot from the observed runtime inputs.
  ///
  /// `self_status` is `None` when the self node is absent from membership.
  #[must_use]
  pub const fn new(
    self_status: Option<NodeStatus>,
    placement_state: PlacementCoordinatorState,
    registered_kinds: Vec<String>,
  ) -> Self {
    Self { self_status, placement_state, registered_kinds }
  }

  /// Derives readiness from this snapshot.
  ///
  /// Returns [`GrainReadiness::Ready`] only when the self node is in an accepting
  /// status (`Up` or `WeaklyUp`), placement coordination can resolve placements
  /// (`Member` or `Client`), and every entry in `expected_kinds` is registered.
  /// When `expected_kinds` is empty the kind condition is satisfied vacuously.
  /// Every unmet condition is reported in `reasons`; the derivation never
  /// short-circuits on the first failure.
  #[must_use]
  pub fn readiness(&self, expected_kinds: &[String]) -> GrainReadiness {
    let mut reasons = Vec::new();

    if !self.self_node_is_up() {
      reasons.push(GrainUnreadyReason::SelfNodeNotUp { status: self.self_status });
    }
    if !self.placement_is_resolvable() {
      reasons.push(GrainUnreadyReason::PlacementNotReady { state: self.placement_state });
    }
    for kind in expected_kinds {
      if !self.registered_kinds.contains(kind) {
        reasons.push(GrainUnreadyReason::KindNotRegistered { kind: kind.clone() });
      }
    }

    if reasons.is_empty() { GrainReadiness::Ready } else { GrainReadiness::NotReady { reasons } }
  }

  const fn self_node_is_up(&self) -> bool {
    matches!(self.self_status, Some(NodeStatus::Up | NodeStatus::WeaklyUp))
  }

  const fn placement_is_resolvable(&self) -> bool {
    matches!(self.placement_state, PlacementCoordinatorState::Member | PlacementCoordinatorState::Client)
  }
}
