//! Smallest-mailbox routing logic.

#[cfg(test)]
mod tests;

use super::{routee::Routee, routing_logic::RoutingLogic};
use crate::core::kernel::actor::messaging::AnyMessage;

/// Selects the routee with the smallest observable mailbox.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.SmallestMailboxRoutingLogic`.
///
/// Only routees whose mailbox length can be observed through the local system
/// state participate in the mailbox comparison. Other routees remain valid
/// fallbacks when no observable routee exists.
pub struct SmallestMailboxRoutingLogic;

impl SmallestMailboxRoutingLogic {
  /// Creates a new smallest-mailbox routing logic.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }

  /// Selects the routee with the smallest observable mailbox, if any.
  ///
  /// This keeps the classic kernel decision logic reusable from typed
  /// wrappers that may want their own fallback when no mailbox metrics are
  /// observable.
  #[must_use]
  pub fn select_observed(routees: &[Routee]) -> Option<&Routee> {
    let mut best_observed_index = None;
    let mut best_observed_len = usize::MAX;

    for (index, routee) in routees.iter().enumerate() {
      let Some(mailbox_len) = observed_mailbox_len(routee) else {
        continue;
      };

      if mailbox_len < best_observed_len {
        best_observed_len = mailbox_len;
        best_observed_index = Some(index);
        // Pekko 互換ではない簡略実装: 空メールボックスが複数あっても最小 index 側に偏る。
        // Pekko 互換化の TODO は docs/plan/20260417-smallest-mailbox-pekko-compat-todo.md 参照。
        if mailbox_len == 0 {
          break;
        }
      }
    }

    best_observed_index.map(|index| &routees[index])
  }
}

impl Default for SmallestMailboxRoutingLogic {
  fn default() -> Self {
    Self::new()
  }
}

impl RoutingLogic for SmallestMailboxRoutingLogic {
  fn select<'a>(&self, _message: &AnyMessage, routees: &'a [Routee]) -> &'a Routee {
    if routees.is_empty() {
      static NO_ROUTEE: Routee = Routee::NoRoutee;
      return &NO_ROUTEE;
    }

    Self::select_observed(routees).unwrap_or(&routees[0])
  }
}

// Routee のメールボックス長を取得する。ActorRef 以外は観測不可。
fn observed_mailbox_len(routee: &Routee) -> Option<usize> {
  let Routee::ActorRef(actor_ref) = routee else {
    return None;
  };

  actor_ref.system_state().and_then(|system| system.cell(&actor_ref.pid())).map(|cell| cell.mailbox().user_len())
}
