//! Vector clock used for gossip seen tracking and convergence checks.

#[cfg(test)]
mod tests;

use alloc::{
  collections::BTreeMap,
  string::{String, ToString},
};

/// Version vector keyed by member authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorClock {
  counters: BTreeMap<String, u64>,
}

impl VectorClock {
  /// Creates an empty vector clock.
  #[must_use]
  pub const fn new() -> Self {
    Self { counters: BTreeMap::new() }
  }

  /// Records the observed version for a member.
  pub fn observe(&mut self, authority: &str, version: u64) {
    let current = self.counters.get(authority).copied().unwrap_or(0);
    if version > current {
      self.counters.insert(authority.to_string(), version);
    }
  }

  /// Returns the counter value for a member.
  #[must_use]
  pub fn value(&self, authority: &str) -> u64 {
    self.counters.get(authority).copied().unwrap_or(0)
  }

  /// Merges all entries from another clock by taking the max value per member.
  pub fn merge(&mut self, other: &Self) {
    for (authority, value) in other.counters.iter() {
      self.observe(authority, *value);
    }
  }

  /// Returns true when every peer has observed at least the provided version.
  #[must_use]
  pub fn has_seen_all(&self, peers: &[String], version: u64) -> bool {
    peers.iter().all(|peer| self.value(peer) >= version)
  }
}

impl Default for VectorClock {
  fn default() -> Self {
    Self::new()
  }
}
