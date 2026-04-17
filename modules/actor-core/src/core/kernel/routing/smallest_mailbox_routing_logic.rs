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
    let index = select_index_internal(routees);
    &routees[index]
  }

  fn select_index(&self, routees: &[Routee]) -> usize {
    assert!(!routees.is_empty(), "SmallestMailboxRoutingLogic::select_index requires non-empty routees");
    select_index_internal(routees)
  }
}

// 2 パス探索で最小スコア routee の index を返す。呼び出し元が非空を保証する前提。
fn select_index_internal(routees: &[Routee]) -> usize {
  debug_assert!(!routees.is_empty(), "select_index_internal requires non-empty routees");

  // 1 パス目: hasMessages の真偽のみで判定し、score=0（idle + 空メールボックス）が
  // 見つかれば即 return する。Pekko の selectNext と同じく score>0 のルーティーは
  // 2 パス目で改めて最小スコアを探索するため、ここでは best_index/best_score の
  // 更新は行わない。
  for (index, routee) in routees.iter().enumerate() {
    if score_of_shallow(routee) == 0 {
      return index;
    }
  }

  // 2 パス目: 実メッセージ件数を取得して最小スコアの routee を決定する。
  let mut best_index = 0_usize;
  let mut best_score = u64::MAX;
  for (index, routee) in routees.iter().enumerate() {
    let score = score_of_deep(routee);
    if score < best_score {
      best_score = score;
      best_index = index;
    }
  }

  best_index
}

// 1 パス目（Pekko の deep=false）用スコア計算。
// メッセージありの場合、件数は取得せず SCORE_UNKNOWN（件数不明）扱いにして
// 処理中ペナルティと合算する。これにより score=0 / 1 だけが確定し、件数比較による
// 早期 return は発生しない。
fn score_of_shallow(routee: &Routee) -> u64 {
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

  // Pekko: hasMessages && !deep の場合は noOfMsgs=0 扱いになり、
  //        結果として Long.MaxValue - 3 が加算される。
  processing_score.saturating_add(SCORE_UNKNOWN)
}

// 2 パス目（Pekko の deep=true）用スコア計算。
// 実 mailbox 件数を取得し、処理中ペナルティと合算する。
fn score_of_deep(routee: &Routee) -> u64 {
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

  let message_score = observation.message_count.max(1) as u64;
  processing_score.saturating_add(message_score)
}

struct ActorRefObservation {
  is_suspended:  bool,
  is_running:    bool,
  has_messages:  bool,
  message_count: usize,
}

// `ActorRefObservation` のフィールド (`is_suspended` / `is_running` / `has_messages` /
// `message_count`) は **個別のアトミック観測値** であり、それぞれが異なる瞬間の
// mailbox / schedule state スナップショットから採取されるため、厳密には TOCTOU
// レースが存在する。
//
// 例えば `user_len()` を読んだ直後に別スレッドが enqueue を行うと、`has_messages`
// と `message_count` が瞬間的にずれる可能性がある。同様に `is_running` と
// `is_suspended` も並行する `set_running` / `suspend` 呼び出しで変化する。
//
// これを許容しているのは、ルーティング判定がベストエフォート・ヒューリスティクス
// であるため。全フィールドを単一のアトミックスナップショットに強制統合することは
// possible だが、それによって routing 判定が数 μs だけ正確になる価値よりも、ロック
// 競合の増加・実装複雑化のコストが上回ると判断した。Pekko の `selectNext` も同様の
// race を持っており、そのコメント（`Race between hasMessages and numberOfMessages here`）
// が互換挙動を示唆している。
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
