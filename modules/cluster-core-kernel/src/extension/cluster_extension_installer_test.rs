use alloc::{boxed::Box, string::String};

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext, error::ActorError, extension::ExtensionInstallers, messaging::AnyMessageView, props::Props,
    setup::ActorSystemConfig,
  },
  system::{ActorSystem, ActorSystemBuildError},
};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::ClusterExtensionInstaller;
use crate::{
  ClusterExtensionConfig, ClusterProviderError,
  activation::NoopIdentityLookup,
  cluster_provider::NoopClusterProvider,
  downing_provider::{DowningDecision, DowningInput, DowningProvider, DowningProviderCompatibility},
  failure_detector::FailureDetectorConfig,
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

#[test]
fn install_rejects_invalid_failure_detector_config_before_building_components() {
  let provider_calls = ArcShared::new(SpinSyncMutex::new(0usize));
  let pubsub_calls = ArcShared::new(SpinSyncMutex::new(0usize));
  let identity_lookup_calls = ArcShared::new(SpinSyncMutex::new(0usize));
  let provider_calls_for_factory = provider_calls.clone();
  let pubsub_calls_for_factory = pubsub_calls.clone();
  let identity_lookup_calls_for_factory = identity_lookup_calls.clone();
  let cluster_config =
    ClusterExtensionConfig::new().with_failure_detector_config(FailureDetectorConfig::new().with_phi_threshold(0.0));
  let cluster_installer =
    ClusterExtensionInstaller::new(cluster_config, move |_event_stream, _block_list, _address| {
      *provider_calls_for_factory.lock() += 1;
      Box::new(NoopClusterProvider::new())
    })
    .with_pubsub_factory(move |_config| {
      *pubsub_calls_for_factory.lock() += 1;
      Box::new(NoopClusterPubSub::new()) as Box<dyn ClusterPubSub>
    })
    .with_identity_lookup_factory(move || {
      *identity_lookup_calls_for_factory.lock() += 1;
      Box::new(NoopIdentityLookup::new())
    });
  let props = Props::from_fn(|| TestGuardian);
  let system = ActorSystem::create_from_props(&props, ActorSystemConfig::new(TestTickDriver::default()))
    .expect("build actor system");

  let result = cluster_installer.install(&system);

  let Err(ActorSystemBuildError::Configuration(reason)) = result else {
    panic!("invalid failure detector config should reject actor system build");
  };
  assert!(reason.contains("InvalidPhiThreshold"));
  assert_eq!(*provider_calls.lock(), 0);
  assert_eq!(*pubsub_calls.lock(), 0);
  assert_eq!(*identity_lookup_calls.lock(), 0);
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
