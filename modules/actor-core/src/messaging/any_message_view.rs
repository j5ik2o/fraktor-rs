//! Borrowed representation of a dynamically typed message.

#[cfg(test)]
mod tests;

use core::any::{Any, TypeId};

use crate::{NoStdToolbox, RuntimeToolbox, actor_prim::actor_ref::ActorRef};

/// Represents a borrowed view of an actor message.
#[derive(Debug)]
pub struct AnyMessageView<'a, TB: RuntimeToolbox = NoStdToolbox> {
  payload:  &'a (dyn Any + Send + Sync + 'static),
  type_id:  TypeId,
  reply_to: Option<&'a ActorRef<TB>>,
}

impl<'a, TB: RuntimeToolbox> AnyMessageView<'a, TB> {
  /// Creates a new borrowed message view.
  #[must_use]
  pub fn new(payload: &'a (dyn Any + Send + Sync + 'static), reply_to: Option<&'a ActorRef<TB>>) -> Self {
    Self { payload, type_id: (*payload).type_id(), reply_to }
  }

  /// Returns the [`TypeId`] of the payload.
  #[must_use]
  pub const fn type_id(&self) -> TypeId {
    self.type_id
  }

  /// Attempts to downcast the payload to the requested type.
  #[must_use]
  pub fn downcast_ref<T: Any + Send + Sync + 'static>(&self) -> Option<&'a T> {
    self.payload.downcast_ref::<T>()
  }

  /// Returns the reply target if present.
  #[must_use]
  pub const fn reply_to(&self) -> Option<&'a ActorRef<TB>> {
    self.reply_to
  }
}
