use alloc::boxed::Box;
use core::{
  future::{Future, ready},
  pin::Pin,
};

use fraktor_persistence_core_kernel_rs::state::{DurableStateError, DurableStateStore, GetObjectResult};

use super::{StateSourcedStoreActor, restore_store};
use crate::{
  PersistenceId, StateSourcedEffectorConfig,
  internal::{StateSourcedStoreCommand, state_sourced_store_command::StateSourcedStore},
};

type StoreFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, DurableStateError>> + Send + 'a>>;

struct TestStore;

impl DurableStateStore<u32> for TestStore {
  fn get_object<'a>(&'a self, _persistence_id: &'a str) -> StoreFuture<'a, GetObjectResult<u32>> {
    Box::pin(ready(Ok(GetObjectResult::new(Some(42), 7))))
  }

  fn upsert_object<'a>(
    &'a mut self,
    _persistence_id: &'a str,
    _expected_revision: u64,
    _object: u32,
    _tag: Option<&'a str>,
  ) -> StoreFuture<'a, ()> {
    Box::pin(ready(Ok(())))
  }

  fn delete_object<'a>(&'a mut self, _persistence_id: &'a str, _expected_revision: u64) -> StoreFuture<'a, ()> {
    Box::pin(ready(Ok(())))
  }
}

fn store() -> StateSourcedStore<u32> {
  Box::new(TestStore)
}

fn actor() -> StateSourcedStoreActor<u32, StateSourcedStoreCommand<u32>> {
  let config = StateSourcedEffectorConfig::new(PersistenceId::of_unique_id("state-store-test"));
  StateSourcedStoreActor::new(config, store())
}

fn store_slot_available(actor: &StateSourcedStoreActor<u32, StateSourcedStoreCommand<u32>>) -> bool {
  actor.store.with_lock(|store| store.is_some())
}

#[test]
fn new_actor_owns_durable_state_store() {
  let actor = actor();

  assert!(store_slot_available(&actor));
  assert!(!actor.in_flight);
}

#[test]
fn take_store_removes_store_until_restored() {
  let mut actor = actor();

  let store = actor.take_store().expect("store should be available");
  let second_take = actor.take_store();

  assert!(!store_slot_available(&actor));
  assert!(second_take.is_err());

  restore_store(&actor.store, store);

  assert!(store_slot_available(&actor));
  assert!(!actor.in_flight);
}
