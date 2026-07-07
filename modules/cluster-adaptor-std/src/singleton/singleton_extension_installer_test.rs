use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    extension::{ExtensionInstaller, ExtensionInstallers},
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_cluster_core_kernel_rs::{
  cluster_provider::NoopClusterProvider,
  extension::{ClusterExtensionConfig, ClusterExtensionInstaller},
  singleton::{ClusterSingletonManagerConfig, ClusterSingletonProxyConfig},
};

use super::ClusterSingletonExtensionInstaller;

#[test]
fn singleton_installer_requires_cluster_extension() {
  let installer =
    ClusterSingletonExtensionInstaller::new(ClusterSingletonManagerConfig::new(), ClusterSingletonProxyConfig::new());
  let config = ActorSystemConfig::new(TestTickDriver::default());
  let system = ActorSystem::create_with_noop_guardian(config).expect("actor system");
  assert!(installer.install(&system).is_err());
}

#[test]
fn singleton_installer_succeeds_when_cluster_extension_is_present() {
  let cluster_installer = ClusterExtensionInstaller::new(
    ClusterExtensionConfig::new().with_advertised_address("node1:8080"),
    |_event_stream, _block_list, _address| Box::new(NoopClusterProvider::new()),
  );
  let singleton_installer =
    ClusterSingletonExtensionInstaller::new(ClusterSingletonManagerConfig::new(), ClusterSingletonProxyConfig::new());
  let installers = ExtensionInstallers::default()
    .with_extension_installer(cluster_installer)
    .with_extension_installer(singleton_installer);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_extension_installers(installers);
  ActorSystem::create_with_noop_guardian(config).expect("actor system with singleton installer");
}
