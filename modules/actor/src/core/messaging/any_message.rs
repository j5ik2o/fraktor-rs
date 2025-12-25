//! Owned representation of a dynamically typed message.

#[cfg(test)]
mod tests;

use alloc::fmt;
use core::any::Any;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};

use crate::core::{actor_prim::actor_ref::ActorRefGeneric, messaging::AnyMessageViewGeneric};

/// Wraps an arbitrary payload for message passing.
pub struct AnyMessageGeneric<TB: RuntimeToolbox> {
  payload: ArcShared<dyn Any + Send + Sync + 'static>,
  sender:  Option<ActorRefGeneric<TB>>,
}

/// Type alias for [AnyMessageGeneric] with the default [NoStdToolbox].
pub type AnyMessage = AnyMessageGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox> AnyMessageGeneric<TB> {
  /// Creates a new owned message from the provided payload.
  #[must_use]
  pub fn new<T>(payload: T) -> Self
  where
    T: Any + Send + Sync + 'static, {
    Self { payload: ArcShared::new(payload), sender: None }
  }

  /// Associates a sender with this message and returns the updated instance.
  #[must_use]
  pub fn with_sender(mut self, sender: ActorRefGeneric<TB>) -> Self {
    self.sender = Some(sender);
    self
  }

  /// Returns the sender, if any.
  #[must_use]
  pub const fn sender(&self) -> Option<&ActorRefGeneric<TB>> {
    self.sender.as_ref()
  }

  /// Converts the owned message into a borrowed view.
  #[must_use]
  pub fn as_view(&self) -> AnyMessageViewGeneric<'_, TB> {
    AnyMessageViewGeneric::new(&*self.payload, self.sender.as_ref())
  }

  /// Reconstructs a message from an erased payload pointer.
  #[must_use]
  pub fn from_erased(payload: ArcShared<dyn Any + Send + Sync + 'static>, sender: Option<ActorRefGeneric<TB>>) -> Self {
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
  pub(crate) fn from_parts(
    payload: ArcShared<dyn Any + Send + Sync + 'static>,
    sender: Option<ActorRefGeneric<TB>>,
  ) -> Self {
    Self { payload, sender }
  }

  /// Consumes the message and returns the payload alongside the sender.
  pub(crate) fn into_payload_and_sender(
    self,
  ) -> (ArcShared<dyn Any + Send + Sync + 'static>, Option<ActorRefGeneric<TB>>) {
    (self.payload, self.sender)
  }
}

impl<TB: RuntimeToolbox> Clone for AnyMessageGeneric<TB> {
  fn clone(&self) -> Self {
    Self { payload: self.payload.clone(), sender: self.sender.clone() }
  }
}

impl<TB: RuntimeToolbox> fmt::Debug for AnyMessageGeneric<TB> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("AnyMessage")
      .field("type_id", &self.payload.type_id())
      .field("has_sender", &self.sender.is_some())
      .finish()
  }
}
