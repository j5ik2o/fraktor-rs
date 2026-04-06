//! Topology update payload including current members and deltas.

use alloc::{string::String, vec::Vec};

use fraktor_utils_rs::core::time::TimerInstant;

use crate::core::cluster_topology::ClusterTopology;

/// Topology update delivered via the event stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopologyUpdate {
  /// Topology delta snapshot.
  pub topology:    ClusterTopology,
  /// Current active members.
  pub members:     Vec<String>,
  /// Newly joined members.
  pub joined:      Vec<String>,
  /// Members that left gracefully.
  pub left:        Vec<String>,
  /// Members marked dead.
  pub dead:        Vec<String>,
  /// Blocked members reported by the block list provider.
  pub blocked:     Vec<String>,
  /// Observation timestamp.
  pub observed_at: TimerInstant,
}

impl TopologyUpdate {
  /// Creates a new topology update.
  #[must_use]
  pub const fn new(
    topology: ClusterTopology,
    members: Vec<String>,
    joined: Vec<String>,
    left: Vec<String>,
    dead: Vec<String>,
    blocked: Vec<String>,
    observed_at: TimerInstant,
  ) -> Self {
    Self { topology, members, joined, left, dead, blocked, observed_at }
  }
}
