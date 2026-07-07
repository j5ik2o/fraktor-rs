//! Comprehensive cluster sharding settings contract.

use alloc::string::String;
use core::time::Duration;

use super::{ClusterShardingSettingsError, PassivationStrategy};

#[cfg(test)]
#[path = "cluster_sharding_settings_test.rs"]
mod tests;

/// Comprehensive settings for cluster sharding behavior.
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterShardingSettings {
  number_of_shards: u32,
  role: Option<String>,
  passivation_strategy: PassivationStrategy,
  remember_entities: bool,
  hand_over_retry_interval: Duration,
  min_hand_over_retries: u32,
  retry_interval: Duration,
  rebalance_interval: Duration,
  hand_off_timeout: Duration,
  shard_region_query_timeout: Duration,
  buffer_size: u32,
  entity_restart_backoff: Duration,
}

impl ClusterShardingSettings {
  /// Creates settings with Pekko-compatible defaults.
  #[must_use]
  pub fn new() -> Self {
    Self {
      number_of_shards: 100,
      role: None,
      passivation_strategy: PassivationStrategy::default(),
      remember_entities: false,
      hand_over_retry_interval: Duration::from_secs(1),
      min_hand_over_retries: 15,
      retry_interval: Duration::from_secs(2),
      rebalance_interval: Duration::from_secs(10),
      hand_off_timeout: Duration::from_secs(60),
      shard_region_query_timeout: Duration::from_secs(3),
      buffer_size: 100_000,
      entity_restart_backoff: Duration::from_secs(10),
    }
  }

  /// Sets the number of shards used for entity-to-shard mapping.
  #[must_use]
  pub const fn with_number_of_shards(mut self, number_of_shards: u32) -> Self {
    self.number_of_shards = number_of_shards;
    self
  }

  /// Sets the cluster role required to host shard regions.
  #[must_use]
  pub fn with_role(mut self, role: impl Into<String>) -> Self {
    self.role = Some(role.into());
    self
  }

  /// Clears the cluster role constraint.
  #[must_use]
  pub fn without_role(mut self) -> Self {
    self.role = None;
    self
  }

  /// Sets the passivation strategy.
  #[must_use]
  pub fn with_passivation_strategy(mut self, strategy: PassivationStrategy) -> Self {
    self.passivation_strategy = strategy;
    self
  }

  /// Sets whether entities are remembered across shard restarts.
  #[must_use]
  pub const fn with_remember_entities(mut self, remember_entities: bool) -> Self {
    self.remember_entities = remember_entities;
    self
  }

  /// Sets the hand-over retry interval.
  #[must_use]
  pub const fn with_hand_over_retry_interval(mut self, interval: Duration) -> Self {
    self.hand_over_retry_interval = interval;
    self
  }

  /// Sets the minimum number of hand-over retries.
  #[must_use]
  pub const fn with_min_hand_over_retries(mut self, retries: u32) -> Self {
    self.min_hand_over_retries = retries;
    self
  }

  /// Sets the coordinator retry interval.
  #[must_use]
  pub const fn with_retry_interval(mut self, interval: Duration) -> Self {
    self.retry_interval = interval;
    self
  }

  /// Sets the rebalance interval.
  #[must_use]
  pub const fn with_rebalance_interval(mut self, interval: Duration) -> Self {
    self.rebalance_interval = interval;
    self
  }

  /// Sets the shard hand-off timeout.
  #[must_use]
  pub const fn with_hand_off_timeout(mut self, timeout: Duration) -> Self {
    self.hand_off_timeout = timeout;
    self
  }

  /// Sets the shard region query timeout.
  #[must_use]
  pub const fn with_shard_region_query_timeout(mut self, timeout: Duration) -> Self {
    self.shard_region_query_timeout = timeout;
    self
  }

  /// Sets the coordinator buffer size.
  #[must_use]
  pub const fn with_buffer_size(mut self, buffer_size: u32) -> Self {
    self.buffer_size = buffer_size;
    self
  }

  /// Sets the entity restart backoff interval.
  #[must_use]
  pub const fn with_entity_restart_backoff(mut self, backoff: Duration) -> Self {
    self.entity_restart_backoff = backoff;
    self
  }

  /// Returns the configured number of shards.
  #[must_use]
  pub const fn number_of_shards(&self) -> u32 {
    self.number_of_shards
  }

  /// Returns the configured cluster role, if any.
  #[must_use]
  pub fn role(&self) -> Option<&str> {
    self.role.as_deref()
  }

  /// Returns the passivation strategy.
  #[must_use]
  pub const fn passivation_strategy(&self) -> &PassivationStrategy {
    &self.passivation_strategy
  }

  /// Returns whether entities are remembered across shard restarts.
  #[must_use]
  pub const fn remember_entities(&self) -> bool {
    self.remember_entities
  }

  /// Returns the hand-over retry interval.
  #[must_use]
  pub const fn hand_over_retry_interval(&self) -> Duration {
    self.hand_over_retry_interval
  }

  /// Returns the minimum number of hand-over retries.
  #[must_use]
  pub const fn min_hand_over_retries(&self) -> u32 {
    self.min_hand_over_retries
  }

  /// Returns the coordinator retry interval.
  #[must_use]
  pub const fn retry_interval(&self) -> Duration {
    self.retry_interval
  }

  /// Returns the rebalance interval.
  #[must_use]
  pub const fn rebalance_interval(&self) -> Duration {
    self.rebalance_interval
  }

  /// Returns the shard hand-off timeout.
  #[must_use]
  pub const fn hand_off_timeout(&self) -> Duration {
    self.hand_off_timeout
  }

  /// Returns the shard region query timeout.
  #[must_use]
  pub const fn shard_region_query_timeout(&self) -> Duration {
    self.shard_region_query_timeout
  }

  /// Returns the coordinator buffer size.
  #[must_use]
  pub const fn buffer_size(&self) -> u32 {
    self.buffer_size
  }

  /// Returns the entity restart backoff interval.
  #[must_use]
  pub const fn entity_restart_backoff(&self) -> Duration {
    self.entity_restart_backoff
  }

  /// Validates sharding settings.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterShardingSettingsError`] when a configured value is outside the accepted
  /// range.
  pub fn validate(&self) -> Result<(), ClusterShardingSettingsError> {
    if self.number_of_shards == 0 {
      return Err(ClusterShardingSettingsError::ZeroNumberOfShards);
    }
    if self.hand_over_retry_interval == Duration::ZERO {
      return Err(ClusterShardingSettingsError::ZeroHandOverRetryInterval);
    }
    if self.retry_interval == Duration::ZERO {
      return Err(ClusterShardingSettingsError::ZeroRetryInterval);
    }
    if self.rebalance_interval == Duration::ZERO {
      return Err(ClusterShardingSettingsError::ZeroRebalanceInterval);
    }
    if self.shard_region_query_timeout == Duration::ZERO {
      return Err(ClusterShardingSettingsError::ZeroShardRegionQueryTimeout);
    }
    if self.hand_off_timeout == Duration::ZERO {
      return Err(ClusterShardingSettingsError::ZeroHandOffTimeout);
    }
    if self.entity_restart_backoff == Duration::ZERO {
      return Err(ClusterShardingSettingsError::ZeroEntityRestartBackoff);
    }
    if self.remember_entities && !self.passivation_strategy.is_disabled() {
      return Err(ClusterShardingSettingsError::PassivationWithRememberEntities);
    }

    self.passivation_strategy.validate()
  }
}

impl Default for ClusterShardingSettings {
  fn default() -> Self {
    Self::new()
  }
}
