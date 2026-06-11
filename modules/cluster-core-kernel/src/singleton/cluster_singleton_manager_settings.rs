//! Cluster Singleton manager settings.

use alloc::{string::String, vec::Vec};
use core::time::Duration;

use super::{ClusterSingletonSettingsError, LeaseUsageSettings};

#[cfg(test)]
#[path = "cluster_singleton_manager_settings_test.rs"]
mod tests;

/// Configuration for the Cluster Singleton manager.
///
/// Holds the operating parameters for the singleton manager with
/// Pekko-compatible defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterSingletonManagerSettings {
  singleton_name:           String,
  role:                     Option<String>,
  removal_margin:           Option<Duration>,
  hand_over_retry_interval: Duration,
  min_hand_over_retries:    u32,
  lease_settings:           Option<LeaseUsageSettings>,
}

impl ClusterSingletonManagerSettings {
  /// Creates a new `ClusterSingletonManagerSettings` with Pekko-compatible defaults.
  ///
  /// Defaults: singleton name `"singleton"`, no role constraint, removal margin
  /// unset (`None`), hand-over retry interval 1 s, minimum hand-over retries 15,
  /// no lease slot.
  #[must_use]
  pub fn new() -> Self {
    Self {
      singleton_name:           String::from("singleton"),
      role:                     None,
      removal_margin:           None,
      hand_over_retry_interval: Duration::from_secs(1),
      min_hand_over_retries:    15,
      lease_settings:           None,
    }
  }

  /// Sets the singleton name.
  #[must_use]
  pub fn with_singleton_name(mut self, name: &str) -> Self {
    self.singleton_name = String::from(name);
    self
  }

  /// Sets the cluster role that constrains singleton placement.
  #[must_use]
  pub fn with_role(mut self, role: &str) -> Self {
    self.role = Some(String::from(role));
    self
  }

  /// Sets the removal margin.
  ///
  /// Passing an explicit value distinguishes this from the unset state (`None`),
  /// which means "follow the downing side's removal margin" (requirement 1.3).
  #[must_use]
  pub const fn with_removal_margin(mut self, margin: Duration) -> Self {
    self.removal_margin = Some(margin);
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

  /// Sets the lease usage settings slot.
  #[must_use]
  pub fn with_lease_settings(mut self, lease: LeaseUsageSettings) -> Self {
    self.lease_settings = Some(lease);
    self
  }

  /// Returns the singleton name.
  #[must_use]
  pub fn singleton_name(&self) -> &str {
    &self.singleton_name
  }

  /// Returns the role constraint, if any.
  #[must_use]
  pub fn role(&self) -> Option<&str> {
    self.role.as_deref()
  }

  /// Returns the removal margin, if explicitly set.
  ///
  /// `None` means the downing side's removal margin is used (requirement 1.3).
  #[must_use]
  pub const fn removal_margin(&self) -> Option<Duration> {
    self.removal_margin
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

  /// Returns a reference to the lease usage settings, if configured.
  #[must_use]
  pub const fn lease_settings(&self) -> Option<&LeaseUsageSettings> {
    self.lease_settings.as_ref()
  }

  /// Validates this manager settings.
  ///
  /// # Errors
  ///
  /// - [`ClusterSingletonSettingsError::EmptySingletonName`] when the singleton name is empty.
  /// - [`ClusterSingletonSettingsError::NonPositiveHandOverRetryInterval`] when the hand-over retry
  ///   interval is zero.
  /// - Delegates to [`LeaseUsageSettings::validate`] when a lease slot is present.
  pub fn validate(&self) -> Result<(), ClusterSingletonSettingsError> {
    if self.singleton_name.is_empty() {
      return Err(ClusterSingletonSettingsError::EmptySingletonName);
    }
    if self.hand_over_retry_interval == Duration::ZERO {
      return Err(ClusterSingletonSettingsError::NonPositiveHandOverRetryInterval);
    }
    if let Some(lease) = &self.lease_settings {
      lease.validate()?;
    }
    Ok(())
  }

  /// Returns the maximum number of hand-over retries.
  ///
  /// Derived deterministically from `min_hand_over_retries` and the removal margin:
  ///
  /// ```text
  /// max_hand_over_retries = max(min_hand_over_retries, margin_ticks + 3)
  /// margin_ticks = removal_margin (None treated as 0) / hand_over_retry_interval
  ///                (integer division in milliseconds, truncating)
  /// ```
  ///
  /// When `hand_over_retry_interval` is zero, `margin_ticks` is treated as 0 to
  /// avoid division by zero (this function is total and never panics).
  #[must_use]
  pub fn max_hand_over_retries(&self) -> u32 {
    let margin_millis = self.removal_margin.unwrap_or(Duration::ZERO).as_millis();
    let interval_millis = self.hand_over_retry_interval.as_millis();

    // checked_div はゼロ除算時に None を返す。None は margin_ticks = 0 として扱う
    // u128 → u32 への変換は現実的なパラメータ範囲内で安全
    let margin_ticks: u32 = margin_millis.checked_div(interval_millis).unwrap_or(0).try_into().unwrap_or(u32::MAX);

    let candidate = margin_ticks.saturating_add(3);
    self.min_hand_over_retries.max(candidate)
  }

  /// Returns the names of fields whose values differ from another settings instance.
  ///
  /// Used by join compatibility checks to enumerate mismatched fields.
  #[must_use]
  pub fn difference_field_names(&self, other: &Self) -> Vec<&'static str> {
    let mut names = Vec::new();

    if self.singleton_name != other.singleton_name {
      names.push("singleton_name");
    }
    if self.role != other.role {
      names.push("role");
    }
    if self.removal_margin != other.removal_margin {
      names.push("removal_margin");
    }
    if self.hand_over_retry_interval != other.hand_over_retry_interval {
      names.push("hand_over_retry_interval");
    }
    if self.min_hand_over_retries != other.min_hand_over_retries {
      names.push("min_hand_over_retries");
    }
    if self.lease_settings != other.lease_settings {
      names.push("lease_settings");
    }

    names
  }
}

impl Default for ClusterSingletonManagerSettings {
  fn default() -> Self {
    Self::new()
  }
}
