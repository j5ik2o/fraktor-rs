use core::mem::size_of;

use fraktor_actor_core_kernel_rs::actor::actor_ref::ActorRef;
use fraktor_actor_core_typed_rs::TypedActorRef;

use crate::{
  PersistenceId, StateSourcedEffector, StateSourcedEffectorConfig, StateSourcedEffectorSignal,
  internal::{StateSourcedStoreCommand, StateSourcedStoreReply},
  state_sourced_effector_signal_auth::StateSourcedEffectorSignalAuth,
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum AggregateCommand {
  Signal(StateSourcedEffectorSignal<u32>),
}

fn typed_ref<M>() -> TypedActorRef<M>
where
  M: Send + Sync + 'static, {
  TypedActorRef::from_untyped(ActorRef::no_sender())
}

fn effector(revision: u64) -> StateSourcedEffector<u32, AggregateCommand> {
  StateSourcedEffector::active(
    StateSourcedEffectorConfig::new(PersistenceId::of_unique_id("state-sourced-effector-test")),
    typed_ref::<StateSourcedStoreCommand<u32>>(),
    typed_ref::<StateSourcedStoreReply<u32>>(),
    revision,
  )
}

#[test]
fn effector_handle_type_is_available() {
  let _message = AggregateCommand::Signal(StateSourcedEffectorSignal::RecoveryCompleted {
    auth:     StateSourcedEffectorSignalAuth::new(),
    state:    Some(1),
    revision: 1,
  });

  assert!(size_of::<StateSourcedEffector<u32, AggregateCommand>>() > 0);
}

#[test]
fn active_effector_reports_recovered_revision_boundaries() {
  for revision in [0, 1, u64::MAX] {
    assert_eq!(effector(revision).revision(), revision);
  }
}

#[test]
fn cloned_effector_shares_revision_cell() {
  let effector = effector(1);
  let cloned = effector.clone();

  cloned.revision.with_lock(|revision| {
    *revision = 2;
  });

  assert_eq!(effector.revision(), 2);
  assert_eq!(cloned.revision(), 2);
}
