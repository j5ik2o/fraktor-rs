//! std-only driver wrapping the Cluster Singleton proxy core.

#[cfg(test)]
#[path = "cluster_singleton_proxy_actor_test.rs"]
mod tests;

use fraktor_cluster_core_kernel_rs::{
  membership::{DataCenter, NodeRecord},
  singleton::{ClusterSingletonProxy, ClusterSingletonProxyConfig, ClusterSingletonProxyOutcome},
};

use crate::membership::ClusterMembershipEventHook;

/// std driver that wraps [`ClusterSingletonProxy`] and exposes membership hook points.
pub struct ClusterSingletonProxyActor<M> {
  proxy:             ClusterSingletonProxy<M>,
  local_data_center: DataCenter,
}

impl<M> ClusterSingletonProxyActor<M> {
  /// Creates a new proxy driver.
  #[must_use]
  pub fn new(config: ClusterSingletonProxyConfig, local_data_center: DataCenter) -> Self {
    Self { proxy: ClusterSingletonProxy::new(config), local_data_center }
  }

  /// Returns immutable access to the wrapped proxy.
  #[must_use]
  pub const fn proxy(&self) -> &ClusterSingletonProxy<M> {
    &self.proxy
  }

  /// Returns mutable access to the wrapped proxy.
  pub fn proxy_mut(&mut self) -> &mut ClusterSingletonProxy<M> {
    &mut self.proxy
  }

  /// Placeholder membership hook for future event-stream integration.
  pub const fn on_membership_event(&mut self, _hook: ClusterMembershipEventHook) {}

  /// Delegates singleton identification to the wrapped proxy.
  #[must_use]
  pub fn identify(&mut self, members: &[NodeRecord]) -> ClusterSingletonProxyOutcome<M> {
    self.proxy.identify(members, &self.local_data_center)
  }

  /// Delegates outbound message handling to the wrapped proxy.
  #[must_use]
  pub fn handle_message(&mut self, message: M) -> ClusterSingletonProxyOutcome<M> {
    self.proxy.handle_message(message)
  }
}
