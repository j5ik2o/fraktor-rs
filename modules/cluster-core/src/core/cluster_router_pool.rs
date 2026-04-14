//! Minimal pool router for cluster routee authorities.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};

use crate::core::ClusterRouterPoolConfig;

/// Round-robin pool router for cluster routees.
pub struct ClusterRouterPool {
  settings:   ClusterRouterPoolConfig,
  routees:    Vec<String>,
  next_index: usize,
}

impl ClusterRouterPool {
  /// Creates a pool router with settings and initial routees.
  #[must_use]
  pub const fn new(settings: ClusterRouterPoolConfig, routees: Vec<String>) -> Self {
    Self { settings, routees, next_index: 0 }
  }

  /// Returns the router settings.
  #[must_use]
  pub const fn settings(&self) -> &ClusterRouterPoolConfig {
    &self.settings
  }

  /// Returns the current routees.
  #[must_use]
  pub fn routees(&self) -> &[String] {
    &self.routees
  }

  /// Replaces current routees.
  pub fn replace_routees(&mut self, routees: Vec<String>) {
    self.routees = routees;
    self.next_index = 0;
  }

  /// Selects the next routee authority using round-robin.
  ///
  /// The effective pool is capped at [`ClusterRouterPoolConfig::total_instances`].
  #[must_use]
  pub fn next_routee(&mut self) -> Option<&str> {
    if self.routees.is_empty() {
      return None;
    }
    let effective_count = self.routees.len().min(self.settings.total_instances());
    let index = self.next_index % effective_count;
    self.next_index = (self.next_index + 1) % effective_count;
    Some(self.routees[index].as_str())
  }
}
