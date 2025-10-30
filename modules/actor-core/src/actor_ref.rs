//! Actor reference handle.

/// Handle used to communicate with an actor instance.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ActorRef {
  _private: (),
}

impl ActorRef {
  /// Creates a placeholder reference.
  #[must_use]
  pub const fn null() -> Self {
    Self { _private: () }
  }
}
