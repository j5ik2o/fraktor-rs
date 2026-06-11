//! Typed unified configuration for Cluster Singleton manager and proxy.

#[cfg(test)]
#[path = "cluster_singleton_config_test.rs"]
mod tests;

use alloc::string::String;
use core::time::Duration;

use fraktor_cluster_core_kernel_rs::{
  membership::DataCenter,
  singleton::{ClusterSingletonManagerConfig, ClusterSingletonProxyConfig, LeaseUsageConfig},
};

/// Unified configuration for Cluster Singleton that covers both manager and proxy configuration.
///
/// Provides a single place to specify all shared parameters and derives
/// manager / proxy configurations by supplying a singleton name.
///
/// Validation is delegated to [`ClusterSingletonManagerConfig::validate`] and
/// [`ClusterSingletonProxyConfig::validate`] — this type does not duplicate
/// those rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterSingletonConfig {
  role: Option<String>,
  data_center: Option<DataCenter>,
  singleton_identification_interval: Duration,
  removal_margin: Option<Duration>,
  hand_over_retry_interval: Duration,
  min_hand_over_retries: u32,
  buffer_size: u32,
  lease_config: Option<LeaseUsageConfig>,
}

impl ClusterSingletonConfig {
  /// Creates a new `ClusterSingletonConfig` with Pekko-compatible defaults.
  ///
  /// Defaults are identical to those of [`ClusterSingletonManagerConfig::new`] and
  /// [`ClusterSingletonProxyConfig::new`]: no role, no data center, identification
  /// interval 1 s, removal margin unset, hand-over retry interval 1 s, 15 minimum
  /// retries, buffer size 1000, no lease slot.
  #[must_use]
  pub fn new() -> Self {
    Self {
      role: None,
      data_center: None,
      singleton_identification_interval: Duration::from_secs(1),
      removal_margin: None,
      hand_over_retry_interval: Duration::from_secs(1),
      min_hand_over_retries: 15,
      buffer_size: 1000,
      lease_config: None,
    }
  }

  /// Sets the cluster role constraint.
  #[must_use]
  pub fn with_role(mut self, role: &str) -> Self {
    self.role = Some(String::from(role));
    self
  }

  /// Sets the data center constraint (proxy only).
  #[must_use]
  pub fn with_data_center(mut self, data_center: DataCenter) -> Self {
    self.data_center = Some(data_center);
    self
  }

  /// Sets the singleton identification interval (proxy only).
  #[must_use]
  pub const fn with_singleton_identification_interval(mut self, interval: Duration) -> Self {
    self.singleton_identification_interval = interval;
    self
  }

  /// Sets the removal margin (manager only).
  #[must_use]
  pub const fn with_removal_margin(mut self, margin: Duration) -> Self {
    self.removal_margin = Some(margin);
    self
  }

  /// Sets the hand-over retry interval (manager only).
  #[must_use]
  pub const fn with_hand_over_retry_interval(mut self, interval: Duration) -> Self {
    self.hand_over_retry_interval = interval;
    self
  }

  /// Sets the minimum number of hand-over retries (manager only).
  #[must_use]
  pub const fn with_min_hand_over_retries(mut self, retries: u32) -> Self {
    self.min_hand_over_retries = retries;
    self
  }

  /// Sets the proxy message buffer size.
  #[must_use]
  pub const fn with_buffer_size(mut self, buffer_size: u32) -> Self {
    self.buffer_size = buffer_size;
    self
  }

  /// Sets the lease usage configuration slot (manager only).
  #[must_use]
  pub fn with_lease_config(mut self, lease: LeaseUsageConfig) -> Self {
    self.lease_config = Some(lease);
    self
  }

  /// Derives a [`ClusterSingletonManagerConfig`] for the given singleton name.
  ///
  /// Manager-only fields (`removal_margin`, `hand_over_retry_interval`,
  /// `min_hand_over_retries`, `lease_config`) are carried over unchanged.
  /// Proxy-only fields (`data_center`, `singleton_identification_interval`,
  /// `buffer_size`) have no effect on the derived manager configuration.
  ///
  /// # Postconditions
  ///
  /// Every field present in `ClusterSingletonManagerConfig` has the same value as the
  /// corresponding field in this configuration instance (requirement 3.2).
  #[must_use]
  pub fn to_manager_config(&self, singleton_name: &str) -> ClusterSingletonManagerConfig {
    // singleton_name は to_manager_config の引数で上書きする
    let mut config = ClusterSingletonManagerConfig::new().with_singleton_name(singleton_name);

    if let Some(ref role) = self.role {
      config = config.with_role(role.as_str());
    }
    if let Some(margin) = self.removal_margin {
      config = config.with_removal_margin(margin);
    }
    config = config
      .with_hand_over_retry_interval(self.hand_over_retry_interval)
      .with_min_hand_over_retries(self.min_hand_over_retries);
    if let Some(lease) = self.lease_config.clone() {
      config = config.with_lease_config(lease);
    }
    config
  }

  /// Derives a [`ClusterSingletonProxyConfig`] for the given singleton name.
  ///
  /// Proxy-only fields (`data_center`, `singleton_identification_interval`, `buffer_size`)
  /// are carried over unchanged. Manager-only fields (`removal_margin`,
  /// `hand_over_retry_interval`, `min_hand_over_retries`, `lease_config`) have no
  /// effect on the derived proxy configuration.
  ///
  /// # Postconditions
  ///
  /// Every field present in `ClusterSingletonProxyConfig` has the same value as the
  /// corresponding field in this configuration instance (requirement 3.3).
  #[must_use]
  pub fn to_proxy_config(&self, singleton_name: &str) -> ClusterSingletonProxyConfig {
    let mut config = ClusterSingletonProxyConfig::new().with_singleton_name(singleton_name);

    if let Some(ref role) = self.role {
      config = config.with_role(role.as_str());
    }
    if let Some(ref dc) = self.data_center {
      config = config.with_data_center(dc.clone());
    }
    config = config
      .with_singleton_identification_interval(self.singleton_identification_interval)
      .with_buffer_size(self.buffer_size);
    config
  }
}

impl Default for ClusterSingletonConfig {
  fn default() -> Self {
    Self::new()
  }
}
