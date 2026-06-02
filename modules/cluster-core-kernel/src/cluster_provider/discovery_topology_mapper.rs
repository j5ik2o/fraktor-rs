//! Maps discovery outcomes to topology update deltas.

use alloc::{string::String, vec::Vec};
use core::time::Duration;

use fraktor_utils_core_rs::{sync::ArcShared, time::TimerInstant};

use super::{DiscoveredAuthority, DiscoveryResult};
use crate::{BlockListProvider, ClusterTopology, TopologyUpdate};

#[cfg(test)]
#[path = "discovery_topology_mapper_test.rs"]
mod tests;

/// Stateful mapper from discovery outcomes to topology update deltas.
pub struct DiscoveryTopologyMapper {
  block_list_provider: ArcShared<dyn BlockListProvider>,
  authorities:         Vec<String>,
  version:             u64,
}

impl DiscoveryTopologyMapper {
  /// Creates a discovery topology mapper.
  #[must_use]
  pub const fn new(block_list_provider: ArcShared<dyn BlockListProvider>) -> Self {
    Self { block_list_provider, authorities: Vec::new(), version: 0 }
  }

  /// Applies a discovery result and returns a topology delta when membership changed.
  #[must_use]
  pub fn apply(&mut self, result: &DiscoveryResult) -> Option<TopologyUpdate> {
    if result.is_failed() {
      return None;
    }

    let current = Self::deduplicated_authorities(result);
    let joined = Self::joined_authorities(&self.authorities, &current);
    let left = Self::left_authorities(&self.authorities, &current);

    if joined.is_empty() && left.is_empty() {
      return None;
    }

    self.authorities = current.clone();
    self.version += 1;

    let topology = ClusterTopology::new(self.version, joined.clone(), left.clone(), Vec::new());
    Some(TopologyUpdate::new(
      topology,
      current,
      joined,
      left,
      Vec::new(),
      self.block_list_provider.blocked_members(),
      Self::observed_at(result, self.version),
    ))
  }

  fn deduplicated_authorities(result: &DiscoveryResult) -> Vec<String> {
    let mut authorities = Vec::new();
    for authority in result.authorities() {
      if !authorities.iter().any(|known| known == authority.authority()) {
        authorities.push(authority.to_authority());
      }
    }
    authorities
  }

  fn joined_authorities(previous: &[String], current: &[String]) -> Vec<String> {
    current.iter().filter(|authority| !previous.contains(authority)).cloned().collect()
  }

  fn left_authorities(previous: &[String], current: &[String]) -> Vec<String> {
    previous.iter().filter(|authority| !current.contains(authority)).cloned().collect()
  }

  fn observed_at(result: &DiscoveryResult, version: u64) -> TimerInstant {
    result
      .observed_at()
      .or_else(|| result.authorities().first().map(DiscoveredAuthority::observed_at))
      .unwrap_or_else(|| TimerInstant::from_ticks(version, Duration::from_secs(1)))
  }
}
