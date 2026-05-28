use alloc::boxed::Box;
use core::{
  future::{Future, ready},
  pin::Pin,
};

use fraktor_persistence_core_kernel_rs::state::{
  DurableStateError, DurableStateStore, DurableStateStoreProvider, GetObjectResult,
};
use fraktor_utils_core_rs::sync::ArcShared;

use crate::{
  PersistenceId, StateSourcedEffectorConfig, StateSourcedEffectorMessageAdapter, StateSourcedEffectorSignal,
  state_sourced_effector_signal_auth::StateSourcedEffectorSignalAuth,
};

type StoreFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, DurableStateError>> + Send + 'a>>;

#[derive(Clone, Debug, PartialEq, Eq)]
enum AggregateCommand {
  Signal(StateSourcedEffectorSignal<u32>),
}

struct TestStore;

impl DurableStateStore<u32> for TestStore {
  fn get_object<'a>(&'a self, _persistence_id: &'a str) -> StoreFuture<'a, GetObjectResult<u32>> {
    Box::pin(ready(Ok(GetObjectResult::empty())))
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

struct TestStoreProvider;

impl DurableStateStoreProvider<u32> for TestStoreProvider {
  fn durable_state_store(&self) -> Box<dyn DurableStateStore<u32>> {
    Box::new(TestStore)
  }
}

#[test]
fn default_stash_capacity_is_bounded() {
  let config = StateSourcedEffectorConfig::<u32, AggregateCommand>::new(PersistenceId::of_unique_id("state-pid"));

  assert_eq!(config.stash_capacity(), 1000);
}

#[test]
fn config_does_not_require_event_type_or_apply_event() {
  let config = StateSourcedEffectorConfig::<u32, AggregateCommand>::new(PersistenceId::of_unique_id("state-pid"));

  assert_eq!(config.persistence_id().as_str(), "state-pid");
}

#[test]
fn zero_stash_capacity_is_rejected() {
  let config = complete_config().with_stash_capacity(0);

  assert!(config.validate().is_err());
}

#[test]
fn with_message_adapter_stores_adapter() {
  let adapter = StateSourcedEffectorMessageAdapter::new(AggregateCommand::Signal, |message| match message {
    | AggregateCommand::Signal(signal) => Some(signal),
  });
  let config = StateSourcedEffectorConfig::new(PersistenceId::of_unique_id("state-pid")).with_message_adapter(adapter);
  let signal =
    StateSourcedEffectorSignal::StateDeleted { auth: StateSourcedEffectorSignalAuth::new(), revision: 3 };

  let message = match config.message_adapter() {
    | Some(adapter) => adapter.wrap_signal(signal),
    | None => panic!("adapter should be configured"),
  };

  assert!(matches!(message, AggregateCommand::Signal(StateSourcedEffectorSignal::StateDeleted { revision: 3, .. })));
}

#[test]
fn with_store_provider_stores_provider() {
  let config = StateSourcedEffectorConfig::<u32, AggregateCommand>::new(PersistenceId::of_unique_id("state-pid"))
    .with_store_provider(store_provider());

  assert!(config.store_provider().is_some());
}

#[test]
fn validate_requires_message_adapter_and_store_provider() {
  let missing_all = StateSourcedEffectorConfig::<u32, AggregateCommand>::new(PersistenceId::of_unique_id("state-pid"));
  let missing_provider =
    StateSourcedEffectorConfig::new(PersistenceId::of_unique_id("state-pid")).with_message_adapter(message_adapter());
  let complete = complete_config();

  assert!(missing_all.validate().is_err());
  assert!(missing_provider.validate().is_err());
  assert!(complete.validate().is_ok());
}

fn complete_config() -> StateSourcedEffectorConfig<u32, AggregateCommand> {
  StateSourcedEffectorConfig::new(PersistenceId::of_unique_id("state-pid"))
    .with_message_adapter(message_adapter())
    .with_store_provider(store_provider())
}

fn message_adapter() -> StateSourcedEffectorMessageAdapter<u32, AggregateCommand> {
  StateSourcedEffectorMessageAdapter::new(AggregateCommand::Signal, |message| match message {
    | AggregateCommand::Signal(signal) => Some(signal),
  })
}

fn store_provider() -> ArcShared<dyn DurableStateStoreProvider<u32>> {
  ArcShared::new(TestStoreProvider)
}
