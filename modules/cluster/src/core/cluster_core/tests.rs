use alloc::{string::String, vec, vec::Vec};

use super::*;
use crate::core::ClusterProviderError;
use fraktor_actor_rs::core::event_stream::EventStreamGeneric;
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

#[derive(Debug, Default)]
struct StubProvider;

impl ClusterProvider for StubProvider {
  fn start_member(&self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn start_client(&self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn shutdown(&self, _graceful: bool) -> Result<(), ClusterProviderError> {
    Ok(())
  }
}

#[derive(Debug, Clone)]
struct StubBlockListProvider {
  blocked: Vec<String>,
}

impl StubBlockListProvider {
  fn new(blocked: Vec<String>) -> Self {
    Self { blocked }
  }
}

impl BlockListProvider for StubBlockListProvider {
  fn blocked_members(&self) -> Vec<String> {
    self.blocked.clone()
  }
}

fn build_core_with_config(config: ClusterExtensionConfig) -> ClusterCore<NoStdToolbox> {
  let provider = ArcShared::new(StubProvider::default());
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec!["blocked-node".to_string()]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());

  ClusterCore::new(config, provider, block_list_provider, event_stream)
}

#[test]
fn new_core_stores_dependencies_and_startup_params() {
  let config = ClusterExtensionConfig::new()
    .with_advertised_address("proto://node-a")
    .with_metrics_enabled(true);

  let provider = ArcShared::new(StubProvider::default());
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec!["blocked-node".to_string()]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());

  let core = ClusterCore::new(
    config.clone(),
    provider.clone(),
    block_list_provider.clone(),
    event_stream.clone(),
  );

  // 依存がそのまま保持されていること
  let provider_dyn: ArcShared<dyn ClusterProvider> = provider.clone();
  assert!(core.provider() == &provider_dyn);

  let block_list_provider_dyn: ArcShared<dyn BlockListProvider> = block_list_provider.clone();
  assert!(core.block_list_provider() == &block_list_provider_dyn);

  assert!(core.event_stream() == &event_stream);

  // 構成が保持されていること
  assert_eq!(core.config().advertised_address(), config.advertised_address());

  // 起動パラメータが両モードで再利用できる形で保持されること
  assert_eq!(core.startup_address(), config.advertised_address());
  assert_eq!(core.startup_address(), config.advertised_address());
}

#[test]
fn metrics_flag_reflects_config_setting() {
  let enabled_core = build_core_with_config(ClusterExtensionConfig::new().with_metrics_enabled(true));
  assert!(enabled_core.metrics_enabled());

  let disabled_core = build_core_with_config(ClusterExtensionConfig::new().with_metrics_enabled(false));
  assert!(!disabled_core.metrics_enabled());
}
