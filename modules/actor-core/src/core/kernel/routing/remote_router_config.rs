//! Pool router configuration for remote routee deployment.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};

use super::{Pool, Router, RouterConfig};
use crate::core::kernel::actor::{
  Address,
  deploy::{Deploy, RemoteScope, Scope},
};

/// Pool router configuration that deploys routees on remote nodes.
///
/// Corresponds to Pekko's
/// `org.apache.pekko.remote.routing.RemoteRouterConfig`.
pub struct RemoteRouterConfig<P: Pool> {
  local: P,
  nodes: Vec<Address>,
}

impl<P: Pool> RemoteRouterConfig<P> {
  /// Creates a remote router configuration backed by a local pool.
  ///
  /// # Panics
  ///
  /// Panics if `nodes` is empty.
  #[must_use]
  pub fn new(local: P, nodes: Vec<Address>) -> Self {
    assert!(!nodes.is_empty(), "nodes must not be empty");
    Self { local, nodes }
  }

  /// Builds a deployment descriptor for a routee index.
  ///
  /// The target node cycles through the configured node list by index, matching
  /// Pekko's repeated node iterator without requiring internal mutable state.
  #[must_use]
  pub fn deploy_for_routee_index(&self, index: usize) -> Deploy {
    let node = self.nodes[index % self.nodes.len()].clone();
    Deploy::new().with_scope(Scope::Remote(RemoteScope::new(node)))
  }

  /// Returns the local pool configuration.
  #[must_use]
  pub const fn local(&self) -> &P {
    &self.local
  }

  /// Returns the configured remote deployment nodes.
  #[must_use]
  pub fn nodes(&self) -> &[Address] {
    &self.nodes
  }
}

impl<P: Pool> RouterConfig for RemoteRouterConfig<P> {
  type Logic = P::Logic;

  fn create_router(&self) -> Router<Self::Logic> {
    self.local.create_router()
  }

  fn router_dispatcher(&self) -> String {
    self.local.router_dispatcher()
  }

  fn stop_router_when_all_routees_removed(&self) -> bool {
    self.local.stop_router_when_all_routees_removed()
  }
}

impl<P: Pool> Pool for RemoteRouterConfig<P> {
  fn nr_of_instances(&self) -> usize {
    self.local.nr_of_instances()
  }

  fn has_resizer(&self) -> bool {
    self.local.has_resizer()
  }

  fn use_pool_dispatcher(&self) -> bool {
    self.local.use_pool_dispatcher()
  }
}
