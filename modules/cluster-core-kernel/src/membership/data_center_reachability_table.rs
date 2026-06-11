//! Data center level reachability latch state machine.

#[cfg(test)]
#[path = "data_center_reachability_table_test.rs"]
mod tests;

use alloc::{collections::BTreeMap, vec::Vec};

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{
  CrossDcHeartbeatEvidence, CrossDcHeartbeatTargetChange, DataCenter, DataCenterReachabilityTransition,
  HeartbeatEvidenceKind,
};

/// Per-DC state: the set of observed targets with per-target reachability,
/// plus a latch flag for the DC-level unreachable state.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DcState {
  /// Observed subjects with their per-target reachability.
  /// `true` = reachable (or not yet observed), `false` = unreachable.
  targets:             BTreeMap<UniqueAddress, bool>,
  /// `true` when the DC is latched as unreachable (all targets unreachable).
  latched_unreachable: bool,
}

impl DcState {
  const fn new() -> Self {
    Self { targets: BTreeMap::new(), latched_unreachable: false }
  }

  /// Returns `true` when the target set is non-empty and every target is unreachable.
  fn all_unreachable(&self) -> bool {
    !self.targets.is_empty() && self.targets.values().all(|reachable| !reachable)
  }
}

/// Pure state machine that tracks data center level reachability based on
/// cross-DC heartbeat evidence.
///
/// Invariants:
/// - The entry for `self_data_center` is never present.
/// - A [`DataCenterReachabilityTransition::BecameUnreachable`] transition is emitted exactly once
///   when all observed targets for a DC become unreachable (latch), either via evidence or via a
///   target removal that leaves only unreachable targets.
/// - A [`DataCenterReachabilityTransition::BecameReachable`] transition is emitted once when at
///   least one target becomes reachable again after the latch.
/// - DCs whose target set becomes empty are removed without emitting a transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataCenterReachabilityTable {
  self_data_center: DataCenter,
  dc_states:        BTreeMap<DataCenter, DcState>,
}

impl DataCenterReachabilityTable {
  /// Creates a new table for a node in `self_data_center`.
  #[must_use]
  pub const fn new(self_data_center: DataCenter) -> Self {
    Self { self_data_center, dc_states: BTreeMap::new() }
  }

  /// Synchronizes the observed target set from a cross-DC heartbeat target
  /// change and returns latch transitions caused by removals.
  ///
  /// Added targets are inserted with reachable status as default.
  /// Removed targets are deleted.  DCs whose target set becomes empty are
  /// removed without emitting a transition.  When a removal leaves a DC whose
  /// remaining targets are all unreachable, the DC is latched and a
  /// [`DataCenterReachabilityTransition::BecameUnreachable`] transition is
  /// returned.  Added targets default to reachable as "not yet observed", so
  /// additions never release an existing latch without actual evidence.
  #[must_use = "latch transitions must be published or explicitly ignored"]
  pub fn apply_target_change(
    &mut self,
    change: &CrossDcHeartbeatTargetChange,
  ) -> Vec<DataCenterReachabilityTransition> {
    for target in &change.added {
      // 自 DC への変更は入力段階で無視する（不変条件: 自 DC エントリなし）
      if target.remote_data_center == self.self_data_center {
        continue;
      }
      let state = self.dc_states.entry(target.remote_data_center.clone()).or_insert_with(DcState::new);
      // 既存エントリがない場合のみ初期状態（reachable）で挿入する
      state.targets.entry(target.peer.clone()).or_insert(true);
    }

    for target in &change.removed {
      if target.remote_data_center == self.self_data_center {
        continue;
      }
      if let Some(state) = self.dc_states.get_mut(&target.remote_data_center) {
        state.targets.remove(&target.peer);
      }
    }

    // 観測対象がゼロになった DC はエントリ削除（遷移出力なし）
    let empty_dcs: Vec<DataCenter> =
      self.dc_states.iter().filter(|(_, state)| state.targets.is_empty()).map(|(dc, _)| dc.clone()).collect();
    for dc in &empty_dcs {
      self.dc_states.remove(dc);
    }

    // 削除の結果「残る観測対象がすべて unreachable」になった DC はラッチを再評価する。
    // ここで latch を立てないと、次の evidence が来るまで DC が reachable 扱いのままになる
    let mut transitions = Vec::new();
    for (dc, state) in &mut self.dc_states {
      if !state.latched_unreachable && state.all_unreachable() {
        state.latched_unreachable = true;
        transitions.push(DataCenterReachabilityTransition::BecameUnreachable { data_center: dc.clone() });
      }
    }
    transitions
  }

  /// Feeds availability evidence and returns a latched transition when the
  /// data center level reachability changes.
  ///
  /// Returns `Some(BecameUnreachable)` when all observed targets for the DC
  /// become unreachable for the first time (latch).  Returns
  /// `Some(BecameReachable)` on the first reachable evidence after the latch.
  /// Returns `None` for self-DC evidence, unknown DCs, unknown targets, or
  /// evidence that does not change the latched state.
  pub fn observe(&mut self, evidence: &CrossDcHeartbeatEvidence) -> Option<DataCenterReachabilityTransition> {
    // 自 DC の evidence は入力段階で無視する
    if evidence.remote_data_center == self.self_data_center {
      return None;
    }

    let dc = evidence.remote_data_center.clone();
    let state = self.dc_states.get_mut(&dc)?;

    // ターゲットとして登録されていない subject は無視する
    if !state.targets.contains_key(&evidence.subject) {
      return None;
    }

    let is_reachable_evidence = is_reachable(&evidence.kind);

    // subject の到達性を更新する
    state.targets.insert(evidence.subject.clone(), is_reachable_evidence);

    if state.latched_unreachable {
      // ラッチ中: reachable evidence が来たら復帰遷移を出力する
      if is_reachable_evidence {
        state.latched_unreachable = false;
        return Some(DataCenterReachabilityTransition::BecameReachable { data_center: dc });
      }
      // ラッチ中の同状態 evidence は None（ラッチ済み）
      None
    } else {
      // 非ラッチ: 全観測対象が unreachable になったら unreachable ラッチへ遷移する
      if state.all_unreachable() {
        state.latched_unreachable = true;
        return Some(DataCenterReachabilityTransition::BecameUnreachable { data_center: dc });
      }
      None
    }
  }

  /// Returns a read-only view of currently unreachable data centers.
  #[must_use]
  pub fn unreachable_data_centers(&self) -> Vec<DataCenter> {
    self.dc_states.iter().filter(|(_, state)| state.latched_unreachable).map(|(dc, _)| dc.clone()).collect()
  }
}

/// Returns `true` when the evidence kind indicates reachability.
const fn is_reachable(kind: &HeartbeatEvidenceKind) -> bool {
  matches!(kind, HeartbeatEvidenceKind::Reachable { .. })
}
