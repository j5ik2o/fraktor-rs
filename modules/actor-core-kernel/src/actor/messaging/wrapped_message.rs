//! Trait for envelopes that expose their wrapped actor message.
//!
//! Mirrors Pekko's `WrappedMessage` trait
//! (`references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorRef.scala:623`).

use core::any::Any;

use crate::actor::messaging::AnyMessage;

/// Envelope trait for event-stream and dead-letter messages that wrap a user
/// message.
pub trait WrappedMessage: Any + Send + Sync {
  /// Returns the wrapped message.
  #[must_use]
  fn message(&self) -> &AnyMessage;
}
