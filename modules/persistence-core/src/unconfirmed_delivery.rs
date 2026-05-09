//! Unconfirmed delivery entry.

#[cfg(test)]
mod tests;

use core::any::Any;

use fraktor_actor_core_kernel_rs::actor::actor_ref::ActorRef;
use fraktor_utils_core_rs::{sync::ArcShared, time::TimerInstant};

/// Unconfirmed delivery tracked by at-least-once delivery.
pub struct UnconfirmedDelivery {
  delivery_id: u64,
  destination: ActorRef,
  payload:     ArcShared<dyn Any + Send + Sync>,
  sender:      Option<ActorRef>,
  timestamp:   TimerInstant,
  attempt:     u32,
}

impl UnconfirmedDelivery {
  /// Creates a new unconfirmed delivery.
  #[must_use]
  pub fn new(
    delivery_id: u64,
    destination: ActorRef,
    payload: ArcShared<dyn Any + Send + Sync>,
    sender: Option<ActorRef>,
    timestamp: TimerInstant,
    attempt: u32,
  ) -> Self {
    Self { delivery_id, destination, payload, sender, timestamp, attempt }
  }

  /// Returns the delivery id.
  #[must_use]
  pub const fn delivery_id(&self) -> u64 {
    self.delivery_id
  }

  /// Returns the destination actor reference.
  #[must_use]
  pub const fn destination(&self) -> &ActorRef {
    &self.destination
  }

  /// Returns the payload.
  #[must_use]
  pub fn payload(&self) -> &(dyn Any + Send + Sync) {
    &*self.payload
  }

  /// Returns a cloned payload handle for resend.
  #[must_use]
  pub fn payload_arc(&self) -> ArcShared<dyn Any + Send + Sync> {
    self.payload.clone()
  }

  /// Returns the sender if present.
  #[must_use]
  pub const fn sender(&self) -> Option<&ActorRef> {
    self.sender.as_ref()
  }

  /// Returns the delivery timestamp.
  #[must_use]
  pub const fn timestamp(&self) -> TimerInstant {
    self.timestamp
  }

  /// Returns the number of delivery attempts.
  #[must_use]
  pub const fn attempt(&self) -> u32 {
    self.attempt
  }

  /// Marks this delivery as redelivered and updates timestamp/attempt.
  pub const fn mark_redelivered(&mut self, timestamp: TimerInstant) {
    self.timestamp = timestamp;
    self.attempt = self.attempt.saturating_add(1);
  }
}

impl Clone for UnconfirmedDelivery {
  fn clone(&self) -> Self {
    Self {
      delivery_id: self.delivery_id,
      destination: self.destination.clone(),
      payload:     self.payload.clone(),
      sender:      self.sender.clone(),
      timestamp:   self.timestamp,
      attempt:     self.attempt,
    }
  }
}
