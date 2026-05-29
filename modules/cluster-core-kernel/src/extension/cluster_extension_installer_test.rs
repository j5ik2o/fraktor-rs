use alloc::{boxed::Box, string::String};

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext, error::ActorError, extension::ExtensionInstallers, messaging::AnyMessageView, props::Props,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::ClusterExtensionInstaller;
use crate::{
  ClusterExtensionConfig, ClusterProviderError,
  cluster_provider::NoopClusterProvider,
  downing_provider::{DowningDecision, DowningInput, DowningProvider, DowningProviderCompatibility},
  pub_sub::{NoopClusterPubSub, cluster_pub_sub::ClusterPubSub},
};

#[test]
fn with_downing_provider_factory_propagates_compatibility_key_to_install_config() {
  let observed_key = ArcShared::new(SpinSyncMutex::new(Option::<String>::None));
  let observed_key_for_pubsub = observed_key.clone();
  let compatibility = DowningProviderCompatibility::new("recording-downing-provider");
  let cluster_config = ClusterExtensionConfig::new().with_advertised_address("node1:8080");
  let cluster_installer = ClusterExtensionInstaller::new(cluster_config, |_event_stream, _block_list, _address| {
    Box::new(NoopClusterProvider::new())
  })
  .with_downing_provider_factory(compatibility, || Box::new(RecordingDowningProvider))
  .with_pubsub_factory(move |config| {
    *observed_key_for_pubsub.lock() = Some(String::from(config.downing_provider_compatibility().provider_key()));
    Box::new(NoopClusterPubSub::new()) as Box<dyn ClusterPubSub>
  });
  let config = ActorSystemConfig::new(TestTickDriver::default())
    .with_extension_installers(ExtensionInstallers::default().with_extension_installer(cluster_installer));
  let props = Props::from_fn(|| TestGuardian);
  let _system = ActorSystem::create_from_props(&props, config).expect("build system");

  assert_eq!(observed_key.lock().clone(), Some(String::from("recording-downing-provider")));
}

struct RecordingDowningProvider;

impl DowningProvider for RecordingDowningProvider {
  fn decide(&mut self, _input: &DowningInput) -> Result<DowningDecision, ClusterProviderError> {
    Ok(DowningDecision::Keep)
  }
}

struct TestGuardian;

impl Actor for TestGuardian {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}
