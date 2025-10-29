//! Wrapper handle for child actors managed by a parent.

use core::ops::Deref;

use crate::ActorRef;

/// Reference to a spawned child actor.
#[derive(Clone)]
pub struct ChildRef(ActorRef);

impl ChildRef {
  /// Creates a new child reference wrapping the given actor reference.
  pub(crate) fn new(inner: ActorRef) -> Self {
    Self(inner)
  }

  /// Returns the inner actor reference.
  #[must_use]
  pub fn actor_ref(&self) -> &ActorRef {
    &self.0
  }
}

impl Deref for ChildRef {
  type Target = ActorRef;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
