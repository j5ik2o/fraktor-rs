use alloc::string::{String, ToString};
use core::any::Any;

use fraktor_utils_core_rs::sync::ArcShared;

use super::EphemeralPersistenceStore;
use crate::{EventAdapter, EventSeq, PersistenceEffectorConfig, PersistenceId, PersistenceMode, Recovery};

fn apply_event(state: &u32, event: &u32) -> u32 {
  state + event
}

fn config(persistence_id: &str) -> PersistenceEffectorConfig<u32, u32, ()> {
  PersistenceEffectorConfig::new(PersistenceId::of_unique_id(persistence_id), 0, apply_event)
    .with_persistence_mode(PersistenceMode::Ephemeral)
}

#[test]
fn recovery_none_skips_ephemeral_replay_but_preserves_highest_sequence_nr() {
  let store = EphemeralPersistenceStore::new();
  store.persist_events(&config("none"), vec![1, 2]).expect("events should persist");
  let recovery_config = config("none").with_recovery(Recovery::none());

  let (state, sequence_nr) = store.recover(&recovery_config).expect("state should recover");

  assert_eq!(state, 0);
  assert_eq!(sequence_nr, 2);
}

#[test]
fn recovery_without_snapshot_replays_ephemeral_events_from_initial_state() {
  let store = EphemeralPersistenceStore::new();
  store.persist_events(&config("without-snapshot"), vec![1]).expect("first event should persist");
  store.persist_snapshot(&config("without-snapshot"), 100, 1).expect("snapshot should persist");
  store.persist_events(&config("without-snapshot"), vec![2]).expect("second event should persist");

  let (default_state, default_sequence_nr) = store.recover(&config("without-snapshot")).expect("state should recover");
  let (state, sequence_nr) = store
    .recover(&config("without-snapshot").with_recovery(Recovery::without_snapshot()))
    .expect("state should recover without snapshot");

  assert_eq!(default_state, 102);
  assert_eq!(default_sequence_nr, 2);
  assert_eq!(state, 3);
  assert_eq!(sequence_nr, 2);
}

#[test]
fn recovery_bounds_limit_ephemeral_replay_events() {
  let store = EphemeralPersistenceStore::new();
  store.persist_events(&config("bounded"), vec![1, 2, 3]).expect("events should persist");

  let (state, sequence_nr) =
    store.recover(&config("bounded").with_recovery(Recovery::new(2, 1))).expect("bounded state should recover");

  assert_eq!(state, 1);
  assert_eq!(sequence_nr, 3);
}

#[test]
fn event_adapters_are_applied_to_ephemeral_persistence() {
  let store = EphemeralPersistenceStore::new();
  let write_config = config("adapter").with_event_adapter(AddTenAdapter);
  store.persist_events(&write_config, vec![5]).expect("adapted event should persist");

  let (state, sequence_nr) = store.recover(&write_config).expect("adapted state should recover");

  assert_eq!(state, 5);
  assert_eq!(sequence_nr, 1);
}

struct AddTenAdapter;

impl EventAdapter<u32> for AddTenAdapter {
  fn manifest(&self, _event: &u32) -> String {
    "add-ten".to_string()
  }

  fn to_journal(&self, event: u32) -> ArcShared<dyn Any + Send + Sync> {
    ArcShared::new(event + 10)
  }

  fn adapt_from_journal(&self, event: ArcShared<dyn Any + Send + Sync>, manifest: &str) -> EventSeq<u32> {
    if manifest != "add-ten" {
      return EventSeq::empty();
    }
    EventSeq::single(event.downcast_ref::<u32>().copied().unwrap_or_default() - 10)
  }
}
