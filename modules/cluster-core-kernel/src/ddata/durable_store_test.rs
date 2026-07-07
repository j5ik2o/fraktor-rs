use alloc::{collections::BTreeMap, string::ToString};

use super::{
  DurableStore, DurableStoreError, DurableStoreLoadAll, DurableStoreLoadAllCompleted, DurableStoreLoadData,
  DurableStoreStore,
};
use crate::ddata::{DurableDataEnvelope, DurableStoreStoreReply, Flag};

struct FlagStore {
  entries: BTreeMap<alloc::string::String, DurableDataEnvelope<Flag>>,
}

impl FlagStore {
  const fn new() -> Self {
    Self { entries: BTreeMap::new() }
  }
}

impl DurableStore<Flag> for FlagStore {
  fn store(&mut self, request: &DurableStoreStore<Flag>) -> Result<(), DurableStoreError> {
    self.entries.insert(request.key().to_string(), request.data().clone());
    Ok(())
  }

  fn load_all(&mut self) -> Result<DurableStoreLoadData<Flag>, DurableStoreError> {
    Ok(DurableStoreLoadData::new(self.entries.clone()))
  }
}

#[test]
fn store_and_load_all_round_trip_flag_entries() {
  let mut store = FlagStore::new();
  let request = DurableStoreStore::new("flag-key", DurableDataEnvelope::new(Flag::disabled().switch_on()));
  store.store(&request).expect("store succeeds");

  let loaded = store.load_all().expect("load succeeds");
  assert_eq!(loaded.data().len(), 1);
  assert!(loaded.data().get("flag-key").expect("entry exists").data().is_enabled());
}

#[test]
fn startup_load_returns_completed_marker() {
  let mut store = FlagStore::new();
  let (data, completed) = store.startup_load(DurableStoreLoadAll).expect("startup load succeeds");
  assert!(data.is_empty());
  assert_eq!(completed, DurableStoreLoadAllCompleted);
}

#[test]
fn store_reply_contract_is_optional() {
  let request = DurableStoreStore::new("flag-key", DurableDataEnvelope::new(Flag::disabled()))
    .with_reply(DurableStoreStoreReply::new(true, false));
  assert!(request.reply().is_some());
}
