//! Routing logic abstraction.

#[cfg(test)]
#[path = "routing_logic_test.rs"]
mod tests;

use super::routee::Routee;
use crate::actor::messaging::AnyMessage;

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

  /// Selects the index of a routee for message-independent routing strategies.
  ///
  /// **Callers must ensure `!routees.is_empty()`**; the default implementation
  /// asserts this precondition and falls back to [`Self::select`] with a dummy
  /// `()` message, performing a linear lookup of the returned routee's
  /// position. Implementations that do not depend on message content (e.g.
  /// `SmallestMailboxRoutingLogic`, round-robin, random) should override this
  /// to avoid the allocation of a dummy [`AnyMessage`] and the `O(n)` pid
  /// scan.
  ///
  /// # Panics
  ///
  /// Panics if `routees` is empty.
  fn select_index(&self, routees: &[Routee]) -> usize {
    assert!(!routees.is_empty(), "RoutingLogic::select_index requires non-empty routees");
    let dummy = AnyMessage::new(());
    let selected = self.select(&dummy, routees);
    match selected {
      | Routee::ActorRef(aref) => {
        let selected_pid = aref.pid();
        routees
          .iter()
          .position(|r| match r {
            | Routee::ActorRef(other) => other.pid() == selected_pid,
            | _ => false,
          })
          .unwrap_or(0)
      },
      | Routee::NoRoutee | Routee::Several(_) => 0,
    }
  }
}
