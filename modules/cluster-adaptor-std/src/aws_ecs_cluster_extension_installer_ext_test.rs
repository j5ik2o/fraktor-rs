use fraktor_cluster_core_rs::{ClusterExtensionConfig, ClusterExtensionInstaller};

use crate::{AwsEcsClusterExtensionInstallerExt, EcsClusterConfig};

#[test]
fn new_with_ecs_builds_installer_through_extension_trait() {
  let ecs_config = EcsClusterConfig::new().with_cluster_name("cluster-a").with_service_name("service-a");

  let _installer = ClusterExtensionInstaller::new_with_ecs(
    ClusterExtensionConfig::default().with_advertised_address("10.0.0.1:8080"),
    ecs_config,
  );
}
