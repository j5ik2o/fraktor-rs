use core::any::{Any, TypeId};

use cellactor_utils_core_rs::sync::ArcShared;

use crate::actor_ref::ActorRef;

/// Borrowed representation of a dynamically typed message.
#[derive(Debug)]
pub struct AnyMessage<'a> {
  payload:  &'a (dyn Any + Send + Sync + 'static),
  type_id:  TypeId,
  reply_to: Option<&'a ActorRef>,
}

impl<'a> AnyMessage<'a> {
  /// Creates a new borrowed message.
  #[must_use]
  pub fn new(payload: &'a (dyn Any + Send + Sync + 'static), reply_to: Option<&'a ActorRef>) -> Self {
    Self { payload, type_id: (*payload).type_id(), reply_to }
  }

  /// Returns the [`TypeId`] of the payload.
  #[must_use]
  pub const fn type_id(&self) -> TypeId {
    self.type_id
  }

  /// Attempts to downcast the payload reference to the requested type.
  #[must_use]
  pub fn downcast_ref<T: Any + Send + Sync + 'static>(&self) -> Option<&'a T> {
    self.payload.downcast_ref::<T>()
  }

  /// Returns the reply target if present.
  #[must_use]
  pub const fn reply_to(&self) -> Option<&'a ActorRef> {
    self.reply_to
  }
}

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
  pub fn reply_to(&self) -> Option<&ActorRef> {
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
