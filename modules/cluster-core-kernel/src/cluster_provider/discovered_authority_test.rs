use alloc::string::String;
use core::time::Duration;

use fraktor_utils_core_rs::time::TimerInstant;

use crate::cluster_provider::DiscoveredAuthority;

fn observed_at(ticks: u64) -> TimerInstant {
  TimerInstant::from_ticks(ticks, Duration::from_secs(1))
}

#[test]
fn discovered_authority_preserves_provider_neutral_observation_fields() {
  let authority =
    DiscoveredAuthority::new(String::from("node-a.example:7331"), String::from("aws-ecs-primary"), observed_at(42));

  assert_eq!(authority.authority(), "node-a.example:7331");
  assert_eq!(authority.source_identity(), "aws-ecs-primary");
  assert_eq!(authority.observed_at(), observed_at(42));
}

#[test]
fn discovered_authority_normalizes_to_authority_only_input() {
  let authority =
    DiscoveredAuthority::new(String::from("node-a.example:7331"), String::from("aws-ecs-primary"), observed_at(42));

  assert_eq!(authority.to_authority(), String::from("node-a.example:7331"));
}
