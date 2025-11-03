//! Owned representation of a dynamically typed message.

use alloc::fmt;
use core::any::Any;

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{RuntimeToolbox, actor_ref::ActorRef, any_message_view::AnyMessageView};

/// Wraps an arbitrary payload for message passing.
pub struct AnyMessage<TB: RuntimeToolbox> {
  payload:  ArcShared<dyn Any + Send + Sync + 'static>,
  reply_to: Option<ActorRef<TB>>,
}

impl<TB: RuntimeToolbox> AnyMessage<TB> {
  /// Creates a new owned message from the provided payload.
  #[must_use]
  pub fn new<T>(payload: T) -> Self
  where
    T: Any + Send + Sync + 'static, {
    Self { payload: ArcShared::new(payload), reply_to: None }
  }

  /// Associates a reply target with this message and returns the updated instance.
  #[must_use]
  pub fn with_reply_to(mut self, reply_to: ActorRef<TB>) -> Self {
    self.reply_to = Some(reply_to);
    self
  }

  /// Returns the reply target, if any.
  #[must_use]
  pub const fn reply_to(&self) -> Option<&ActorRef<TB>> {
    self.reply_to.as_ref()
  }

  /// Converts the owned message into a borrowed view.
  #[must_use]
  pub fn as_view(&self) -> AnyMessageView<'_, TB> {
    AnyMessageView::new(&*self.payload, self.reply_to.as_ref())
  }

  /// Returns the payload as a trait object reference.
  #[must_use]
  pub fn payload(&self) -> &(dyn Any + Send + Sync + 'static) {
    &*self.payload
  }
}

impl<TB: RuntimeToolbox> Clone for AnyMessage<TB> {
  fn clone(&self) -> Self {
    Self { payload: self.payload.clone(), reply_to: self.reply_to.clone() }
  }
}

impl<TB: RuntimeToolbox> fmt::Debug for AnyMessage<TB> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("AnyMessage")
      .field("type_id", &self.payload.type_id())
      .field("has_reply_to", &self.reply_to.is_some())
      .finish()
  }
}
