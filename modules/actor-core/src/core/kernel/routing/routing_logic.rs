//! Routing logic abstraction.

#[cfg(test)]
mod tests;

use super::routee::Routee;
use crate::core::kernel::actor::messaging::AnyMessage;

/// Determines how a message is routed to one of the available routees.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.RoutingLogic`.
///
/// Implementations must be safe to call from multiple threads concurrently.
pub trait RoutingLogic: Send + Sync + 'static {
  /// Selects a routee for the given message from the provided slice.
  ///
  /// The returned reference must have the same lifetime `'a` as the input
  /// `routees` slice.
  ///
  /// When `routees` is empty, implementations should return a reference to a
  /// static [`Routee::NoRoutee`] sentinel, for example via
  /// `static NO_ROUTEE: Routee = Routee::NoRoutee;`.
  fn select<'a>(&self, message: &AnyMessage, routees: &'a [Routee]) -> &'a Routee;
}
