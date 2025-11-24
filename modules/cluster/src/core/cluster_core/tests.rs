use alloc::{string::String, vec, vec::Vec};

use super::*;
use crate::core::{ActivatedKind, ClusterProviderError, IdentityLookup, IdentitySetupError, KindRegistry, TOPIC_ACTOR_KIND};
use fraktor_actor_rs::core::event_stream::EventStreamGeneric;
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::core::{runtime_toolbox::{NoStdMutex, NoStdToolbox}, sync::ArcShared};

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

#[derive(Clone, Debug, PartialEq)]
enum IdentityMode {
  Member,
  Client,
}

#[derive(Clone, Debug, PartialEq)]
struct IdentityCall {
  mode:  IdentityMode,
  kinds: Vec<String>,
}

#[derive(Clone)]
struct StubIdentityLookup {
  calls: ArcShared<NoStdMutex<Vec<IdentityCall>>>,
}

impl StubIdentityLookup {
  fn new() -> Self {
    Self { calls: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }

  fn record(&self, mode: IdentityMode, kinds: &[ActivatedKind]) {
    let mut guard = self.calls.lock();
    let mut names: Vec<String> = kinds.iter().map(|k| k.name().to_string()).collect();
    names.sort();
    guard.push(IdentityCall { mode, kinds: names });
  }

  fn calls(&self) -> Vec<IdentityCall> {
    self.calls.lock().clone()
  }
}

impl IdentityLookup for StubIdentityLookup {
  fn setup_member(&self, kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    self.record(IdentityMode::Member, kinds);
    Ok(())
  }

  fn setup_client(&self, kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    self.record(IdentityMode::Client, kinds);
    Ok(())
  }
}

fn build_core_with_config(config: ClusterExtensionConfig) -> ClusterCore<NoStdToolbox> {
  let provider = ArcShared::new(StubProvider::default());
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec!["blocked-node".to_string()]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let kind_registry = KindRegistry::new();
  let identity_lookup: ArcShared<dyn IdentityLookup> = ArcShared::new(StubIdentityLookup::new());

  ClusterCore::new(config, provider, block_list_provider, event_stream, kind_registry, identity_lookup)
}

#[test]
fn new_core_stores_dependencies_and_startup_params() {
  let config = ClusterExtensionConfig::new()
    .with_advertised_address("proto://node-a")
    .with_metrics_enabled(true);

  let provider = ArcShared::new(StubProvider::default());
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec!["blocked-node".to_string()]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let kind_registry = KindRegistry::new();
  let identity_lookup: ArcShared<dyn IdentityLookup> = ArcShared::new(StubIdentityLookup::new());

  let core = ClusterCore::new(
    config.clone(),
    provider.clone(),
    block_list_provider.clone(),
    event_stream.clone(),
    kind_registry,
    identity_lookup,
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

#[test]
fn setup_member_kinds_registers_and_updates_virtual_actor_count() {
  let provider = ArcShared::new(StubProvider::default());
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let kind_registry = KindRegistry::new();
  let identity_lookup = ArcShared::new(StubIdentityLookup::new());
  let mut core = ClusterCore::new(
    ClusterExtensionConfig::new(),
    provider,
    block_list_provider,
    event_stream,
    kind_registry,
    identity_lookup.clone(),
  );

  core.setup_member_kinds(vec![ActivatedKind::new("worker"), ActivatedKind::new("analytics")]).unwrap();

  assert_eq!(3, core.virtual_actor_count()); // worker + analytics + topic kind

  let recorded = identity_lookup.calls();
  assert_eq!(1, recorded.len());
  assert_eq!(recorded[0].mode, IdentityMode::Member);
  assert_eq!(recorded[0].kinds, vec![
    String::from("analytics"),
    String::from(TOPIC_ACTOR_KIND),
    String::from("worker"),
  ]);
}

#[test]
fn setup_client_kinds_registers_and_updates_virtual_actor_count() {
  let provider = ArcShared::new(StubProvider::default());
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let kind_registry = KindRegistry::new();
  let identity_lookup = ArcShared::new(StubIdentityLookup::new());
  let mut core = ClusterCore::new(
    ClusterExtensionConfig::new(),
    provider,
    block_list_provider,
    event_stream,
    kind_registry,
    identity_lookup.clone(),
  );

  core.setup_client_kinds(vec![ActivatedKind::new("worker")]).unwrap();

  assert_eq!(2, core.virtual_actor_count());

  let recorded = identity_lookup.calls();
  assert_eq!(1, recorded.len());
  assert_eq!(IdentityMode::Client, recorded[0].mode);
  assert_eq!(recorded[0].kinds, vec![String::from(TOPIC_ACTOR_KIND), String::from("worker")]);
}
