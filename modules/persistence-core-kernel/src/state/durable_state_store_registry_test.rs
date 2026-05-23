use alloc::{
  boxed::Box,
  collections::BTreeMap,
  string::{String, ToString},
  vec::Vec,
};
use core::{
  future::{Future, ready},
  pin::Pin,
  task::{Context, Poll, Waker},
};

use fraktor_utils_core_rs::sync::ArcShared;

use crate::state::{
  DurableStateChange, DurableStateError, DurableStateStore, DurableStateStoreProvider, DurableStateStoreRegistry,
  DurableStateUpdateStore, GetObjectResult,
};

const TEST_PROVIDER_ID: &str = "in-memory";
const TEST_PERSISTENCE_ID: &str = "persistence-1";
const TEST_UNKNOWN_PROVIDER_ID: &str = "missing-provider";

type DurableStateFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, DurableStateError>> + Send + 'a>>;

#[derive(Default)]
struct TestDurableStateStore {
  objects:   BTreeMap<String, i32>,
  revisions: BTreeMap<String, u64>,
  updates:   BTreeMap<String, Vec<DurableStateChange<i32>>>,
}

impl TestDurableStateStore {
  const fn new() -> Self {
    Self { objects: BTreeMap::new(), revisions: BTreeMap::new(), updates: BTreeMap::new() }
  }
}

impl DurableStateStore<i32> for TestDurableStateStore {
  fn get_object<'a>(&'a self, persistence_id: &'a str) -> DurableStateFuture<'a, GetObjectResult<i32>> {
    let result = self.objects.get(persistence_id).copied().map_or_else(GetObjectResult::empty, |value| {
      let revision = *self.revisions.get(persistence_id).expect("revision must exist when object exists");
      GetObjectResult::new(Some(value), revision)
    });
    Box::pin(ready(Ok(result)))
  }

  fn upsert_object<'a>(
    &'a mut self,
    persistence_id: &'a str,
    expected_revision: u64,
    object: i32,
    tag: Option<&'a str>,
  ) -> DurableStateFuture<'a, ()> {
    let actual_revision = self.revisions.get(persistence_id).copied().unwrap_or(0);
    if actual_revision != expected_revision {
      return Box::pin(ready(Err(DurableStateError::upsert_revision(
        persistence_id,
        expected_revision,
        actual_revision,
      ))));
    }

    self.objects.insert(persistence_id.to_string(), object);
    let revision = actual_revision.saturating_add(1);
    self.revisions.insert(persistence_id.to_string(), revision);

    if let Some(tag) = tag {
      let updates = self.updates.entry(tag.to_string()).or_default();
      let offset = updates.len().saturating_add(1);
      updates.push(DurableStateChange::new(offset, persistence_id.to_string(), revision, tag.to_string(), object));
    }

    Box::pin(ready(Ok(())))
  }

  fn delete_object<'a>(&'a mut self, persistence_id: &'a str, expected_revision: u64) -> DurableStateFuture<'a, ()> {
    let actual_revision = self.revisions.get(persistence_id).copied().unwrap_or(0);
    if actual_revision != expected_revision {
      return Box::pin(ready(Err(DurableStateError::delete_revision(
        persistence_id,
        expected_revision,
        actual_revision,
      ))));
    }

    self.objects.remove(persistence_id);
    self.revisions.remove(persistence_id);
    Box::pin(ready(Ok(())))
  }
}

impl DurableStateUpdateStore<i32> for TestDurableStateStore {
  fn changes<'a>(
    &'a self,
    tag: &'a str,
    from_offset: usize,
  ) -> DurableStateFuture<'a, Option<DurableStateChange<i32>>> {
    let next_change = self.updates.get(tag).and_then(|updates| updates.get(from_offset)).cloned();
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

fn poll_ready<F: Future>(future: F) -> F::Output {
  let waker = Waker::noop();
  let mut context = Context::from_waker(waker);
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

  poll_ready(store.upsert_object(TEST_PERSISTENCE_ID, 0, 42, None)).expect("upsert durable state");
  let loaded = poll_ready(store.get_object(TEST_PERSISTENCE_ID)).expect("get durable state");
  assert_eq!(loaded.value(), Some(&42));
  assert_eq!(loaded.revision(), 1);

  poll_ready(store.delete_object(TEST_PERSISTENCE_ID, 1)).expect("delete durable state");
  let loaded_after_delete = poll_ready(store.get_object(TEST_PERSISTENCE_ID)).expect("get durable state after delete");
  assert!(loaded_after_delete.is_empty());
  assert_eq!(loaded_after_delete.revision(), 0);
}

