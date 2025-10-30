//! Borrowed representation of a dynamically typed message.

use core::any::{Any, TypeId};

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
