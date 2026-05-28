//! Internal state-sourced store commands.

#[cfg(test)]
#[path = "state_sourced_store_command_test.rs"]
mod tests;

use alloc::{boxed::Box, string::String};

use fraktor_actor_core_typed_rs::TypedActorRef;
use fraktor_persistence_core_kernel_rs::state::{DurableStateError, DurableStateStore, GetObjectResult};

use super::StateSourcedStoreReply;

pub(crate) type StateSourcedStoreResult<T> = Result<T, DurableStateError>;
pub(crate) type StateSourcedStore<S> = Box<dyn DurableStateStore<S>>;

pub(crate) enum StateSourcedStoreCommand<S>
where
  S: Send + Sync + 'static, {
  Recover {
    reply_to: TypedActorRef<StateSourcedStoreReply<S>>,
  },
  RecoveryFinished {
    result:   StateSourcedStoreResult<GetObjectResult<S>>,
    reply_to: TypedActorRef<StateSourcedStoreReply<S>>,
  },
  PersistState {
    state:             S,
    expected_revision: u64,
    tag:               Option<String>,
    reply_to:          TypedActorRef<StateSourcedStoreReply<S>>,
  },
  PersistStateFinished {
    state:             S,
    expected_revision: u64,
    result:            StateSourcedStoreResult<()>,
    reply_to:          TypedActorRef<StateSourcedStoreReply<S>>,
  },
  DeleteState {
    expected_revision: u64,
    reply_to:          TypedActorRef<StateSourcedStoreReply<S>>,
  },
  DeleteStateFinished {
    expected_revision: u64,
    result:            StateSourcedStoreResult<()>,
    reply_to:          TypedActorRef<StateSourcedStoreReply<S>>,
  },
}

impl<S> StateSourcedStoreCommand<S>
where
  S: Send + Sync + 'static,
{
  pub(crate) const fn is_completion(&self) -> bool {
    matches!(self, Self::RecoveryFinished { .. } | Self::PersistStateFinished { .. } | Self::DeleteStateFinished { .. })
  }
}
