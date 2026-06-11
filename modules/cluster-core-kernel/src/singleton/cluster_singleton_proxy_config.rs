//! Cluster Singleton proxy configuration.

use alloc::{string::String, vec::Vec};
use core::time::Duration;

use super::ClusterSingletonConfigError;
use crate::membership::DataCenter;

#[cfg(test)]
#[path = "cluster_singleton_proxy_config_test.rs"]
mod tests;

/// Configuration for the Cluster Singleton proxy.
///
/// Holds the operating parameters for the singleton proxy with
/// Pekko-compatible defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterSingletonProxyConfig {
  singleton_name: String,
  role: Option<String>,
  data_center: Option<DataCenter>,
  singleton_identification_interval: Duration,
  buffer_size: u32,
}

impl ClusterSingletonProxyConfig {
  /// Creates a new `ClusterSingletonProxyConfig` with Pekko-compatible defaults.
  ///
  /// Defaults: singleton name `"singleton"`, no role constraint, no data center
  /// constraint, identification interval 1 s, buffer size 1000.
  #[must_use]
  pub fn new() -> Self {
    Self {
      singleton_name: String::from("singleton"),
      role: None,
      data_center: None,
      singleton_identification_interval: Duration::from_secs(1),
      buffer_size: 1000,
    }
  }

  /// Sets the singleton name.
  #[must_use]
  pub fn with_singleton_name(mut self, name: &str) -> Self {
    self.singleton_name = String::from(name);
    self
  }

  /// Sets the cluster role that constrains singleton proxy routing.
  #[must_use]
  pub fn with_role(mut self, role: &str) -> Self {
    self.role = Some(String::from(role));
    self
  }

  /// Sets the data center constraint for singleton lookup.
  #[must_use]
  pub fn with_data_center(mut self, data_center: DataCenter) -> Self {
    self.data_center = Some(data_center);
    self
  }

  /// Sets the singleton identification interval.
  #[must_use]
  pub const fn with_singleton_identification_interval(mut self, interval: Duration) -> Self {
    self.singleton_identification_interval = interval;
    self
  }

  /// Sets the proxy message buffer size.
  ///
  /// A value of 0 means "no buffering" and is a valid configuration
  /// (requirement 2.3).
  #[must_use]
  pub const fn with_buffer_size(mut self, buffer_size: u32) -> Self {
    self.buffer_size = buffer_size;
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

  /// Returns the data center constraint, if any.
  #[must_use]
  pub const fn data_center(&self) -> Option<&DataCenter> {
    self.data_center.as_ref()
  }

  /// Returns the singleton identification interval.
  #[must_use]
  pub const fn singleton_identification_interval(&self) -> Duration {
    self.singleton_identification_interval
  }

  /// Returns the proxy message buffer size.
  ///
  /// A value of 0 means "no buffering" (requirement 2.3).
  #[must_use]
  pub const fn buffer_size(&self) -> u32 {
    self.buffer_size
  }

  /// Validates this proxy configuration.
  ///
  /// # Errors
  ///
  /// - [`ClusterSingletonConfigError::EmptySingletonName`] when the singleton name is empty.
  /// - [`ClusterSingletonConfigError::NonPositiveIdentificationInterval`] when the identification
  ///   interval is zero.
  /// - [`ClusterSingletonConfigError::BufferSizeOutOfRange`] when buffer size exceeds 10000. Note:
  ///   buffer size 0 is accepted as "no buffering" (requirement 2.3).
  pub fn validate(&self) -> Result<(), ClusterSingletonConfigError> {
    if self.singleton_name.is_empty() {
      return Err(ClusterSingletonConfigError::EmptySingletonName);
    }
    if self.singleton_identification_interval == Duration::ZERO {
      return Err(ClusterSingletonConfigError::NonPositiveIdentificationInterval);
    }
    if self.buffer_size > 10000 {
      return Err(ClusterSingletonConfigError::BufferSizeOutOfRange { value: self.buffer_size });
    }
    Ok(())
  }

  /// Returns the names of fields whose values differ from another configuration instance.
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
    if self.data_center != other.data_center {
      names.push("data_center");
    }
    if self.singleton_identification_interval != other.singleton_identification_interval {
      names.push("singleton_identification_interval");
    }
    if self.buffer_size != other.buffer_size {
      names.push("buffer_size");
    }

    names
  }
}

impl Default for ClusterSingletonProxyConfig {
  fn default() -> Self {
    Self::new()
  }
}
