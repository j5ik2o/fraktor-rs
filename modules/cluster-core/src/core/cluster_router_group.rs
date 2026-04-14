//! Minimal group router for configured cluster routee paths.

#[cfg(test)]
mod tests;

use alloc::string::String;

use crate::core::ClusterRouterGroupConfig;

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
}
