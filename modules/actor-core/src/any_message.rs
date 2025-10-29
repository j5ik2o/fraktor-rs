//! Borrowed and owned message abstractions for the untyped runtime.

mod metadata;
mod owned;

use core::any::{Any, TypeId};

pub use metadata::MessageMetadata;
pub use owned::AnyOwnedMessage;

use crate::actor_ref::ActorRef;

/// Borrowed dynamic message wrapper delivered to actor callbacks.
#[derive(Clone, Copy)]
pub struct AnyMessage<'a> {
  payload:  &'a (dyn Any + Send + Sync),
  type_id:  TypeId,
  metadata: Option<&'a MessageMetadata>,
  reply_to: Option<&'a ActorRef>,
}

impl<'a> AnyMessage<'a> {
  /// Creates a borrowed wrapper around the provided payload.
  #[must_use]
  pub fn new<T>(payload: &'a T) -> Self
  where
    T: Any + Send + Sync + 'a, {
    Self { payload, type_id: TypeId::of::<T>(), metadata: None, reply_to: None }
  }

  /// Returns the dynamic type identifier of the payload.
  #[must_use]
  pub const fn type_id(&self) -> TypeId {
    self.type_id
  }

  /// Attempts to downcast the payload reference to the requested type.
  #[must_use]
  pub fn downcast_ref<T>(&self) -> Option<&T>
  where
    T: Any, {
    self.payload.downcast_ref()
  }

  /// Returns metadata associated with the message if present.
  #[must_use]
  pub const fn metadata(&self) -> Option<&MessageMetadata> {
    self.metadata
  }

  /// Returns the reply target captured with the message.
  #[must_use]
  pub const fn reply_to(&self) -> Option<&ActorRef> {
    self.reply_to
  }

  pub(crate) fn from_parts(
    payload: &'a (dyn Any + Send + Sync),
    type_id: TypeId,
    metadata: Option<&'a MessageMetadata>,
    reply_to: Option<&'a ActorRef>,
  ) -> Self {
    Self { payload, type_id, metadata, reply_to }
  }
}