#[test]
fn revision_mismatch_rejects_upsert_without_mutation() {
  let mut store = TestDurableStateStore::new();

  poll_ready(store.upsert_object(TEST_PERSISTENCE_ID, 0, 10, Some("orders"))).expect("upsert initial state");

  let result = poll_ready(store.upsert_object(TEST_PERSISTENCE_ID, 0, 20, Some("orders")));
  assert_eq!(result, Err(DurableStateError::upsert_revision(TEST_PERSISTENCE_ID, 0, 1)));

  let loaded = poll_ready(store.get_object(TEST_PERSISTENCE_ID)).expect("get durable state after failed upsert");
  assert_eq!(loaded.value(), Some(&10));
  assert_eq!(loaded.revision(), 1);

  let first_change = poll_ready(store.changes("orders", 0)).expect("load first change");
  assert_eq!(first_change.map(|change| (change.offset(), *change.value())), Some((1, 10)));

  let no_second_change = poll_ready(store.changes("orders", 1)).expect("load missing second change");
  assert_eq!(no_second_change, None);
}

#[test]
fn revision_mismatch_rejects_delete_without_mutation() {
  let mut store = TestDurableStateStore::new();

  poll_ready(store.upsert_object(TEST_PERSISTENCE_ID, 0, 10, None)).expect("upsert initial state");

  let result = poll_ready(store.delete_object(TEST_PERSISTENCE_ID, 0));
  assert_eq!(result, Err(DurableStateError::delete_revision(TEST_PERSISTENCE_ID, 0, 1)));

  let loaded = poll_ready(store.get_object(TEST_PERSISTENCE_ID)).expect("get durable state after failed delete");
  assert_eq!(loaded.value(), Some(&10));
  assert_eq!(loaded.revision(), 1);
}

#[test]
fn register_duplicate_provider_fails() {
  let mut registry = DurableStateStoreRegistry::<i32>::empty();
  let provider: ArcShared<dyn DurableStateStoreProvider<i32>> = ArcShared::new(TestDurableStateStoreProvider::new());

  registry.register(TEST_PROVIDER_ID, provider.clone()).expect("first register provider");

  let result = registry.register(TEST_PROVIDER_ID, provider);
  assert_eq!(result, Err(DurableStateError::ProviderAlreadyRegistered(TEST_PROVIDER_ID.to_string())));
}

#[test]
fn resolve_unknown_provider_fails() {
  let registry = DurableStateStoreRegistry::<i32>::empty();

  let result = registry.resolve(TEST_UNKNOWN_PROVIDER_ID);
  assert_eq!(result.err(), Some(DurableStateError::ProviderNotFound(TEST_UNKNOWN_PROVIDER_ID.to_string())));
}

#[test]
fn durable_state_update_store_reports_changes() {
  let mut store = TestDurableStateStore::new();

  // changes(tag, offset) は指定 offset より大きい tagged change を返す
  let no_change = poll_ready(store.changes("orders", 0)).expect("load changes before first upsert");
  assert_eq!(no_change, None);

  // 最初の tagged upsert -> offset=1, revision=1, value=10
  poll_ready(store.upsert_object(TEST_PERSISTENCE_ID, 0, 10, Some("orders"))).expect("upsert first state");
  let first_change = poll_ready(store.changes("orders", 0)).expect("load first change");
  assert_eq!(
    first_change.as_ref().map(|change| (
      change.offset(),
      change.persistence_id(),
      change.revision(),
      change.tag(),
      *change.value()
    )),
    Some((1, TEST_PERSISTENCE_ID, 1, "orders", 10))
  );

  // offset=1 以降は未更新なので None
  let no_second_change = poll_ready(store.changes("orders", 1)).expect("load second change before upsert");
  assert_eq!(no_second_change, None);

  // 2回目の tagged upsert -> offset=2, revision=2, value=20
  poll_ready(store.upsert_object(TEST_PERSISTENCE_ID, 1, 20, Some("orders"))).expect("upsert second state");
  let second_change = poll_ready(store.changes("orders", 1)).expect("load second change");
  assert_eq!(
    second_change.as_ref().map(|change| (change.offset(), change.revision(), change.tag(), *change.value())),
    Some((2, 2, "orders", 20))
  );

  // offset=2 以降は未更新なので None
  let no_third_change = poll_ready(store.changes("orders", 2)).expect("load third change");
  assert_eq!(no_third_change, None);
}

#[test]
fn untagged_update_is_not_returned_by_tag_query() {
  let mut store = TestDurableStateStore::new();

  poll_ready(store.upsert_object(TEST_PERSISTENCE_ID, 0, 10, None)).expect("upsert untagged state");

  let no_change = poll_ready(store.changes("orders", 0)).expect("load tagged changes");
  assert_eq!(no_change, None);
}

#[test]
fn durable_state_update_store_isolates_tags() {
  let mut store = TestDurableStateStore::new();

  poll_ready(store.upsert_object("order-1", 0, 10, Some("orders"))).expect("upsert order state");
  poll_ready(store.upsert_object("payment-1", 0, 20, Some("payments"))).expect("upsert payment state");

  let order_change = poll_ready(store.changes("orders", 0)).expect("load order change");
  assert_eq!(
    order_change.as_ref().map(|change| (
      change.offset(),
      change.persistence_id(),
      change.revision(),
      change.tag(),
      *change.value()
    )),
    Some((1, "order-1", 1, "orders", 10))
  );

  let no_second_order_change = poll_ready(store.changes("orders", 1)).expect("load missing order change");
  assert_eq!(no_second_order_change, None);
}
