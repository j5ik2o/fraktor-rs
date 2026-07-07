//! std-only driver wrapping the distributed-data Replicator core.

#[cfg(test)]
#[path = "replicator_actor_test.rs"]
mod tests;

use alloc::string::String;

use fraktor_cluster_core_kernel_rs::ddata::{
  Get, ReplicatedData, ReplicatorCore, ReplicatorOutcome, ReplicatorSettings, Update,
};

/// std driver that wraps [`ReplicatorCore`].
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

  /// Delegates get handling to the wrapped core.
  #[must_use]
  pub fn handle_get<C: Clone>(&self, command: &Get<D, C>) -> ReplicatorOutcome<D, C, S> {
    self.core.handle_get(command)
  }

  /// Delegates update handling to the wrapped core.
  pub fn handle_update<C: Clone, F>(&mut self, command: &Update<D, C>, modify: F) -> ReplicatorOutcome<D, C, S>
  where
    F: FnOnce(Option<&D>) -> Result<D, String>, {
    self.core.handle_update(command, modify)
  }
}
