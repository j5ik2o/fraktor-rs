//! Marker trait for messages that may be unsafe across untrusted remoting.
//!
//! Mirrors Pekko's `PossiblyHarmful` trait
//! (`references/pekko/actor/src/main/scala/org/apache/pekko/actor/Actor.scala:39`).
//! Payloads implementing this trait can be wrapped with
//! [`AnyMessage::possibly_harmful`](super::AnyMessage::possibly_harmful)
//! so transport code can inspect the erased envelope.

use core::any::Any;

/// Marker trait declaring that a message may be blocked by untrusted remoting.
pub trait PossiblyHarmful: Any + Send + Sync {}
