//! Owned representation of a dynamically typed message.

#[cfg(test)]
mod tests;

use alloc::fmt::{Debug, Formatter, Result as FmtResult};
use core::any::Any;

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::actor::{
  actor_ref::ActorRef,
  messaging::{AnyMessageView, NotInfluenceReceiveTimeout},
};

/// Wraps an arbitrary payload for message passing.
pub struct AnyMessage {
  payload: ArcShared<dyn Any + Send + Sync + 'static>,
  sender: Option<ActorRef>,
  is_control: bool,
  not_influence_receive_timeout: bool,
}

impl AnyMessage {
  /// Creates a new owned message from the provided payload.
  ///
  /// The resulting envelope carries `not_influence_receive_timeout = false`,
  /// so the receiving actor will reset its receive timeout as usual after
  /// successful delivery. To opt the payload out of that reset, the
  /// payload type must implement [`NotInfluenceReceiveTimeout`] and the
  /// envelope must be built with [`Self::not_influence`] instead — this
  /// `new` path never inspects the trait.
  #[must_use]
  pub fn new<T>(payload: T) -> Self
  where
    T: Any + Send + Sync + 'static, {
    Self { payload: ArcShared::new(payload), sender: None, is_control: false, not_influence_receive_timeout: false }
  }

  /// Creates a new owned message marked as a control message.
  ///
  /// Control messages are prioritised by control-aware mailboxes. Just like
  /// [`Self::new`], this constructor always sets
  /// `not_influence_receive_timeout = false`; use [`Self::not_influence`]
  /// if the receive timeout must be preserved across delivery.
  #[must_use]
  pub fn control<T>(payload: T) -> Self
  where
    T: Any + Send + Sync + 'static, {
    Self { payload: ArcShared::new(payload), sender: None, is_control: true, not_influence_receive_timeout: false }
  }

  /// Creates a new owned message whose successful delivery must not reset
  /// the receiving actor's receive timeout.
  ///
  /// The trait bound forces the payload type to implement
  /// [`NotInfluenceReceiveTimeout`]. The resulting envelope carries
  /// `not_influence_receive_timeout = true`, which
  /// [`ActorCellInvoker::invoke`](crate::actor::ActorCell)
  /// inspects to skip the `reschedule_receive_timeout` call that normally
  /// fires after a successful user message invocation.
  ///
  /// This is the Rust mirror of Pekko's
  /// `!message.isInstanceOf[NotInfluenceReceiveTimeout]` check
  /// (`dungeon/ReceiveTimeout.scala:40-42`): in Rust the
  /// trait-object downcast is impossible from `dyn Any`, so the marker
  /// contract is collapsed into this compile-time-checked flag.
  ///
  /// # Examples
  ///
  /// Payloads that do not implement [`NotInfluenceReceiveTimeout`] are
  /// rejected at compile time (`E0277`):
  ///
  /// ```compile_fail,E0277
  /// use fraktor_actor_core_rs::actor::messaging::AnyMessage;
  /// struct RegularMsg;
  /// // RegularMsg does not implement NotInfluenceReceiveTimeout,
  /// // so the trait bound on `not_influence` rejects this call.
  /// let _ = AnyMessage::not_influence(RegularMsg);
  /// ```
  #[must_use]
  pub fn not_influence<T>(payload: T) -> Self
  where
    T: NotInfluenceReceiveTimeout + Any + Send + Sync + 'static, {
    Self { payload: ArcShared::new(payload), sender: None, is_control: false, not_influence_receive_timeout: true }
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

  /// Returns `true` when this message's payload must not reset the
  /// receiving actor's receive timeout (Pekko `NotInfluenceReceiveTimeout`
  /// contract, `Actor.scala:165`).
  #[must_use]
  pub const fn is_not_influence_receive_timeout(&self) -> bool {
    self.not_influence_receive_timeout
  }

  /// Converts the owned message into a borrowed view.
  #[must_use]
  pub fn as_view(&self) -> AnyMessageView<'_> {
    AnyMessageView::with_flags(
      &*self.payload,
      self.sender.as_ref(),
      self.is_control,
      self.not_influence_receive_timeout,
    )
  }

  /// Reconstructs a message from an erased payload pointer.
  #[must_use]
  pub fn from_erased(
    payload: ArcShared<dyn Any + Send + Sync + 'static>,
    sender: Option<ActorRef>,
    is_control: bool,
    not_influence_receive_timeout: bool,
  ) -> Self {
    Self::from_parts(payload, sender, is_control, not_influence_receive_timeout)
  }

  /// Returns the payload as a trait object reference.
  #[must_use]
  pub fn payload(&self) -> &(dyn Any + Send + Sync + 'static) {
    &*self.payload
  }

  /// Attempts to downcast the payload to a concrete type.
  #[must_use]
  pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
    self.payload.downcast_ref::<T>()
  }

  /// Returns a clone of the shared payload pointer (internal use).
  #[doc(hidden)]
  #[must_use]
  pub fn payload_arc(&self) -> ArcShared<dyn Any + Send + Sync + 'static> {
    self.payload.clone()
  }

  /// Reconstructs an envelope from erased components.
  #[doc(hidden)]
  #[must_use]
  pub fn from_parts(
    payload: ArcShared<dyn Any + Send + Sync + 'static>,
    sender: Option<ActorRef>,
    is_control: bool,
    not_influence_receive_timeout: bool,
  ) -> Self {
    Self { payload, sender, is_control, not_influence_receive_timeout }
  }

  /// Consumes the message and returns the payload, sender, and flags.
  #[doc(hidden)]
  #[must_use]
  pub fn into_parts(self) -> (ArcShared<dyn Any + Send + Sync + 'static>, Option<ActorRef>, bool, bool) {
    (self.payload, self.sender, self.is_control, self.not_influence_receive_timeout)
  }
}

impl Clone for AnyMessage {
  fn clone(&self) -> Self {
    Self {
      payload: self.payload.clone(),
      sender: self.sender.clone(),
      is_control: self.is_control,
      not_influence_receive_timeout: self.not_influence_receive_timeout,
    }
  }
}

impl Debug for AnyMessage {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.debug_struct("AnyMessage")
      .field("type_id", &self.payload.type_id())
      .field("has_sender", &self.sender.is_some())
      .field("is_control", &self.is_control)
      .field("not_influence_receive_timeout", &self.not_influence_receive_timeout)
      .finish()
  }
}
