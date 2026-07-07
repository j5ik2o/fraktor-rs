use fraktor_cluster_core_kernel_rs::ddata::{
  DurableDataEnvelope, DurableStore, DurableStoreLoadAll, DurableStoreLoadAllCompleted, DurableStoreStore, Flag,
};

use crate::ddata::InMemoryDurableStore;

#[test]
fn store_and_load_all_round_trip_entries() {
  let mut store = InMemoryDurableStore::<Flag>::new();
  store
    .store(&DurableStoreStore::new("flag-key", DurableDataEnvelope::new(Flag::disabled().switch_on())))
    .expect("store succeeds");

  let loaded = store.load_all().expect("load succeeds");
  assert_eq!(loaded.data().len(), 1);
  assert!(loaded.data().get("flag-key").expect("entry exists").data().is_enabled());
}

#[test]
fn startup_load_returns_completed_marker() {
  let mut store = InMemoryDurableStore::<Flag>::new();
  let (data, completed) = store.startup_load(DurableStoreLoadAll).expect("startup load succeeds");
  assert!(data.data().is_empty());
  assert_eq!(completed, DurableStoreLoadAllCompleted);
}

#[test]
fn len_tracks_inserted_entries() {
  let mut store = InMemoryDurableStore::<Flag>::new();
  assert!(store.is_empty());
  store.store(&DurableStoreStore::new("flag-key", DurableDataEnvelope::new(Flag::disabled()))).expect("store succeeds");
  assert_eq!(store.len(), 1);
}
