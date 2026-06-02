use std::{
  string::{String, ToString},
  time::Duration,
};

use fraktor_cluster_core_kernel_rs::{
  cluster_provider::{DiscoveredAuthority, DiscoveryResult, DiscoveryTopologyMapper},
  extension::ClusterProviderError,
  topology::BlockListProvider,
};
use fraktor_utils_core_rs::{sync::ArcShared, time::TimerInstant};

use crate::cluster_provider::{DiscoveryBackend, DiscoveryBackendError, GenericDiscoveryAdapter};

struct EmptyBlockList;

impl BlockListProvider for EmptyBlockList {
  fn blocked_members(&self) -> Vec<String> {
    Vec::new()
  }
}

struct FakeDiscoveryBackend {
  source_identity: String,
  outcome:         Result<Vec<String>, DiscoveryBackendError>,
}

impl FakeDiscoveryBackend {
  fn successful(source_identity: &str, authorities: Vec<String>) -> Self {
    Self { source_identity: source_identity.to_string(), outcome: Ok(authorities) }
  }

  fn failing(source_identity: &str, error: DiscoveryBackendError) -> Self {
    Self { source_identity: source_identity.to_string(), outcome: Err(error) }
  }
}

impl DiscoveryBackend for FakeDiscoveryBackend {
  fn source_identity(&self) -> &str {
    self.source_identity.as_str()
  }

  fn discover(&mut self) -> Result<Vec<String>, DiscoveryBackendError> {
    self.outcome.clone()
  }
}

fn observed_at(ticks: u64) -> TimerInstant {
  TimerInstant::from_ticks(ticks, Duration::from_secs(1))
}

fn mapper() -> DiscoveryTopologyMapper {
  DiscoveryTopologyMapper::new(ArcShared::new(EmptyBlockList))
}

fn assert_authority(authority: &DiscoveredAuthority, expected_authority: &str, expected_source: &str) {
  assert_eq!(authority.authority(), expected_authority);
  assert_eq!(authority.source_identity(), expected_source);
  assert_eq!(authority.observed_at(), observed_at(42));
}

#[test]
fn generic_discovery_adapter_converts_backend_polling_success_to_discovery_result() {
  let backend = FakeDiscoveryBackend::successful(
    "fake-poller",
    Vec::from([String::from("node-a.example:7331"), String::from("node-b.example:7331")]),
  );
  let mut adapter = GenericDiscoveryAdapter::new(backend);

  let result = adapter.discover(observed_at(42));

  assert_eq!(result.authorities().len(), 2);
  assert_authority(&result.authorities()[0], "node-a.example:7331", "fake-poller");
  assert_authority(&result.authorities()[1], "node-b.example:7331", "fake-poller");
}

#[test]
fn generic_discovery_adapter_converts_empty_backend_success_to_empty_discovery_result() {
  let backend = FakeDiscoveryBackend::successful("fake-subscription", Vec::new());
  let mut adapter = GenericDiscoveryAdapter::new(backend);

  let result = adapter.discover(observed_at(42));

  assert!(result.is_empty());
  assert_eq!(result.source_identity(), Some("fake-subscription"));
  assert_eq!(result.observed_at(), Some(observed_at(42)));
}

#[test]
fn generic_discovery_adapter_converts_backend_failure_to_observable_discovery_result() {
  let error = DiscoveryBackendError::temporary("backend unavailable");
  let backend = FakeDiscoveryBackend::failing("fake-poller", error);
  let mut adapter = GenericDiscoveryAdapter::new(backend);

  let result = adapter.discover(observed_at(42));

  assert!(result.is_failed());
  assert_eq!(result.source_identity(), Some("fake-poller"));
  assert_eq!(result.observed_at(), Some(observed_at(42)));
  assert!(
    matches!(result, DiscoveryResult::Failed(_, _, ClusterProviderError::JoinFailed(ref reason)) if reason == "backend unavailable")
  );
  assert!(result.to_authorities().is_empty());
}

#[test]
fn generic_discovery_adapter_rejects_invalid_backend_authorities() {
  let backend = FakeDiscoveryBackend::successful(
    "fake-poller",
    Vec::from([String::from("valid-node"), String::from("invalid node")]),
  );
  let mut adapter = GenericDiscoveryAdapter::new(backend);

  let result = adapter.discover(observed_at(42));

  assert!(result.is_failed());
  assert!(matches!(
    result,
    DiscoveryResult::Failed(_, _, ClusterProviderError::JoinFailed(ref reason)) if reason == "invalid discovery authority"
  ));
  assert!(result.to_authorities().is_empty());
}

#[test]
fn generic_discovery_adapter_keeps_aws_ecs_style_authorities_compatible_with_topology_mapping() {
  let backend = FakeDiscoveryBackend::successful(
    "aws-ecs",
    Vec::from([String::from("10.0.1.12:8080"), String::from("10.0.1.12:8080")]),
  );
  let mut adapter = GenericDiscoveryAdapter::new(backend);
  let mut mapper = mapper();

  let result = adapter.discover(observed_at(42));
  let update = mapper.apply(&result).expect("aws ecs style authority should publish topology");

  assert_eq!(update.joined, vec![String::from("10.0.1.12:8080")]);
  assert_eq!(update.members, update.joined);
  assert!(update.left.is_empty());
  assert!(update.blocked.is_empty());
}
