//! Internal persistence store replies.

use alloc::vec::Vec;

use fraktor_persistence_core_kernel_rs::PersistenceError;

use crate::PersistenceEffectorSignal;

#[derive(Clone, Debug)]
pub(crate) enum PersistenceStoreReply<S, E> {
  RecoveryCompleted { state: S, sequence_nr: u64 },
  PersistedEvents { events: Vec<E>, sequence_nr: u64 },
  PersistedSnapshot { snapshot: S, sequence_nr: u64 },
  DeletedSnapshots { to_sequence_nr: u64 },
  Failed { error: PersistenceError },
}

impl<S, E> From<PersistenceStoreReply<S, E>> for PersistenceEffectorSignal<S, E> {
  fn from(reply: PersistenceStoreReply<S, E>) -> Self {
    match reply {
      | PersistenceStoreReply::RecoveryCompleted { state, sequence_nr } => {
        Self::RecoveryCompleted { state, sequence_nr }
      },
      | PersistenceStoreReply::PersistedEvents { events, sequence_nr } => Self::PersistedEvents { events, sequence_nr },
      | PersistenceStoreReply::PersistedSnapshot { snapshot, sequence_nr } => {
        Self::PersistedSnapshot { snapshot, sequence_nr }
      },
      | PersistenceStoreReply::DeletedSnapshots { to_sequence_nr } => Self::DeletedSnapshots { to_sequence_nr },
      | PersistenceStoreReply::Failed { error } => Self::Failed { error },
    }
  }
}
