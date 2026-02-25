use alloc::{
  boxed::Box,
  collections::BTreeMap,
  string::{String, ToString},
  vec::Vec,
};
use core::{
  future::{Future, ready},
  pin::Pin,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{
  durable_state_exception::DurableStateException, durable_state_store::DurableStateStore,
  durable_state_store_provider::DurableStateStoreProvider, durable_state_store_registry::DurableStateStoreRegistry,
  durable_state_update_store::DurableStateUpdateStore,
};

const TEST_PROVIDER_ID: &str = "in-memory";
const TEST_PERSISTENCE_ID: &str = "persistence-1";
const TEST_UNKNOWN_PROVIDER_ID: &str = "missing-provider";

type DurableStateFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, DurableStateException>> + Send + 'a>>;

#[derive(Default)]
struct TestDurableStateStore {
  objects: BTreeMap<String, i32>,
  updates: BTreeMap<String, Vec<i32>>,
}

impl TestDurableStateStore {
  const fn new() -> Self {
    Self { objects: BTreeMap::new(), updates: BTreeMap::new() }
  }
}

impl DurableStateStore<i32> for TestDurableStateStore {
  fn get_object<'a>(&'a self, persistence_id: &'a str) -> DurableStateFuture<'a, Option<i32>> {
    Box::pin(ready(Ok(self.objects.get(persistence_id).copied())))
  }

  fn upsert_object<'a>(&'a mut self, persistence_id: &'a str, object: i32) -> DurableStateFuture<'a, ()> {
    self.objects.insert(persistence_id.to_string(), object);
    self.updates.entry(persistence_id.to_string()).or_default().push(object);
    Box::pin(ready(Ok(())))
  }

  fn delete_object<'a>(&'a mut self, persistence_id: &'a str) -> DurableStateFuture<'a, ()> {
    self.objects.remove(persistence_id);
    Box::pin(ready(Ok(())))
  }
}

impl DurableStateUpdateStore<i32> for TestDurableStateStore {
  fn changes<'a>(
    &'a self,
    persistence_id: &'a str,
    from_offset: usize,
  ) -> DurableStateFuture<'a, Option<(usize, i32)>> {
    let next_change = self
      .updates
      .get(persistence_id)
      .and_then(|updates| updates.get(from_offset))
      .copied()
      .map(|state| (from_offset.saturating_add(1), state));
    Box::pin(ready(Ok(next_change)))
  }
}

struct TestDurableStateStoreProvider;

impl TestDurableStateStoreProvider {
  const fn new() -> Self {
    Self
  }
}

impl DurableStateStoreProvider<i32> for TestDurableStateStoreProvider {
  fn durable_state_store(&self) -> Box<dyn DurableStateStore<i32>> {
    Box::new(TestDurableStateStore::new())
  }
}

fn noop_waker() -> Waker {
  const VTABLE: RawWakerVTable = RawWakerVTable::new(|data| RawWaker::new(data, &VTABLE), |_| {}, |_| {}, |_| {});

  unsafe fn raw_waker() -> RawWaker {
    RawWaker::new(core::ptr::null(), &VTABLE)
  }

  unsafe { Waker::from_raw(raw_waker()) }
}

fn poll_ready<F: Future>(future: F) -> F::Output {
  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);
  let mut future = Box::pin(future);
  match Future::poll(future.as_mut(), &mut context) {
    | Poll::Ready(output) => output,
    | Poll::Pending => panic!("future was pending"),
  }
}

#[test]
fn register_and_resolve_provider_for_crud_operations() {
  let mut registry = DurableStateStoreRegistry::<i32>::empty();
  let provider: ArcShared<dyn DurableStateStoreProvider<i32>> = ArcShared::new(TestDurableStateStoreProvider::new());
  registry.register(TEST_PROVIDER_ID, provider).expect("register provider");

  let mut store = registry.resolve(TEST_PROVIDER_ID).expect("resolve provider");

  poll_ready(store.upsert_object(TEST_PERSISTENCE_ID, 42)).expect("upsert durable state");
  let loaded = poll_ready(store.get_object(TEST_PERSISTENCE_ID)).expect("get durable state");
  assert_eq!(loaded, Some(42));

  poll_ready(store.delete_object(TEST_PERSISTENCE_ID)).expect("delete durable state");
  let loaded_after_delete = poll_ready(store.get_object(TEST_PERSISTENCE_ID)).expect("get durable state after delete");
  assert_eq!(loaded_after_delete, None);
}

#[test]
fn register_duplicate_provider_fails() {
  let mut registry = DurableStateStoreRegistry::<i32>::empty();
  let provider: ArcShared<dyn DurableStateStoreProvider<i32>> = ArcShared::new(TestDurableStateStoreProvider::new());

  registry.register(TEST_PROVIDER_ID, provider.clone()).expect("first register provider");

  let result = registry.register(TEST_PROVIDER_ID, provider);
  assert_eq!(result, Err(DurableStateException::ProviderAlreadyRegistered(TEST_PROVIDER_ID.to_string())));
}

#[test]
fn resolve_unknown_provider_fails() {
  let registry = DurableStateStoreRegistry::<i32>::empty();

  let result = registry.resolve(TEST_UNKNOWN_PROVIDER_ID);
  assert_eq!(result.err(), Some(DurableStateException::ProviderNotFound(TEST_UNKNOWN_PROVIDER_ID.to_string())));
}

#[test]
fn durable_state_update_store_reports_changes() {
  let mut store = TestDurableStateStore::new();

  let no_change = poll_ready(store.changes(TEST_PERSISTENCE_ID, 0)).expect("load changes before first upsert");
  assert_eq!(no_change, None);

  poll_ready(store.upsert_object(TEST_PERSISTENCE_ID, 10)).expect("upsert first state");
  let first_change = poll_ready(store.changes(TEST_PERSISTENCE_ID, 0)).expect("load first change");
  assert_eq!(first_change, Some((1, 10)));

  let no_second_change = poll_ready(store.changes(TEST_PERSISTENCE_ID, 1)).expect("load second change before upsert");
  assert_eq!(no_second_change, None);

  poll_ready(store.upsert_object(TEST_PERSISTENCE_ID, 20)).expect("upsert second state");
  let second_change = poll_ready(store.changes(TEST_PERSISTENCE_ID, 1)).expect("load second change");
  assert_eq!(second_change, Some((2, 20)));

  let no_third_change = poll_ready(store.changes(TEST_PERSISTENCE_ID, 2)).expect("load third change");
  assert_eq!(no_third_change, None);
}
