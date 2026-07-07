//! std-only driver wrapping the Cluster Singleton manager core.

#[cfg(test)]
#[path = "cluster_singleton_manager_actor_test.rs"]
mod tests;

use fraktor_cluster_core_kernel_rs::singleton::{
  ClusterSingletonManager, ClusterSingletonManagerConfig, ClusterSingletonManagerMessage,
  ClusterSingletonManagerOutcome,
};
use fraktor_utils_core_rs::time::TimerInstant;

use crate::membership::ClusterMembershipEventHook;

/// std driver that wraps [`ClusterSingletonManager`] and exposes membership hook points.
pub struct ClusterSingletonManagerActor {
  manager: ClusterSingletonManager,
}

impl ClusterSingletonManagerActor {
  /// Creates a new manager driver.
  #[must_use]
  pub fn new(config: ClusterSingletonManagerConfig, local_authority: impl Into<String>) -> Self {
    Self { manager: ClusterSingletonManager::new(config, local_authority) }
  }

  /// Returns immutable access to the wrapped manager.
  #[must_use]
  pub const fn manager(&self) -> &ClusterSingletonManager {
    &self.manager
  }

  /// Returns mutable access to the wrapped manager.
  pub fn manager_mut(&mut self) -> &mut ClusterSingletonManager {
    &mut self.manager
  }

  /// Placeholder membership hook for future event-stream integration.
  pub const fn on_membership_event(&mut self, _hook: ClusterMembershipEventHook) {}

  /// Delegates topology application to the wrapped manager.
  #[must_use]
  pub fn apply_topology(
    &mut self,
    members: &[fraktor_cluster_core_kernel_rs::membership::NodeRecord],
    now: TimerInstant,
  ) -> ClusterSingletonManagerOutcome {
    self.manager.apply_topology(members, now)
  }

  /// Delegates hand-over message handling to the wrapped manager.
  #[must_use]
  pub fn handle_message(&mut self, message: ClusterSingletonManagerMessage) -> ClusterSingletonManagerOutcome {
    self.manager.handle_message(message)
  }

  /// Delegates retry polling to the wrapped manager.
  #[must_use]
  pub fn poll(&mut self, now: TimerInstant) -> ClusterSingletonManagerOutcome {
    self.manager.poll(now)
  }
}
