//! Typed unified settings for Cluster Singleton manager and proxy.

#[cfg(test)]
#[path = "cluster_singleton_settings_test.rs"]
mod tests;

use alloc::string::String;
use core::time::Duration;

use fraktor_cluster_core_kernel_rs::{
  membership::DataCenter,
  singleton::{ClusterSingletonManagerSettings, ClusterSingletonProxySettings, LeaseUsageSettings},
};

/// Unified settings for Cluster Singleton that covers both manager and proxy configuration.
///
/// Provides a single place to specify all shared parameters and derives
/// manager / proxy settings by supplying a singleton name.
///
/// Validation is delegated to [`ClusterSingletonManagerSettings::validate`] and
/// [`ClusterSingletonProxySettings::validate`] — this type does not duplicate
/// those rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterSingletonSettings {
  role: Option<String>,
  data_center: Option<DataCenter>,
  singleton_identification_interval: Duration,
  removal_margin: Option<Duration>,
  hand_over_retry_interval: Duration,
  min_hand_over_retries: u32,
  buffer_size: u32,
  lease_settings: Option<LeaseUsageSettings>,
}

impl ClusterSingletonSettings {
  /// Creates a new `ClusterSingletonSettings` with Pekko-compatible defaults.
  ///
  /// Defaults are identical to those of [`ClusterSingletonManagerSettings::new`] and
  /// [`ClusterSingletonProxySettings::new`]: no role, no data center, identification
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
      lease_settings: None,
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

  /// Sets the lease usage settings slot (manager only).
  #[must_use]
  pub fn with_lease_settings(mut self, lease: LeaseUsageSettings) -> Self {
    self.lease_settings = Some(lease);
    self
  }

  /// Derives a [`ClusterSingletonManagerSettings`] for the given singleton name.
  ///
  /// Manager-only fields (`removal_margin`, `hand_over_retry_interval`,
  /// `min_hand_over_retries`, `lease_settings`) are carried over unchanged.
  /// Proxy-only fields (`data_center`, `singleton_identification_interval`,
  /// `buffer_size`) have no effect on the derived manager settings.
  ///
  /// # Postconditions
  ///
  /// Every field present in `ClusterSingletonManagerSettings` has the same value as the
  /// corresponding field in this settings instance (requirement 3.2).
  #[must_use]
  pub fn to_manager_settings(&self, singleton_name: &str) -> ClusterSingletonManagerSettings {
    // singleton_name は to_manager_settings の引数で上書きする
    let mut settings = ClusterSingletonManagerSettings::new().with_singleton_name(singleton_name);

    if let Some(ref role) = self.role {
      settings = settings.with_role(role.as_str());
    }
    if let Some(margin) = self.removal_margin {
      settings = settings.with_removal_margin(margin);
    }
    settings = settings
      .with_hand_over_retry_interval(self.hand_over_retry_interval)
      .with_min_hand_over_retries(self.min_hand_over_retries);
    if let Some(lease) = self.lease_settings.clone() {
      settings = settings.with_lease_settings(lease);
    }
    settings
  }

  /// Derives a [`ClusterSingletonProxySettings`] for the given singleton name.
  ///
  /// Proxy-only fields (`data_center`, `singleton_identification_interval`, `buffer_size`)
  /// are carried over unchanged. Manager-only fields (`removal_margin`,
  /// `hand_over_retry_interval`, `min_hand_over_retries`, `lease_settings`) have no
  /// effect on the derived proxy settings.
  ///
  /// # Postconditions
  ///
  /// Every field present in `ClusterSingletonProxySettings` has the same value as the
  /// corresponding field in this settings instance (requirement 3.3).
  #[must_use]
  pub fn to_proxy_settings(&self, singleton_name: &str) -> ClusterSingletonProxySettings {
    let mut settings = ClusterSingletonProxySettings::new().with_singleton_name(singleton_name);

    if let Some(ref role) = self.role {
      settings = settings.with_role(role.as_str());
    }
    if let Some(ref dc) = self.data_center {
      settings = settings.with_data_center(dc.clone());
    }
    settings = settings
      .with_singleton_identification_interval(self.singleton_identification_interval)
      .with_buffer_size(self.buffer_size);
    settings
  }
}

impl Default for ClusterSingletonSettings {
  fn default() -> Self {
    Self::new()
  }
}
