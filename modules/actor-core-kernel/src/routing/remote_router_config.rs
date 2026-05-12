//! Pool router configuration for remote routee deployment.

#[cfg(test)]
#[path = "remote_router_config_test.rs"]
mod tests;

use alloc::{string::String, vec::Vec};

use super::{Pool, RemoteRouterPool, RemoteRoutingLogic, Router, RouterConfig};
use crate::actor::{
  Address,
  deploy::{Deploy, RemoteScope, Scope},
};

/// Pool router configuration that deploys routees on remote nodes.
///
/// Corresponds to Pekko's
/// `org.apache.pekko.remote.routing.RemoteRouterConfig`.
pub struct RemoteRouterConfig {
  local: RemoteRouterPool,
  nodes: Vec<Address>,
}

impl RemoteRouterConfig {
  /// Creates a remote router configuration backed by a local pool.
  ///
  /// # Panics
  ///
  /// Panics if `nodes` is empty, or if any entry is a local address (no
  /// host/port). The local-address check mirrors `RemoteScope::new` so that
  /// `deploy_for_routee_index` cannot construct a `RemoteScope` from a
  /// local-only `Address`.
  #[must_use]
  pub fn new(local: impl Into<RemoteRouterPool>, nodes: Vec<Address>) -> Self {
    assert!(!nodes.is_empty(), "nodes must not be empty");
    assert!(
      nodes.iter().all(Address::has_global_scope),
      "RemoteRouterConfig requires every node to be a remote address with host and port",
    );
    Self { local: local.into(), nodes }
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
  pub const fn local(&self) -> &RemoteRouterPool {
    &self.local
  }

  /// Returns the configured remote deployment nodes.
  #[must_use]
  pub fn nodes(&self) -> &[Address] {
    &self.nodes
  }
}

impl RouterConfig for RemoteRouterConfig {
  type Logic = RemoteRoutingLogic;

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

impl Pool for RemoteRouterConfig {
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
