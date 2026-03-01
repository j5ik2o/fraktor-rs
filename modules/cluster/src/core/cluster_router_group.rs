//! Minimal group router for configured cluster routee paths.

#[cfg(test)]
mod tests;

use crate::core::ClusterRouterGroupSettings;

/// Group router that maps hash keys to configured routee paths.
pub struct ClusterRouterGroup {
  settings: ClusterRouterGroupSettings,
}

impl ClusterRouterGroup {
  /// Creates a group router from settings.
  #[must_use]
  pub const fn new(settings: ClusterRouterGroupSettings) -> Self {
    Self { settings }
  }

  /// Returns router settings.
  #[must_use]
  pub const fn settings(&self) -> &ClusterRouterGroupSettings {
    &self.settings
  }

  /// Returns configured routee paths.
  #[must_use]
  pub fn routee_paths(&self) -> &[alloc::string::String] {
    self.settings.routee_paths()
  }

  /// Selects a routee path for the provided hash key.
  #[must_use]
  pub fn routee_for_key(&self, key: u64) -> Option<&str> {
    let routees = self.settings.routee_paths();
    if routees.is_empty() {
      return None;
    }
    let index = (key as usize) % routees.len();
    Some(routees[index].as_str())
  }
}
