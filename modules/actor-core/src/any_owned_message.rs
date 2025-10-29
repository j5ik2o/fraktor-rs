//! Owned message container used inside mailboxes.

use alloc::{boxed::Box, string::String};
use core::any::{Any, TypeId};

use crate::actor_ref::ActorRef;

/// Owned representation of a pending message.
pub struct AnyOwnedMessage {
  payload:  Box<dyn Any>,
  reply_to: Option<ActorRef>,
  metadata: Option<Box<dyn Any>>,
  type_id:  TypeId,
  label:    Option<String>,
}

impl AnyOwnedMessage {
  /// Creates an owned envelope from a typed payload.
  #[must_use]
  pub fn new<T: Any>(payload: T) -> Self {
    let type_id = TypeId::of::<T>();
    Self { payload: Box::new(payload), reply_to: None, metadata: None, type_id, label: None }
  }

  /// Attaches metadata to the owned message.
  #[must_use]
  pub fn with_metadata(mut self, metadata: Box<dyn Any>) -> Self {
    self.metadata = Some(metadata);
    self
  }

  /// Attaches reply-to information for request/response patterns.
  #[must_use]
  pub fn with_reply_to(mut self, reply_to: ActorRef) -> Self {
    self.reply_to = Some(reply_to);
    self
  }

  /// Adds a diagnostic label used for logging or debugging.
  #[must_use]
  pub fn with_label<S>(mut self, label: S) -> Self
  where
    S: Into<String>, {
    self.label = Some(label.into());
    self
  }

  /// Returns the stored reply target, if any.
  #[must_use]
  pub fn reply_to(&self) -> Option<&ActorRef> {
    self.reply_to.as_ref()
  }

  /// Consumes the envelope and returns the reply target.
  #[must_use]
  pub fn take_reply_to(&mut self) -> Option<ActorRef> {
    self.reply_to.take()
  }

  /// Borrows the payload as a dynamic reference for message invocation.
  #[must_use]
  pub fn as_any(&self) -> &dyn Any {
    self.payload.as_ref()
  }

  /// Borrows the payload and constructs a transient [`AnyMessage`] view.
  #[must_use]
  pub fn borrow(&self) -> crate::any_message::AnyMessage<'_> {
    crate::any_message::AnyMessage::from_dyn_with_reply(self.as_any(), self.metadata(), self.type_id, self.reply_to())
  }

  /// Borrows the metadata if available.
  #[must_use]
  pub fn metadata(&self) -> Option<&dyn Any> {
    self.metadata.as_deref()
  }

  /// Returns the message type identifier.
  #[must_use]
  pub const fn type_id(&self) -> TypeId {
    self.type_id
  }

  /// Returns the diagnostic label.
  #[must_use]
  pub fn label(&self) -> Option<&str> {
    self.label.as_deref()
  }

  /// Attempts to downcast the payload into the requested type.
  #[must_use]
  pub fn downcast<T: Any>(self) -> Result<T, Self> {
    if self.type_id == TypeId::of::<T>() {
      match self.payload.downcast::<T>() {
        | Ok(value) => Ok(*value),
        | Err(payload) => Err(Self { payload, ..self }),
      }
    } else {
      Err(self)
    }
  }
}
