//! Owned representation of a dynamically typed message.

use core::{any::Any, fmt};

use cellactor_utils_core_rs::sync::ArcShared;

use super::any_message_struct::AnyMessage;
use crate::actor_ref::ActorRef;

/// Owned representation of a dynamically typed message.
#[derive(Clone)]
pub struct AnyOwnedMessage {
  payload:  ArcShared<dyn Any + Send + Sync + 'static>,
  reply_to: Option<ActorRef>,
}

impl AnyOwnedMessage {
  /// Creates a new owned message.
  #[must_use]
  pub fn new<T>(payload: T) -> Self
  where
    T: Any + Send + Sync + 'static, {
    Self { payload: ArcShared::new(payload), reply_to: None }
  }

  /// Associates a reply target with the message.
  #[must_use]
  pub fn with_reply_to(mut self, reply_to: ActorRef) -> Self {
    self.reply_to = Some(reply_to);
    self
  }

  /// Returns the reply target.
  #[must_use]
  pub const fn reply_to(&self) -> Option<&ActorRef> {
    self.reply_to.as_ref()
  }

  /// Converts into a borrowed representation.
  #[must_use]
  pub fn as_any(&self) -> AnyMessage<'_> {
    AnyMessage::new(&*self.payload, self.reply_to.as_ref())
  }

  /// Returns the payload as a trait object reference.
  #[must_use]
  pub fn payload(&self) -> &(dyn Any + Send + Sync + 'static) {
    &*self.payload
  }
}

impl fmt::Debug for AnyOwnedMessage {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("AnyOwnedMessage")
      .field("type_id", &self.payload.type_id())
      .field("has_reply_to", &self.reply_to.is_some())
      .finish()
  }
}
