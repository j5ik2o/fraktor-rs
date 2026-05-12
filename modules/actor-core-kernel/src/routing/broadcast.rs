//! Broadcast message wrapper.

#[cfg(test)]
#[path = "broadcast_test.rs"]
mod tests;

use crate::actor::messaging::AnyMessage;

/// Wraps a message to indicate it should be sent to all routees.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.Broadcast`.
///
/// When a [`Router`](super::Router) receives a message whose payload is a
/// `Broadcast`, it unwraps the inner message and sends it to every routee
/// instead of using the configured [`RoutingLogic`](super::RoutingLogic).
#[derive(Debug)]
pub struct Broadcast(pub AnyMessage);
