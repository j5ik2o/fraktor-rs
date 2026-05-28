use fraktor_actor_core_kernel_rs::actor::actor_ref::ActorRef;
use fraktor_actor_core_typed_rs::TypedActorRef;
use fraktor_persistence_core_kernel_rs::state::GetObjectResult;

use crate::internal::{StateSourcedStoreCommand, StateSourcedStoreReply};

type TestCommand = StateSourcedStoreCommand<u32>;

fn reply_ref() -> TypedActorRef<StateSourcedStoreReply<u32>> {
  TypedActorRef::from_untyped(ActorRef::no_sender())
}

#[test]
fn user_facing_store_commands_are_not_completions() {
  let recover = TestCommand::Recover { reply_to: reply_ref() };
  let persist = TestCommand::PersistState {
    state:             42,
    expected_revision: 1,
    tag:               None,
    reply_to:          reply_ref(),
  };
  let delete = TestCommand::DeleteState { expected_revision: 2, reply_to: reply_ref() };

  assert!(!recover.is_completion());
  assert!(!persist.is_completion());
  assert!(!delete.is_completion());
}

#[test]
fn store_completion_commands_are_completions() {
  let recovery = TestCommand::RecoveryFinished { result: Ok(GetObjectResult::empty()), reply_to: reply_ref() };
  let persist = TestCommand::PersistStateFinished {
    state:             42,
    expected_revision: 1,
    result:            Ok(()),
    reply_to:          reply_ref(),
  };
  let delete = TestCommand::DeleteStateFinished {
    expected_revision: 2,
    result:            Ok(()),
    reply_to:          reply_ref(),
  };

  assert!(recovery.is_completion());
  assert!(persist.is_completion());
  assert!(delete.is_completion());
}
