//! Configuration for AWS ECS cluster provider.

use std::{string::String, time::Duration};

use aws_config::{BehaviorVersion, defaults};
use aws_sdk_ecs::{Client as EcsClient, config::Region};

/// Configuration for AWS ECS cluster provider.
#[derive(Clone, Debug)]
pub struct EcsClusterConfig {
  cluster_name:  String,
  service_name:  Option<String>,
  poll_interval: Duration,
  port:          u16,
  region:        Option<String>,
}

impl Default for EcsClusterConfig {
  fn default() -> Self {
    Self::new()
  }
}

impl EcsClusterConfig {
  /// Creates a new ECS cluster configuration with default values.
  #[must_use]
  pub fn new() -> Self {
    Self {
      cluster_name:  String::new(),
      service_name:  None,
      poll_interval: Duration::from_secs(30),
      port:          8080,
      region:        None,
    }
  }

  /// Sets the ECS cluster name.
  #[must_use]
  pub fn with_cluster_name(mut self, name: impl Into<String>) -> Self {
    self.cluster_name = name.into();
    self
  }

  /// Sets the ECS service name for filtering tasks.
  #[must_use]
  pub fn with_service_name(mut self, name: impl Into<String>) -> Self {
    self.service_name = Some(name.into());
    self
  }

  /// Sets the polling interval for task discovery.
  #[must_use]
  pub const fn with_poll_interval(mut self, interval: Duration) -> Self {
    self.poll_interval = interval;
    self
  }

  /// Sets the port used for cluster communication.
  #[must_use]
  pub const fn with_port(mut self, port: u16) -> Self {
    self.port = port;
    self
  }

  /// Sets the AWS region.
  #[must_use]
  pub fn with_region(mut self, region: impl Into<String>) -> Self {
    self.region = Some(region.into());
    self
  }

  /// Returns the cluster name.
  #[must_use]
  pub fn cluster_name(&self) -> &str {
    &self.cluster_name
  }

  /// Returns the service name.
  #[must_use]
  pub fn service_name(&self) -> Option<&str> {
    self.service_name.as_deref()
  }

  /// Returns the polling interval.
  #[must_use]
  pub const fn poll_interval(&self) -> Duration {
    self.poll_interval
  }

  /// Returns the port.
  #[must_use]
  pub const fn port(&self) -> u16 {
    self.port
  }

  /// Returns the region.
  #[must_use]
  pub fn region(&self) -> Option<&str> {
    self.region.as_deref()
  }

  pub(super) async fn create_client(&self) -> EcsClient {
    let mut aws_config_builder = defaults(BehaviorVersion::latest());
    if let Some(ref region) = self.region {
      aws_config_builder = aws_config_builder.region(Region::new(region.clone()));
    }
    let aws_config = aws_config_builder.load().await;
    EcsClient::new(&aws_config)
  }
}
