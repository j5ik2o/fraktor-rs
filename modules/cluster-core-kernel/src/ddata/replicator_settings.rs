//! Configuration for the distributed-data Replicator runtime.

#[cfg(test)]
#[path = "replicator_settings_test.rs"]
mod tests;

use alloc::string::String;
use core::time::Duration;

/// Operating parameters for a distributed-data Replicator instance.
///
/// Defaults follow Pekko `pekko.cluster.distributed-data` configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplicatorSettings {
  role: Option<String>,
  gossip_interval: Duration,
  notify_subscribers_interval: Duration,
  max_delta_elements: u32,
  prefer_oldest: bool,
  actor_name: String,
}

impl ReplicatorSettings {
  /// Creates settings with Pekko-compatible defaults.
  #[must_use]
  pub fn new() -> Self {
    Self {
      role: None,
      gossip_interval: Duration::from_secs(2),
      notify_subscribers_interval: Duration::from_millis(500),
      max_delta_elements: 1000,
      prefer_oldest: true,
      actor_name: String::from("ddataReplicator"),
    }
  }

  /// Sets the cluster role constraint for replica placement.
  #[must_use]
  pub fn with_role(mut self, role: &str) -> Self {
    self.role = Some(String::from(role));
    self
  }

  /// Sets the gossip interval.
  #[must_use]
  pub const fn with_gossip_interval(mut self, interval: Duration) -> Self {
    self.gossip_interval = interval;
    self
  }

  /// Sets the subscriber notification interval.
  #[must_use]
  pub const fn with_notify_subscribers_interval(mut self, interval: Duration) -> Self {
    self.notify_subscribers_interval = interval;
    self
  }

  /// Sets the maximum number of delta elements transferred per gossip round.
  #[must_use]
  pub const fn with_max_delta_elements(mut self, max_delta_elements: u32) -> Self {
    self.max_delta_elements = max_delta_elements;
    self
  }

  /// Sets whether read/write operations prefer oldest members first.
  #[must_use]
  pub const fn with_prefer_oldest(mut self, prefer_oldest: bool) -> Self {
    self.prefer_oldest = prefer_oldest;
    self
  }

  /// Sets the actor name used when the Replicator is started.
  #[must_use]
  pub fn with_actor_name(mut self, name: &str) -> Self {
    self.actor_name = String::from(name);
    self
  }

  /// Returns the role constraint, if any.
  #[must_use]
  pub fn role(&self) -> Option<&str> {
    self.role.as_deref()
  }

  /// Returns the gossip interval.
  #[must_use]
  pub const fn gossip_interval(&self) -> Duration {
    self.gossip_interval
  }

  /// Returns the subscriber notification interval.
  #[must_use]
  pub const fn notify_subscribers_interval(&self) -> Duration {
    self.notify_subscribers_interval
  }

  /// Returns the maximum number of delta elements per gossip round.
  #[must_use]
  pub const fn max_delta_elements(&self) -> u32 {
    self.max_delta_elements
  }

  /// Returns whether read/write operations prefer oldest members first.
  #[must_use]
  pub const fn prefer_oldest(&self) -> bool {
    self.prefer_oldest
  }

  /// Returns the configured actor name.
  #[must_use]
  pub fn actor_name(&self) -> &str {
    &self.actor_name
  }

  /// Validates these settings.
  ///
  /// # Errors
  ///
  /// Returns [`ReplicatorSettingsError`] when a field is invalid.
  pub fn validate(&self) -> Result<(), ReplicatorSettingsError> {
    if self.actor_name.is_empty() {
      return Err(ReplicatorSettingsError::EmptyActorName);
    }
    if self.gossip_interval == Duration::ZERO {
      return Err(ReplicatorSettingsError::NonPositiveGossipInterval);
    }
    if self.notify_subscribers_interval == Duration::ZERO {
      return Err(ReplicatorSettingsError::NonPositiveNotifySubscribersInterval);
    }
    if self.max_delta_elements == 0 {
      return Err(ReplicatorSettingsError::NonPositiveMaxDeltaElements);
    }
    Ok(())
  }
}

impl Default for ReplicatorSettings {
  fn default() -> Self {
    Self::new()
  }
}

/// Validation errors for [`ReplicatorSettings`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplicatorSettingsError {
  /// Actor name is empty.
  EmptyActorName,
  /// Gossip interval is zero.
  NonPositiveGossipInterval,
  /// Subscriber notification interval is zero.
  NonPositiveNotifySubscribersInterval,
  /// Maximum delta elements is zero.
  NonPositiveMaxDeltaElements,
}
