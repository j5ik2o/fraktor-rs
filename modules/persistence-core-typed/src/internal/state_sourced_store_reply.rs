//! Internal state-sourced store replies.

#[cfg(test)]
#[path = "state_sourced_store_reply_test.rs"]
mod tests;

use fraktor_persistence_core_kernel_rs::state::DurableStateError;

use crate::{StateSourcedEffectorSignal, state_sourced_effector_signal_auth::StateSourcedEffectorSignalAuth};

#[derive(Clone, Debug)]
pub(crate) enum StateSourcedStoreReply<S> {
  RecoveryCompleted { state: Option<S>, revision: u64 },
  RecoveryFailed { error: DurableStateError },
  StatePersisted { state: S, revision: u64 },
  StateDeleted { revision: u64 },
  PersistenceFailed { error: DurableStateError },
}

impl<S> From<StateSourcedStoreReply<S>> for StateSourcedEffectorSignal<S> {
  fn from(reply: StateSourcedStoreReply<S>) -> Self {
    match reply {
      | StateSourcedStoreReply::RecoveryCompleted { state, revision } => {
        Self::RecoveryCompleted { auth: StateSourcedEffectorSignalAuth::new(), state, revision }
      },
      | StateSourcedStoreReply::RecoveryFailed { error } => {
        Self::RecoveryFailed { auth: StateSourcedEffectorSignalAuth::new(), error }
      },
      | StateSourcedStoreReply::StatePersisted { state, revision } => {
        Self::StatePersisted { auth: StateSourcedEffectorSignalAuth::new(), state, revision }
      },
      | StateSourcedStoreReply::StateDeleted { revision } => {
        Self::StateDeleted { auth: StateSourcedEffectorSignalAuth::new(), revision }
      },
      | StateSourcedStoreReply::PersistenceFailed { error } => {
        Self::PersistenceFailed { auth: StateSourcedEffectorSignalAuth::new(), error }
      },
    }
  }
}
