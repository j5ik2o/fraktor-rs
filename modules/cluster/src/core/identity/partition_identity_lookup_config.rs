//! Configuration for partition identity lookup.

#[cfg(test)]
mod tests;

/// Configuration for the partition identity lookup component.
#[derive(Debug, Clone)]
pub struct PartitionIdentityLookupConfig {
  /// Maximum number of entries in the PID cache.
  cache_capacity: usize,
  /// Time-to-live for cached PIDs in seconds.
  pid_ttl_secs:   u64,
  /// Time-to-live for idle activations in seconds.
  idle_ttl_secs:  u64,
}

impl PartitionIdentityLookupConfig {
  /// Creates a new configuration with the specified parameters.
  ///
  /// # Arguments
  ///
  /// * `cache_capacity` - Maximum number of entries in the PID cache
  /// * `pid_ttl_secs` - Time-to-live for cached PIDs in seconds
  /// * `idle_ttl_secs` - Time-to-live for idle activations in seconds
  #[must_use]
  pub const fn new(cache_capacity: usize, pid_ttl_secs: u64, idle_ttl_secs: u64) -> Self {
    Self { cache_capacity, pid_ttl_secs, idle_ttl_secs }
  }

  /// Returns the cache capacity.
  #[must_use]
  pub const fn cache_capacity(&self) -> usize {
    self.cache_capacity
  }

  /// Returns the PID TTL in seconds.
  #[must_use]
  pub const fn pid_ttl_secs(&self) -> u64 {
    self.pid_ttl_secs
  }

  /// Returns the idle TTL in seconds.
  #[must_use]
  pub const fn idle_ttl_secs(&self) -> u64 {
    self.idle_ttl_secs
  }
}

impl Default for PartitionIdentityLookupConfig {
  fn default() -> Self {
    Self {
      cache_capacity: 1024,
      pid_ttl_secs:   300,  // 5 minutes
      idle_ttl_secs:  3600, // 1 hour
    }
  }
}
