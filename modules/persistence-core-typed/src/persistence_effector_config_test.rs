use alloc::{
  collections::BTreeSet,
  string::{String, ToString},
};
use core::{any::Any, time::Duration};

use fraktor_utils_core_rs::sync::ArcShared;

use crate::{
  BackoffConfig, EventAdapter, EventSeq, PersistenceEffectorConfig, PersistenceId, Recovery, SnapshotCriteria,
};

fn apply_event(state: &u32, event: &u32) -> u32 {
  state + event
}

#[test]
fn default_stash_capacity_is_bounded() {
  let config = PersistenceEffectorConfig::<u32, u32, ()>::new(PersistenceId::of_unique_id("test"), 0, apply_event);

  assert_eq!(config.stash_capacity(), 1000);
  assert!(config.validate().is_ok());
}

#[test]
fn default_recovery_is_unbounded() {
  let config = PersistenceEffectorConfig::<u32, u32, ()>::new(PersistenceId::of_unique_id("test"), 0, apply_event);

  assert_eq!(config.recovery().to_sequence_nr(), u64::MAX);
  assert_eq!(config.recovery().replay_max(), u64::MAX);
}

#[test]
fn event_publishing_is_enabled_by_default() {
  let config = PersistenceEffectorConfig::<u32, u32, ()>::new(PersistenceId::of_unique_id("test"), 0, apply_event);

  assert!(config.event_publishing_enabled());
}

#[test]
fn persist_failure_backoff_is_disabled_by_default() {
  let config = PersistenceEffectorConfig::<u32, u32, ()>::new(PersistenceId::of_unique_id("test"), 0, apply_event);

  assert!(!config.persist_failure_backoff_enabled());
}

#[test]
fn on_persist_failure_enables_hidden_store_backoff_config() {
  let backoff_config = BackoffConfig::new(Duration::from_millis(10), Duration::from_secs(1), 0.0);
  let config = PersistenceEffectorConfig::<u32, u32, ()>::new(PersistenceId::of_unique_id("test"), 0, apply_event);

  let configured = config.on_persist_failure(backoff_config.clone());

  assert!(configured.persist_failure_backoff_enabled());
  assert_eq!(configured.backoff_config(), &backoff_config);
}

#[test]
fn with_backoff_config_keeps_persist_failure_backoff_disabled() {
  let backoff_config = BackoffConfig::new(Duration::from_millis(20), Duration::from_secs(2), 0.1);
  let config = PersistenceEffectorConfig::<u32, u32, ()>::new(PersistenceId::of_unique_id("test"), 0, apply_event);

  let configured = config.with_backoff_config(backoff_config.clone());

  assert!(!configured.persist_failure_backoff_enabled());
  assert_eq!(configured.backoff_config(), &backoff_config);
}

#[test]
fn with_event_publishing_enables_and_disables_event_stream_publication() {
  let config = PersistenceEffectorConfig::<u32, u32, ()>::new(PersistenceId::of_unique_id("test"), 0, apply_event);

  let disabled = config.clone().with_event_publishing(false);
  let enabled = disabled.clone().with_event_publishing(true);

  assert!(enabled.event_publishing_enabled());
  assert!(!disabled.event_publishing_enabled());
}

#[test]
fn with_tagger_selects_tags_for_published_events() {
  let config = PersistenceEffectorConfig::<u32, u32, ()>::new(PersistenceId::of_unique_id("test"), 0, apply_event)
    .with_tagger(|event| BTreeSet::from([format!("event-{event}")]));

  assert_eq!(config.event_tags(&7), BTreeSet::from([String::from("event-7")]));
}

#[test]
fn recovery_selection_is_separate_from_snapshot_write_criteria() {
  let config = PersistenceEffectorConfig::<u32, u32, ()>::new(PersistenceId::of_unique_id("test"), 0, apply_event)
    .with_snapshot_criteria(SnapshotCriteria::Every { number_of_events: 10 })
    .with_recovery(Recovery::without_snapshot());

  assert!(matches!(config.snapshot_criteria(), SnapshotCriteria::Every { number_of_events: 10 }));
  assert_eq!(config.recovery().snapshot_criteria().max_sequence_nr(), 0);
}

#[test]
fn event_adapter_registration_updates_kernel_registry() {
  let config = PersistenceEffectorConfig::<u32, u32, ()>::new(PersistenceId::of_unique_id("test"), 0, apply_event)
    .with_event_adapter(AddTenAdapter);

  assert_eq!(config.event_adapters().len(), 1);
  let payload = config.event_adapters().to_journal::<u32>(ArcShared::new(5_u32));

  assert_eq!(payload.downcast_ref::<u32>(), Some(&15_u32));
}

struct AddTenAdapter;

impl EventAdapter<u32> for AddTenAdapter {
  fn manifest(&self, _event: &u32) -> String {
    "add-ten".to_string()
  }

  fn to_journal(&self, event: u32) -> ArcShared<dyn Any + Send + Sync> {
    ArcShared::new(event + 10)
  }

  fn adapt_from_journal(&self, event: ArcShared<dyn Any + Send + Sync>, _manifest: &str) -> EventSeq<u32> {
    EventSeq::single(event.downcast_ref::<u32>().copied().unwrap_or_default())
  }
}
