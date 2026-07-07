use core::time::Duration;

use super::ReplicatorSettingsError;
use crate::ddata::ReplicatorSettings;

#[test]
fn defaults_match_pekko_configuration() {
  let settings = ReplicatorSettings::new();
  assert_eq!(settings.role(), None);
  assert_eq!(settings.gossip_interval(), Duration::from_secs(2));
  assert_eq!(settings.notify_subscribers_interval(), Duration::from_millis(500));
  assert_eq!(settings.max_delta_elements(), 1000);
  assert!(settings.prefer_oldest());
  assert_eq!(settings.actor_name(), "ddataReplicator");
}

#[test]
fn validate_rejects_empty_actor_name() {
  let settings = ReplicatorSettings::new().with_actor_name("");
  assert_eq!(settings.validate(), Err(ReplicatorSettingsError::EmptyActorName));
}

#[test]
fn validate_rejects_zero_gossip_interval() {
  let settings = ReplicatorSettings::new().with_gossip_interval(Duration::ZERO);
  assert_eq!(settings.validate(), Err(ReplicatorSettingsError::NonPositiveGossipInterval));
}

#[test]
fn validate_accepts_custom_role() {
  let settings = ReplicatorSettings::new().with_role("backend");
  assert_eq!(settings.role(), Some("backend"));
  assert_eq!(settings.validate(), Ok(()));
}
