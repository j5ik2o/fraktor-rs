//! Marker trait that opts a message type out of receive-timeout resets.
//!
//! Mirrors Pekko's `NotInfluenceReceiveTimeout` trait
//! (`references/pekko/actor/src/main/scala/org/apache/pekko/actor/Actor.scala:165`).
//! Messages whose payload type implements this trait must skip the
//! `reschedule_receive_timeout` call that normally runs after
//! [`ActorCellInvoker::invoke`] completes a user message successfully
//! (Pekko `dungeon/ReceiveTimeout.scala:40-42`).
//!
//! The marker is turned into a runtime flag by
//! [`AnyMessage::not_influence`](super::AnyMessage::not_influence): the trait
//! bound on that constructor forces callers to opt in at the type system
//! level, and the resulting `AnyMessage` carries
//! `not_influence_receive_timeout = true`. Wrapping the same payload with
//! [`AnyMessage::new`](super::AnyMessage::new) keeps the flag `false`, so the
//! mailbox machinery must rely on `not_influence` callers to surface the
//! marker.

use core::any::Any;

/// Marker trait declaring that a message type must not reset the receiving
/// actor's receive timeout when delivered successfully.
///
/// Implementors only need an empty `impl` block:
///
/// ```rust
/// use fraktor_actor_core_rs::core::kernel::actor::messaging::NotInfluenceReceiveTimeout;
/// struct Tick;
/// impl NotInfluenceReceiveTimeout for Tick {}
/// ```
///
/// Once the marker is in place, enqueue the message via
/// [`AnyMessage::not_influence`](super::AnyMessage::not_influence) so the
/// runtime flag is propagated. Sending the same payload through
/// [`AnyMessage::new`](super::AnyMessage::new) would silently reset the
/// timer because the flag stays at its default `false`.
pub trait NotInfluenceReceiveTimeout: Any + Send + Sync {}
