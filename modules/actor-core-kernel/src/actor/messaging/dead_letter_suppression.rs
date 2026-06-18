//! Marker trait for messages that should be published as suppressed dead letters.
//!
//! Mirrors Pekko's `DeadLetterSuppression` trait
//! (`references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorRef.scala:573`).
//! Payloads implementing this trait must be wrapped with
//! [`AnyMessage::dead_letter_suppressed`](super::AnyMessage::dead_letter_suppressed)
//! so the erased envelope carries the marker after type erasure.

use core::any::Any;

/// Marker trait declaring that a message should use suppressed dead-letter
/// observation when delivery fails.
pub trait DeadLetterSuppression: Any + Send + Sync {}
