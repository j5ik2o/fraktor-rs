//! AWS ECS installer extension trait.

#[cfg(test)]
#[path = "aws_ecs_cluster_extension_installer_ext_test.rs"]
mod tests;

use alloc::boxed::Box;

use fraktor_cluster_core_rs::{ClusterExtensionConfig, ClusterExtensionInstaller};

use crate::{AwsEcsClusterProvider, EcsClusterConfig};

/// Extension methods for creating cluster installers backed by AWS ECS discovery.
pub trait AwsEcsClusterExtensionInstallerExt {
  /// Creates a new installer with `AwsEcsClusterProvider`.
  ///
  /// This is a convenience constructor for AWS ECS environments where task discovery
  /// is performed via the ECS API (ListTasks + DescribeTasks).
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
