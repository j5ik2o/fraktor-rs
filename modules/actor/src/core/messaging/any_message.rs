//! Owned representation of a dynamically typed message.

#[cfg(test)]
mod tests;

use alloc::fmt;
use core::any::Any;

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{actor::actor_ref::ActorRef, messaging::AnyMessageView};

/// Wraps an arbitrary payload for message passing.
pub struct AnyMessage {
  payload: ArcShared<dyn Any + Send + Sync + 'static>,
  sender:  Option<ActorRef>,
}

impl AnyMessage {
  /// Creates a new owned message from the provided payload.
  #[must_use]
  pub fn new<T>(payload: T) -> Self
  where
    T: Any + Send + Sync + 'static, {
    Self { payload: ArcShared::new(payload), sender: None }
  }

  /// Associates a sender with this message and returns the updated instance.
  #[must_use]
  pub fn with_sender(mut self, sender: ActorRef) -> Self {
    self.sender = Some(sender);
    self
  }

  /// Returns the sender, if any.
  #[must_use]
  pub const fn sender(&self) -> Option<&ActorRef> {
    self.sender.as_ref()
  }

  /// Converts the owned message into a borrowed view.
  #[must_use]
  pub fn as_view(&self) -> AnyMessageView<'_> {
    AnyMessageView::new(&*self.payload, self.sender.as_ref())
  }

  /// Reconstructs a message from an erased payload pointer.
  #[must_use]
  pub fn from_erased(payload: ArcShared<dyn Any + Send + Sync + 'static>, sender: Option<ActorRef>) -> Self {
    Self::from_parts(payload, sender)
  }

  /// Returns the payload as a trait object reference.
  #[must_use]
  pub fn payload(&self) -> &(dyn Any + Send + Sync + 'static) {
    &*self.payload
  }

  /// Returns a clone of the shared payload pointer (internal use).
  pub(crate) fn payload_arc(&self) -> ArcShared<dyn Any + Send + Sync + 'static> {
    self.payload.clone()
  }

  /// Reconstructs an envelope from erased components.
  pub(crate) fn from_parts(payload: ArcShared<dyn Any + Send + Sync + 'static>, sender: Option<ActorRef>) -> Self {
    Self { payload, sender }
  }

  /// Consumes the message and returns the payload alongside the sender.
  pub(crate) fn into_payload_and_sender(self) -> (ArcShared<dyn Any + Send + Sync + 'static>, Option<ActorRef>) {
    (self.payload, self.sender)
  }
}

impl Clone for AnyMessage {
  fn clone(&self) -> Self {
    Self { payload: self.payload.clone(), sender: self.sender.clone() }
  }
}

impl fmt::Debug for AnyMessage {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("AnyMessage")
      .field("type_id", &self.payload.type_id())
      .field("has_sender", &self.sender.is_some())
      .finish()
  }
}
