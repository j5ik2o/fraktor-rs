//! Minimal group router for configured cluster routee paths.

#[cfg(test)]
#[path = "cluster_router_group_test.rs"]
mod tests;

use alloc::string::String;

use crate::ClusterRouterGroupConfig;

/// Group router that maps hash keys to configured routee paths.
pub struct ClusterRouterGroup {
  config: ClusterRouterGroupConfig,
}

impl ClusterRouterGroup {
  /// Creates a group router from config.
  #[must_use]
  pub const fn new(config: ClusterRouterGroupConfig) -> Self {
    Self { config }
  }

  /// Returns router config.
  #[must_use]
  pub const fn config(&self) -> &ClusterRouterGroupConfig {
    &self.config
  }

  /// Returns configured routee paths.
  #[must_use]
  pub fn routee_paths(&self) -> &[String] {
    self.config.routee_paths()
  }

  /// Selects a routee path for the provided hash key.
  #[must_use]
  pub fn routee_for_key(&self, key: u64) -> Option<&str> {
    let routees = self.config.routee_paths();
    if routees.is_empty() {
      return None;
    }
    let index = (key as usize) % routees.len();
    Some(routees[index].as_str())
  }

  /// Returns the configured routee paths when the local node participates.
  ///
  /// The local node contributes its routee paths only when local routees are
  /// allowed ([`ClusterRouterGroupConfig::allow_local_routees`]) and it carries
  /// every required role ([`ClusterRouterGroupConfig::satisfies_roles`]),
  /// mirroring Pekko's cluster router group path selection. Otherwise no local
  /// paths participate.
  ///
  /// This is the core selection policy; the std cluster runtime drives it from
  /// membership and self-role updates.
  #[must_use]
  pub fn local_routee_paths(&self, self_roles: &[String]) -> &[String] {
    if self.config.allow_local_routees() && self.config.satisfies_roles(self_roles) {
      self.config.routee_paths()
    } else {
      &[]
    }
  }
}
