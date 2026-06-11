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
  membership::NoopGossiper,
  pub_sub::{NoopClusterPubSub, cluster_pub_sub::ClusterPubSub},
  singleton::{ClusterSingletonManagerSettings, ClusterSingletonProxySettings},
};

#[test]
fn with_downing_provider_factory_propagates_compatibility_key_to_install_config() {
  let observed_key = ArcShared::new(SpinSyncMutex::new(Option::<String>::None));
  let observed_phi_threshold = ArcShared::new(SpinSyncMutex::new(Option::<f64>::None));
  let observed_key_for_pubsub = observed_key.clone();
  let observed_phi_threshold_for_gossiper = observed_phi_threshold.clone();
  let compatibility = DowningProviderCompatibility::new("recording-downing-provider");
  let cluster_config = ClusterExtensionConfig::new()
    .with_advertised_address("node1:8080")
    .with_failure_detector_config(FailureDetectorConfig::new().with_phi_threshold(9.0));
  let cluster_installer = ClusterExtensionInstaller::new(cluster_config, |_event_stream, _block_list, _address| {
    Box::new(NoopClusterProvider::new())
  })
  .with_downing_provider_factory(compatibility, || Box::new(RecordingDowningProvider))
  .with_gossiper_factory(move |config| {
    *observed_phi_threshold_for_gossiper.lock() = Some(config.failure_detector_config().phi_threshold());
    Box::new(NoopGossiper)
  })
  .with_pubsub_factory(move |config| {
    *observed_key_for_pubsub.lock() = Some(String::from(config.downing_provider_compatibility().provider_key()));
    Box::new(NoopClusterPubSub::new()) as Box<dyn ClusterPubSub>
  });
  let config = ActorSystemConfig::new(TestTickDriver::default())
    .with_extension_installers(ExtensionInstallers::default().with_extension_installer(cluster_installer));
  let props = Props::from_fn(|| TestGuardian);
  let _system = ActorSystem::create_from_props(&props, config).expect("build system");

  assert_eq!(observed_key.lock().clone(), Some(String::from("recording-downing-provider")));
  assert_eq!(*observed_phi_threshold.lock(), Some(9.0));
}

#[test]
fn install_rejects_invalid_failure_detector_config_before_building_components() {
  let provider_calls = ArcShared::new(SpinSyncMutex::new(0usize));
  let gossiper_calls = ArcShared::new(SpinSyncMutex::new(0usize));
  let pubsub_calls = ArcShared::new(SpinSyncMutex::new(0usize));
  let identity_lookup_calls = ArcShared::new(SpinSyncMutex::new(0usize));
  let provider_calls_for_factory = provider_calls.clone();
  let gossiper_calls_for_factory = gossiper_calls.clone();
  let pubsub_calls_for_factory = pubsub_calls.clone();
  let identity_lookup_calls_for_factory = identity_lookup_calls.clone();
  let cluster_config =
    ClusterExtensionConfig::new().with_failure_detector_config(FailureDetectorConfig::new().with_phi_threshold(0.0));
  let cluster_installer =
    ClusterExtensionInstaller::new(cluster_config, move |_event_stream, _block_list, _address| {
      *provider_calls_for_factory.lock() += 1;
      Box::new(NoopClusterProvider::new())
    })
    .with_gossiper_factory(move |_config| {
      *gossiper_calls_for_factory.lock() += 1;
      Box::new(NoopGossiper)
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
  assert_eq!(*gossiper_calls.lock(), 0);
  assert_eq!(*pubsub_calls.lock(), 0);
  assert_eq!(*identity_lookup_calls.lock(), 0);
}

// RED フェーズ: validate_singleton() が install で呼ばれていない現時点では、
// 不正な singleton 設定でも install が成立してしまい、このテストは失敗することを確認する。
#[test]
fn install_rejects_invalid_singleton_config_with_configuration_error() {
  // 空の singleton 名 → EmptySingletonName（要件 4.4）
  let cluster_config = ClusterExtensionConfig::new()
    .with_advertised_address("node1:8080")
    .with_singleton_manager_settings(ClusterSingletonManagerSettings::new().with_singleton_name(""));
  let cluster_installer = ClusterExtensionInstaller::new(cluster_config, |_event_stream, _block_list, _address| {
    Box::new(NoopClusterProvider::new())
  });
  let props = Props::from_fn(|| TestGuardian);
  let system = ActorSystem::create_from_props(&props, ActorSystemConfig::new(TestTickDriver::default()))
    .expect("build actor system");

  let result = cluster_installer.install(&system);

  let Err(ActorSystemBuildError::Configuration(reason)) = result else {
    panic!("invalid singleton config should reject install with Configuration error");
  };
  assert!(reason.contains("EmptySingletonName"), "reason was: {reason}");
}

// 既定値設定では install が成立する（要件 6.2）。
// singleton 未指定の ClusterExtensionConfig は既定値の singleton 設定を持つため
// validate_singleton() が Ok(()) を返し、install が継続しなければならない。
#[test]
fn install_succeeds_with_default_singleton_settings() {
  let cluster_config = ClusterExtensionConfig::new().with_advertised_address("node1:8080");
  let cluster_installer = ClusterExtensionInstaller::new(cluster_config, |_event_stream, _block_list, _address| {
    Box::new(NoopClusterProvider::new())
  });
  let props = Props::from_fn(|| TestGuardian);
  let config = ActorSystemConfig::new(TestTickDriver::default())
    .with_extension_installers(ExtensionInstallers::default().with_extension_installer(cluster_installer));
  // 既定値設定で ActorSystem 構築が成立することを確認する
  let _system =
    ActorSystem::create_from_props(&props, config).expect("default singleton settings should allow install");
}

// buffer_size 10001 → BufferSizeOutOfRange（要件 4.2）
#[test]
fn install_rejects_singleton_proxy_with_buffer_size_out_of_range() {
  let cluster_config = ClusterExtensionConfig::new()
    .with_advertised_address("node1:8080")
    .with_singleton_proxy_settings(ClusterSingletonProxySettings::new().with_buffer_size(10001));
  let cluster_installer = ClusterExtensionInstaller::new(cluster_config, |_event_stream, _block_list, _address| {
    Box::new(NoopClusterProvider::new())
  });
  let props = Props::from_fn(|| TestGuardian);
  let system = ActorSystem::create_from_props(&props, ActorSystemConfig::new(TestTickDriver::default()))
    .expect("build actor system");

  let result = cluster_installer.install(&system);

  let Err(ActorSystemBuildError::Configuration(reason)) = result else {
    panic!("buffer_size out of range should reject install with Configuration error");
  };
  assert!(reason.contains("BufferSizeOutOfRange"), "reason was: {reason}");
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
