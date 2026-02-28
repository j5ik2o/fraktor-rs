//! Persistent envelope used during batching.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use fraktor_actor_rs::core::actor::Pid;
use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{event_adapters::EventAdapters, persistent_repr::PersistentRepr};

type PersistentHandler<A> = Box<dyn FnOnce(&mut A, &PersistentRepr) + Send + Sync>;

/// Persistent envelope holding event and handler.
pub struct PersistentEnvelope<A> {
  event:       ArcShared<dyn core::any::Any + Send + Sync>,
  sequence_nr: u64,
  handler:     PersistentHandler<A>,
  stashing:    bool,
  sender:      Option<Pid>,
}

impl<A> PersistentEnvelope<A> {
  /// Creates a new persistent envelope.
  #[must_use]
  pub fn new(
    event: ArcShared<dyn core::any::Any + Send + Sync>,
    sequence_nr: u64,
    handler: PersistentHandler<A>,
    stashing: bool,
    sender: Option<Pid>,
  ) -> Self {
    Self { event, sequence_nr, handler, stashing, sender }
  }

  /// Returns true when the envelope stashes commands.
  #[must_use]
  pub const fn is_stashing(&self) -> bool {
    self.stashing
  }

  /// Returns the sequence number.
  #[must_use]
  pub const fn sequence_nr(&self) -> u64 {
    self.sequence_nr
  }

  /// Converts the envelope into a persistent representation.
  #[must_use]
  pub fn into_persistent_repr(
    &self,
    persistence_id: impl Into<alloc::string::String>,
    adapters: EventAdapters,
  ) -> PersistentRepr {
    PersistentRepr::new(persistence_id, self.sequence_nr, self.event.clone())
      .with_sender(self.sender)
      .with_adapters(adapters)
  }

  /// Consumes the envelope and returns the stored handler.
  #[must_use]
  pub fn into_handler(self) -> PersistentHandler<A> {
    self.handler
  }
}
