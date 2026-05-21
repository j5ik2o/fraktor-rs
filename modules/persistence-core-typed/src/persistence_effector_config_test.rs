use alloc::string::{String, ToString};
use core::any::Any;

use fraktor_utils_core_rs::sync::ArcShared;

use crate::{EventAdapter, EventSeq, PersistenceEffectorConfig, PersistenceId, Recovery, SnapshotCriteria};

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
fn recovery_selection_is_separate_from_snapshot_write_criteria() {
  let config = PersistenceEffectorConfig::<u32, u32, ()>::new(PersistenceId::of_unique_id("test"), 0, apply_event)
    .with_snapshot_criteria(SnapshotCriteria::Every { number_of_events: 10 })
    .with_recovery(Recovery::without_snapshot());

  assert!(matches!(config.snapshot_criteria(), SnapshotCriteria::Every { number_of_events: 10 }));
  assert_eq!(config.recovery().snapshot_selection_criteria().max_sequence_nr(), 0);
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
