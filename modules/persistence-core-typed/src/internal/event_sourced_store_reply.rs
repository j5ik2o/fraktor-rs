//! Internal event-sourced store replies.

use alloc::vec::Vec;

use crate::{
  EventSourcedEffectorSignal, EventSourcedSignal, PublishedEvent,
  event_sourced_effector_signal_auth::EventSourcedEffectorSignalAuth,
};

#[derive(Clone, Debug)]
pub(crate) enum EventSourcedStoreReply<S, E> {
  RecoveryCompleted { state: S, sequence_nr: u64 },
  PersistedEvents { events: Vec<E>, published_events: Vec<PublishedEvent<E>>, sequence_nr: u64 },
  PersistedSnapshot { snapshot: S, sequence_nr: u64 },
  DeletedSnapshots { to_sequence_nr: u64 },
  EventSourced { signal: EventSourcedSignal },
}

impl<S, E> From<EventSourcedStoreReply<S, E>> for EventSourcedEffectorSignal<S, E> {
  fn from(reply: EventSourcedStoreReply<S, E>) -> Self {
    match reply {
      | EventSourcedStoreReply::RecoveryCompleted { state, sequence_nr } => {
        Self::RecoveryCompleted { auth: EventSourcedEffectorSignalAuth::new(), state, sequence_nr }
      },
      | EventSourcedStoreReply::PersistedEvents { events, published_events, sequence_nr } => {
        Self::PersistedEvents { auth: EventSourcedEffectorSignalAuth::new(), events, published_events, sequence_nr }
      },
      | EventSourcedStoreReply::PersistedSnapshot { snapshot, sequence_nr } => {
        Self::PersistedSnapshot { auth: EventSourcedEffectorSignalAuth::new(), snapshot, sequence_nr }
      },
      | EventSourcedStoreReply::DeletedSnapshots { to_sequence_nr } => {
        Self::DeletedSnapshots { auth: EventSourcedEffectorSignalAuth::new(), to_sequence_nr }
      },
      | EventSourcedStoreReply::EventSourced { signal } => {
        Self::EventSourced { auth: EventSourcedEffectorSignalAuth::new(), signal }
      },
    }
  }
}
