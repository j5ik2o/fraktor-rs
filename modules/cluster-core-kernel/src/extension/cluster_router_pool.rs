//! Minimal pool router for cluster routee authorities.

#[cfg(test)]
#[path = "cluster_router_pool_test.rs"]
mod tests;

use alloc::{string::String, vec, vec::Vec};

use crate::ClusterRouterPoolConfig;

/// Round-robin pool router for cluster routees.
pub struct ClusterRouterPool {
  config:     ClusterRouterPoolConfig,
  routees:    Vec<String>,
  next_index: usize,
}

impl ClusterRouterPool {
  /// Creates a pool router with config and initial routees.
  #[must_use]
  pub const fn new(config: ClusterRouterPoolConfig, routees: Vec<String>) -> Self {
    Self { config, routees, next_index: 0 }
  }

  /// Creates a pool router by allocating routees across the given candidate node
  /// authorities.
  ///
  /// Candidates are expected to be distinct node authorities, pre-filtered by
  /// [`ClusterRouterPoolConfig::satisfies_roles`] and node availability. Routees
  /// are distributed least-loaded first, capped at
  /// [`ClusterRouterPoolConfig::total_instances`] in total and at
  /// [`ClusterRouterPoolConfig::max_instances_per_node`] per authority. Ties for
  /// the least-loaded authority are broken in favor of the earliest entry in
  /// `candidates`, so the allocation is deterministic for a given candidate
  /// order.
  #[must_use]
  pub fn from_candidates(config: ClusterRouterPoolConfig, candidates: &[String]) -> Self {
    let routees = allocate_routees(&config, candidates);
    Self { config, routees, next_index: 0 }
  }

  /// Returns the router config.
  #[must_use]
  pub const fn config(&self) -> &ClusterRouterPoolConfig {
    &self.config
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
    let effective_count = self.routees.len().min(self.config.total_instances());
    let index = self.next_index % effective_count;
    self.next_index = (self.next_index + 1) % effective_count;
    Some(self.routees[index].as_str())
  }
}

/// Distributes routees across candidate authorities honoring the total and
/// per-node caps, using least-loaded round-robin placement.
fn allocate_routees(config: &ClusterRouterPoolConfig, candidates: &[String]) -> Vec<String> {
  let total = config.total_instances();
  let max_per_node = config.max_instances_per_node();
  if candidates.is_empty() {
    return Vec::new();
  }
  let mut counts = vec![0usize; candidates.len()];
  let mut routees: Vec<String> = Vec::new();
  while routees.len() < total {
    let mut best: Option<usize> = None;
    for (index, &count) in counts.iter().enumerate() {
      if count >= max_per_node {
        continue;
      }
      match best {
        | Some(best_index) if counts[best_index] <= count => {},
        | _ => best = Some(index),
      }
    }
    match best {
      | Some(index) => {
        routees.push(candidates[index].clone());
        counts[index] += 1;
      },
      | None => break,
    }
  }
  routees
}
