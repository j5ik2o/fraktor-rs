//! Smallest-mailbox routing logic.

#[cfg(test)]
#[path = "smallest_mailbox_routing_logic_test.rs"]
mod tests;

use super::{routee::Routee, routing_logic::RoutingLogic};
use crate::actor::{actor_ref::ActorRef, messaging::AnyMessage};

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
///
/// # Inherited Pekko behaviour: index-0 bias on equal scores
///
/// Iteration always starts from index 0 (matches Pekko's `selectNext` with
/// `at: Int = 0`, `SmallestMailbox.scala:68`). When multiple routees share the
/// best score — most commonly when all routees are idle with empty mailboxes —
/// the routee at the lowest index always wins, producing a hot-spot on
/// `routees[0]` under low load.
///
/// Pekko exhibits the same property; randomisation (e.g. `ThreadLocalRandom`)
/// is only used in one specific path: selecting a random terminated-fallback
/// when the proposed target ends up terminated (`SmallestMailbox.scala:74`).
/// Introducing a rotating / random start index would distribute load more
/// evenly but diverge from Pekko semantics, so it is intentionally **not**
/// done here. Pekko-parity is the primary goal; any rebalancing optimisation
/// is deferred until the full Pekko contract is satisfied.
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
//
// Pekko の `selectNext` (`SmallestMailbox.scala:64-92`) と同じく、1 パス目の
// best_index / best_score は 2 パス目に引き継がれる。これにより「empty mailbox
// (pass-1 score=1) を優先する」という Pekko documented priority が保たれる。
//
// 例: A=processing+empty (pass-1 score=1, pass-2 score=1)、
//     B=idle+1msg (pass-1 score=SCORE_UNKNOWN, pass-2 score=1) の場合、
//   - pass 1 で best=A (score 1) を確定
//   - pass 2 で A / B ともに deep score = 1。best_score=1 なので `< 1` の比較で どちらも skip され
//     A が保持される
//   これは routee の index 順に依存せず A が選ばれる Pekko 仕様に一致する。
fn select_index_internal(routees: &[Routee]) -> usize {
  debug_assert!(!routees.is_empty(), "select_index_internal requires non-empty routees");

  // 1 パス目: hasMessages の真偽のみで判定し、score=0（idle + 空メールボックス）が
  // 見つかれば即 return する。score>0 の場合でも best_index/best_score を追跡し、
  // 2 パス目に引き継ぐ（Pekko の `proposedTarget` / `currentScore` 引継ぎに相当）。
  let mut best_index = 0_usize;
  let mut best_score = u64::MAX;
  for (index, routee) in routees.iter().enumerate() {
    let score = score_of_shallow(routee);
    if score == 0 {
      return index;
    }
    if score < best_score {
      best_score = score;
      best_index = index;
    }
  }

  // 2 パス目: 実メッセージ件数を取得し、pass 1 の best_score を下回る routee があれば更新する。
  // best_score を u64::MAX にリセットせずに引き継ぐことで、pass 1 で選ばれた
  // 「empty mailbox」ルーティーが pass 2 の「has messages」ルーティーにタイブレーク
  // で負けないようにする。
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

  // `observe_actor_ref` は `has_messages` と `message_count` を同一の `user_len()`
  // スナップショットから導出しているため、`has_messages == true` のときは必ず
  // `message_count >= 1` が成立する。追加のクランプは不要。
  processing_score.saturating_add(observation.message_count as u64)
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
