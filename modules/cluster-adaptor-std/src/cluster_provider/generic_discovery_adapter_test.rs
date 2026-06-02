use std::{
  string::{String, ToString},
  time::Duration,
};

use fraktor_cluster_core_kernel_rs::{
  cluster_provider::{DiscoveredAuthority, DiscoveryResult},
  extension::ClusterProviderError,
};
use fraktor_utils_core_rs::time::TimerInstant;

use crate::cluster_provider::{DiscoveryBackend, DiscoveryBackendError, GenericDiscoveryAdapter};

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
    matches!(result, DiscoveryResult::Failed(_, _, ClusterProviderError::StartMemberFailed(ref reason)) if reason == "backend unavailable")
  );
  assert!(result.to_authorities().is_empty());
}
