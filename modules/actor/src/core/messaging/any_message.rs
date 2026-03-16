//! Owned representation of a dynamically typed message.

#[cfg(test)]
mod tests;

use alloc::fmt;
use core::any::Any;

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{actor::actor_ref::ActorRef, messaging::AnyMessageView};

/// Wraps an arbitrary payload for message passing.
pub struct AnyMessage {
  payload:    ArcShared<dyn Any + Send + Sync + 'static>,
  sender:     Option<ActorRef>,
  is_control: bool,
}

impl AnyMessage {
  /// Creates a new owned message from the provided payload.
  #[must_use]
  pub fn new<T>(payload: T) -> Self
  where
    T: Any + Send + Sync + 'static, {
    Self { payload: ArcShared::new(payload), sender: None, is_control: false }
  }

  /// Creates a new owned message marked as a control message.
  ///
  /// Control messages are prioritised by control-aware mailboxes.
  #[must_use]
  pub fn control<T>(payload: T) -> Self
  where
    T: Any + Send + Sync + 'static, {
    Self { payload: ArcShared::new(payload), sender: None, is_control: true }
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

  /// Returns `true` when this message was created as a control message.
  #[must_use]
  pub const fn is_control(&self) -> bool {
    self.is_control
  }

  /// Converts the owned message into a borrowed view.
  #[must_use]
  pub fn as_view(&self) -> AnyMessageView<'_> {
    AnyMessageView::with_control(&*self.payload, self.sender.as_ref(), self.is_control)
  }

  /// Reconstructs a message from an erased payload pointer.
  #[must_use]
  pub fn from_erased(
    payload: ArcShared<dyn Any + Send + Sync + 'static>,
    sender: Option<ActorRef>,
    is_control: bool,
  ) -> Self {
    Self::from_parts(payload, sender, is_control)
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
  pub(crate) fn from_parts(
    payload: ArcShared<dyn Any + Send + Sync + 'static>,
    sender: Option<ActorRef>,
    is_control: bool,
  ) -> Self {
    Self { payload, sender, is_control }
  }

  /// Consumes the message and returns the payload, sender, and control flag.
  pub(crate) fn into_parts(self) -> (ArcShared<dyn Any + Send + Sync + 'static>, Option<ActorRef>, bool) {
    (self.payload, self.sender, self.is_control)
  }
}

impl Clone for AnyMessage {
  fn clone(&self) -> Self {
    Self { payload: self.payload.clone(), sender: self.sender.clone(), is_control: self.is_control }
  }
}

impl fmt::Debug for AnyMessage {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("AnyMessage")
      .field("type_id", &self.payload.type_id())
      .field("has_sender", &self.sender.is_some())
      .field("is_control", &self.is_control)
      .finish()
  }
}
