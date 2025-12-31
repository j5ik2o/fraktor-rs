//! Unconfirmed delivery entry.

#[cfg(test)]
mod tests;

use core::any::Any;

use fraktor_actor_rs::core::actor::actor_ref::ActorRefGeneric;
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared, time::TimerInstant};

/// Unconfirmed delivery tracked by at-least-once delivery.
pub struct UnconfirmedDelivery<TB: RuntimeToolbox + 'static> {
  delivery_id: u64,
  destination: ActorRefGeneric<TB>,
  payload:     ArcShared<dyn Any + Send + Sync>,
  sender:      Option<ActorRefGeneric<TB>>,
  timestamp:   TimerInstant,
}

impl<TB: RuntimeToolbox + 'static> UnconfirmedDelivery<TB> {
  /// Creates a new unconfirmed delivery.
  #[must_use]
  pub fn new(
    delivery_id: u64,
    destination: ActorRefGeneric<TB>,
    payload: ArcShared<dyn Any + Send + Sync>,
    sender: Option<ActorRefGeneric<TB>>,
    timestamp: TimerInstant,
  ) -> Self {
    Self { delivery_id, destination, payload, sender, timestamp }
  }

  /// Returns the delivery id.
  #[must_use]
  pub const fn delivery_id(&self) -> u64 {
    self.delivery_id
  }

  /// Returns the destination actor reference.
  #[must_use]
  pub const fn destination(&self) -> &ActorRefGeneric<TB> {
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
  pub const fn sender(&self) -> Option<&ActorRefGeneric<TB>> {
    self.sender.as_ref()
  }

  /// Returns the delivery timestamp.
  #[must_use]
  pub const fn timestamp(&self) -> TimerInstant {
    self.timestamp
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for UnconfirmedDelivery<TB> {
  fn clone(&self) -> Self {
    Self {
      delivery_id: self.delivery_id,
      destination: self.destination.clone(),
      payload:     self.payload.clone(),
      sender:      self.sender.clone(),
      timestamp:   self.timestamp,
    }
  }
}
