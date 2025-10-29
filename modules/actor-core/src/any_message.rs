//! Untyped message container abstraction.

use core::any::{Any, TypeId};

use crate::actor_ref::ActorRef;

/// Borrowed representation of an untyped message payload.
#[derive(Debug, Clone, Copy)]
pub struct AnyMessage<'a> {
  payload:  &'a dyn Any,
  metadata: Option<&'a dyn Any>,
  type_id:  TypeId,
  reply_to: Option<&'a ActorRef>,
}

impl<'a> AnyMessage<'a> {
  /// Creates a message view from a typed payload.
  #[must_use]
  pub fn from_ref<T: Any + 'a>(value: &'a T) -> Self {
    Self::from_dyn_with_reply(value, None, TypeId::of::<T>(), None)
  }

  /// Creates a message view from a dynamic payload and explicit metadata.
  #[must_use]
  pub fn from_dyn(payload: &'a dyn Any, metadata: Option<&'a dyn Any>, type_id: TypeId) -> Self {
    Self::from_dyn_with_reply(payload, metadata, type_id, None)
  }

  /// Creates a message view from a dynamic payload, metadata, and reply target.
  #[must_use]
  pub fn from_dyn_with_reply(
    payload: &'a dyn Any,
    metadata: Option<&'a dyn Any>,
    type_id: TypeId,
    reply_to: Option<&'a ActorRef>,
  ) -> Self {
    Self { payload, metadata, type_id, reply_to }
  }

  /// Creates a message view with attached metadata.
  #[must_use]
  pub fn from_ref_with_metadata<T, M>(value: &'a T, metadata: &'a M) -> Self
  where
    T: Any + 'a,
    M: Any + 'a, {
    Self::from_dyn_with_reply(value, Some(metadata), TypeId::of::<T>(), None)
  }

  /// Returns the type identifier of the payload.
  #[must_use]
  pub const fn type_id(&self) -> TypeId {
    self.type_id
  }

  /// Attempts to borrow the payload as the requested type.
  #[must_use]
  pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
    if self.type_id == TypeId::of::<T>() { self.payload.downcast_ref::<T>() } else { None }
  }

  /// Returns the attached metadata if present.
  #[must_use]
  pub fn metadata(&self) -> Option<&'a dyn Any> {
    self.metadata
  }

  /// Returns the reply target if available.
  #[must_use]
  pub fn reply_to(&self) -> Option<&'a ActorRef> {
    self.reply_to
  }
}
