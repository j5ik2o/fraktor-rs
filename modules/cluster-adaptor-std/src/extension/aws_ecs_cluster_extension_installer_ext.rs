//! AWS ECS installer extension trait.

#[cfg(test)]
#[path = "aws_ecs_cluster_extension_installer_ext_test.rs"]
mod tests;

use alloc::boxed::Box;

use fraktor_cluster_core_kernel_rs::extension::{ClusterExtensionConfig, ClusterExtensionInstaller};

use crate::cluster_provider::{AwsEcsClusterProvider, EcsClusterConfig};

/// Extension methods for creating cluster installers backed by AWS ECS discovery.
pub trait AwsEcsClusterExtensionInstallerExt {
  /// Creates a new installer with `AwsEcsClusterProvider`.
  ///
  /// This is a convenience constructor for AWS ECS environments where task discovery
  /// is performed via the ECS API (ListTasks + DescribeTasks).
  ///
  /// # Example
  ///
  /// ```rust
  /// use std::time::Duration;
  ///
  /// use fraktor_cluster_adaptor_std_rs::{
  ///   cluster_provider::EcsClusterConfig, extension::AwsEcsClusterExtensionInstallerExt,
  /// };
  /// use fraktor_cluster_core_kernel_rs::extension::{
  ///   ClusterExtensionConfig, ClusterExtensionInstaller,
  /// };
  ///
  /// let config = ClusterExtensionConfig::default().with_advertised_address("10.0.0.1:8080");
  /// let ecs_config = EcsClusterConfig::new()
  ///   .with_cluster_name("my-cluster")
  ///   .with_service_name("my-service")
  ///   .with_poll_interval(Duration::from_secs(10));
  ///
  /// let installer = ClusterExtensionInstaller::new_with_ecs(config, ecs_config);
  /// ```
  #[must_use]
  fn new_with_ecs(config: ClusterExtensionConfig, ecs_config: EcsClusterConfig) -> ClusterExtensionInstaller;
}

impl AwsEcsClusterExtensionInstallerExt for ClusterExtensionInstaller {
  fn new_with_ecs(config: ClusterExtensionConfig, ecs_config: EcsClusterConfig) -> ClusterExtensionInstaller {
    ClusterExtensionInstaller::new(config, move |event_stream, block_list_provider, advertised_address| {
      Box::new(
        AwsEcsClusterProvider::new(event_stream, block_list_provider, advertised_address)
          .with_ecs_config(ecs_config.clone()),
      )
    })
  }
}
