//! Borrowed representation of a dynamically typed message.

#[cfg(test)]
#[path = "any_message_view_test.rs"]
mod tests;

use core::any::{Any, TypeId};

use crate::actor::actor_ref::ActorRef;

/// Represents a borrowed view of an actor message.
#[derive(Debug)]
pub struct AnyMessageView<'a> {
  payload: &'a (dyn Any + Send + Sync + 'static),
  type_id: TypeId,
  sender: Option<&'a ActorRef>,
  is_control: bool,
  not_influence_receive_timeout: bool,
  dead_letter_suppressed: bool,
  possibly_harmful: bool,
}

impl<'a> AnyMessageView<'a> {
  /// Creates a new borrowed message view.
  #[must_use]
  pub fn new(payload: &'a (dyn Any + Send + Sync + 'static), sender: Option<&'a ActorRef>) -> Self {
    Self {
      payload,
      type_id: (*payload).type_id(),
      sender,
      is_control: false,
      not_influence_receive_timeout: false,
      dead_letter_suppressed: false,
      possibly_harmful: false,
    }
  }

  /// Creates a new borrowed message view with a control flag.
  ///
  /// The `not_influence_receive_timeout` flag is always `false` in this path;
  /// use [`Self::with_flags`] when the envelope needs to propagate the
  /// `NotInfluenceReceiveTimeout` marker.
  #[must_use]
  pub fn with_control(
    payload: &'a (dyn Any + Send + Sync + 'static),
    sender: Option<&'a ActorRef>,
    is_control: bool,
  ) -> Self {
    Self {
      payload,
      type_id: (*payload).type_id(),
      sender,
      is_control,
      not_influence_receive_timeout: false,
      dead_letter_suppressed: false,
      possibly_harmful: false,
    }
  }

  /// Creates a new borrowed message view carrying every envelope flag.
  ///
  /// Used by [`AnyMessage::as_view`](super::AnyMessage::as_view) to surface
  /// both the control-aware classification and the receive-timeout marker
  /// (Pekko `NotInfluenceReceiveTimeout`).
  #[must_use]
  pub fn with_flags(
    payload: &'a (dyn Any + Send + Sync + 'static),
    sender: Option<&'a ActorRef>,
    is_control: bool,
    not_influence_receive_timeout: bool,
    dead_letter_suppressed: bool,
    possibly_harmful: bool,
  ) -> Self {
    Self {
      payload,
      type_id: (*payload).type_id(),
      sender,
      is_control,
      not_influence_receive_timeout,
      dead_letter_suppressed,
      possibly_harmful,
    }
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

  /// Returns the sender if present.
  #[must_use]
  pub const fn sender(&self) -> Option<&'a ActorRef> {
    self.sender
  }

  /// Returns whether this message is a control message.
  #[must_use]
  pub const fn is_control(&self) -> bool {
    self.is_control
  }

  /// Returns whether this message carries the
  /// `NotInfluenceReceiveTimeout` marker (Pekko `Actor.scala:165`).
  #[must_use]
  pub const fn not_influence_receive_timeout(&self) -> bool {
    self.not_influence_receive_timeout
  }

  /// Returns whether this message carries the `DeadLetterSuppression` marker
  /// (Pekko `ActorRef.scala:573`).
  #[must_use]
  pub const fn dead_letter_suppressed(&self) -> bool {
    self.dead_letter_suppressed
  }

  /// Returns whether this message carries the `PossiblyHarmful` marker
  /// (Pekko `Actor.scala:39`).
  #[must_use]
  pub const fn possibly_harmful(&self) -> bool {
    self.possibly_harmful
  }
}
