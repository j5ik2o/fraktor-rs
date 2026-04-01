//! Routing logic abstraction.

#[cfg(test)]
mod tests;

use crate::core::kernel::{actor::messaging::AnyMessage, routing::Routee};

/// Determines how a message is routed to one of the available routees.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.RoutingLogic`.
///
/// Implementations must be safe to call from multiple threads concurrently.
pub trait RoutingLogic: Send + Sync + 'static {
  /// Selects a routee for the given message from the provided slice.
  ///
  /// When `routees` is empty, implementations should return a reference to a
  /// static [`Routee::NoRoutee`] sentinel.
  fn select<'a>(&self, message: &AnyMessage, routees: &'a [Routee]) -> &'a Routee;
}
