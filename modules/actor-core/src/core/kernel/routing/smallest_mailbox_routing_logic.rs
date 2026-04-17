//! Smallest-mailbox routing logic.

#[cfg(test)]
mod tests;

use super::{routee::Routee, routing_logic::RoutingLogic};
use crate::core::kernel::actor::{actor_ref::ActorRef, messaging::AnyMessage};

// Pekko の `SmallestMailbox.scala` に準拠したスコア定数。数値が小さいほど優先度が高い。
// 0 は「idle かつ空メールボックス」を表し、1 パス目で発見次第早期 return の対象となる。
const SCORE_SUSPENDED: u64 = u64::MAX - 1;
const SCORE_UNKNOWN: u64 = u64::MAX - 3;
const SCORE_PROCESSING_PENALTY: u64 = 1;

/// Routes messages to the non-suspended routee with the fewest pending messages.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.SmallestMailboxRoutingLogic`.
/// Score formula (low score wins):
///
/// ```text
/// if suspended:              u64::MAX - 1
/// else:                      processing_penalty + message_component
///   processing_penalty:      1 if is_running else 0
///   message_component:       0                  if !has_messages
///                            count              if deep=true and count > 0
///                            u64::MAX - 3       otherwise (unknown / shallow)
/// ```
///
/// This matches Pekko's `selectNext` verbatim: the processing penalty is also
/// added to the message-count score, so a processing routee with 3 messages
/// scores 4 (1 + 3), while an idle routee with 3 messages scores 3.
///
/// A two-pass search is performed: the first pass only inspects `has_messages`
/// (equivalent to Pekko's `deep=false`) so a score-0 routee short-circuits the
/// lookup. If no score-0 routee exists, the second pass computes exact message
/// counts (`deep=true`) to break ties.
pub struct SmallestMailboxRoutingLogic;

impl SmallestMailboxRoutingLogic {
  /// Creates a new smallest-mailbox routing logic.
  #[must_use]
  pub const fn new() -> Self {
    Self
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

    // 1 パス目: deep=false。hasMessages の真偽のみで判定し、score=0 を見つけたら即 return する。
    let mut best_index = 0_usize;
    let mut best_score = u64::MAX;

    for (index, routee) in routees.iter().enumerate() {
      let score = score_of(routee, false);
      if score == 0 {
        return &routees[index];
      }
      if score < best_score {
        best_score = score;
        best_index = index;
      }
    }

    // 2 パス目: deep=true。実メッセージ件数を取得して最小スコアの routee を決定する。
    best_score = u64::MAX;
    for (index, routee) in routees.iter().enumerate() {
      let score = score_of(routee, true);
      if score < best_score {
        best_score = score;
        best_index = index;
      }
    }

    &routees[best_index]
  }
}

// Routee のスコアを計算する。Pekko の `selectNext` と同じロジック:
// - suspended: SCORE_SUSPENDED
// - !suspended: processing ? 1 : 0 を base として、messages の有無・件数で加算
// - 観測不能 (ActorSelection / NoRoutee / Several / 未登録 ActorRef): SCORE_UNKNOWN
fn score_of(routee: &Routee, deep: bool) -> u64 {
  let Routee::ActorRef(actor_ref) = routee else {
    return SCORE_UNKNOWN;
  };

  let Some(observation) = observe_actor_ref(actor_ref) else {
    return SCORE_UNKNOWN;
  };

  if observation.is_suspended {
    return SCORE_SUSPENDED;
  }

  let processing_score = if observation.is_running { SCORE_PROCESSING_PENALTY } else { 0 };

  if !observation.has_messages {
    return processing_score;
  }

  let message_score = if deep { observation.message_count.max(1) as u64 } else { SCORE_UNKNOWN };

  processing_score.saturating_add(message_score)
}

struct ActorRefObservation {
  is_suspended:  bool,
  is_running:    bool,
  has_messages:  bool,
  message_count: usize,
}

fn observe_actor_ref(actor_ref: &ActorRef) -> Option<ActorRefObservation> {
  let system = actor_ref.system_state()?;
  let cell = system.cell(&actor_ref.pid())?;
  let mailbox = cell.mailbox();
  let message_count = mailbox.user_len();
  Some(ActorRefObservation {
    is_suspended: mailbox.is_suspended(),
    is_running: mailbox.is_running(),
    has_messages: message_count > 0,
    message_count,
  })
}
