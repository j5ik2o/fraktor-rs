use alloc::{string::String, vec};
use core::time::Duration;

use fraktor_utils_core_rs::time::TimerInstant;

use crate::{
  ClusterProviderError,
  cluster_provider::{DiscoveredAuthority, DiscoveryResult},
};

fn observed_at(ticks: u64) -> TimerInstant {
  TimerInstant::from_ticks(ticks, Duration::from_secs(1))
}

#[test]
fn discovery_result_represents_discovered_authorities() {
  let result = DiscoveryResult::discovered(vec![
    DiscoveredAuthority::new(String::from("node-a.example:7331"), String::from("aws-ecs-primary"), observed_at(7)),
    DiscoveredAuthority::new(String::from("node-b.example:7331"), String::from("aws-ecs-primary"), observed_at(7)),
  ]);

  assert_eq!(result.authorities().len(), 2);
  assert_eq!(result.to_authorities(), vec![String::from("node-a.example:7331"), String::from("node-b.example:7331")]);
}

#[test]
fn discovery_result_represents_empty_success() {
  let result = DiscoveryResult::empty(String::from("static-empty"), observed_at(8));

  assert!(result.is_empty());
  assert_eq!(result.source_identity(), Some("static-empty"));
  assert_eq!(result.observed_at(), Some(observed_at(8)));
  assert!(result.to_authorities().is_empty());
}

#[test]
fn discovery_result_represents_observable_failure_without_authority_input() {
  let error = ClusterProviderError::start_member("temporary discovery failure");
  let result = DiscoveryResult::failed(String::from("aws-ecs-primary"), observed_at(9), error.clone());

  assert!(result.is_failed());
  assert_eq!(result.source_identity(), Some("aws-ecs-primary"));
  assert_eq!(result.observed_at(), Some(observed_at(9)));
  assert_eq!(result.error(), Some(&error));
  assert!(result.to_authorities().is_empty());
}
