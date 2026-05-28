use fraktor_actor_adaptor_std_rs::system::create_noop_actor_system_with;
use fraktor_actor_core_kernel_rs::actor::extension::ExtensionInstallers;
use fraktor_cluster_core_rs::extension::{ClusterExtension, ClusterExtensionConfig, ClusterExtensionInstaller};

use crate::{AwsEcsClusterExtensionInstallerExt, EcsClusterConfig};

#[test]
fn new_with_ecs_builds_installer_through_extension_trait() {
  let ecs_config = EcsClusterConfig::new().with_cluster_name("cluster-a").with_service_name("service-a");

  let installer = ClusterExtensionInstaller::new_with_ecs(
    ClusterExtensionConfig::default().with_advertised_address("10.0.0.1:8080"),
    ecs_config,
  );

  let extensions = ExtensionInstallers::default().with_extension_installer(installer);
  let system = create_noop_actor_system_with(|config| config.with_extension_installers(extensions));

  assert!(system.extended().extension_by_type::<ClusterExtension>().is_some());
}
