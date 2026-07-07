//! std-only driver wrapping the distributed-data Replicator core.

#[cfg(test)]
#[path = "replicator_actor_test.rs"]
mod tests;

use alloc::string::String;

use fraktor_cluster_core_kernel_rs::ddata::{ReplicatedData, ReplicatorCore, ReplicatorOutcome, ReplicatorSettings};

/// Placeholder hook invoked when membership topology events are observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplicatorMembershipHook;

/// Placeholder hook invoked when gossip deltas are observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplicatorGossipHook;

/// std driver that wraps [`ReplicatorCore`] and exposes membership/gossip hook points.
pub struct ReplicatorActor<D: ReplicatedData, S> {
  core: ReplicatorCore<D, S>,
}

impl<D: ReplicatedData, S: Clone> ReplicatorActor<D, S> {
  /// Creates a new Replicator driver.
  #[must_use]
  pub fn new(settings: ReplicatorSettings) -> Self {
    Self { core: ReplicatorCore::new(settings) }
  }

  /// Returns immutable access to the wrapped core.
  #[must_use]
  pub const fn core(&self) -> &ReplicatorCore<D, S> {
    &self.core
  }

  /// Returns mutable access to the wrapped core.
  pub fn core_mut(&mut self) -> &mut ReplicatorCore<D, S> {
    &mut self.core
  }

  /// Placeholder membership hook for future topology-driven replication policy.
  pub const fn on_membership_event(&mut self, _hook: ReplicatorMembershipHook) {}

  /// Placeholder gossip hook for future delta dissemination integration.
  pub const fn on_gossip_event(&mut self, _hook: ReplicatorGossipHook) {}

  /// Delegates get handling to the wrapped core.
  #[must_use]
  pub fn handle_get<C: Clone>(&self, command: &super::ReplicatorGet<D, C>) -> ReplicatorOutcome<D, C, S> {
    self.core.handle_get(command)
  }

  /// Delegates update handling to the wrapped core.
  pub fn handle_update<C: Clone, F>(
    &mut self,
    command: &super::ReplicatorUpdate<D, C>,
    modify: F,
  ) -> ReplicatorOutcome<D, C, S>
  where
    F: FnOnce(Option<&D>) -> Result<D, String>, {
    self.core.handle_update(command, modify)
  }
}

/// Type alias preserving kernel get commands at the adaptor boundary.
pub type ReplicatorGet<D, C> = fraktor_cluster_core_kernel_rs::ddata::Get<D, C>;

/// Type alias preserving kernel update commands at the adaptor boundary.
pub type ReplicatorUpdate<D, C> = fraktor_cluster_core_kernel_rs::ddata::Update<D, C>;
