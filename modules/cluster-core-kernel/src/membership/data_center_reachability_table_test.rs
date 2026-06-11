use alloc::{vec, vec::Vec};

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use super::{DataCenterReachabilityTable, DataCenterReachabilityTransition};
use crate::membership::{
  CrossDcHeartbeatEvidence, CrossDcHeartbeatTarget, CrossDcHeartbeatTargetChange, DataCenter, HeartbeatEvidenceKind,
};

fn unique_address(host: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", host, 2552), uid)
}

fn make_change(
  added: Vec<CrossDcHeartbeatTarget>,
  removed: Vec<CrossDcHeartbeatTarget>,
  retained: Vec<CrossDcHeartbeatTarget>,
) -> CrossDcHeartbeatTargetChange {
  CrossDcHeartbeatTargetChange::new(added, removed, retained)
}

fn target(peer_host: &str, peer_uid: u64, local_dc: DataCenter, remote_dc: DataCenter) -> CrossDcHeartbeatTarget {
  CrossDcHeartbeatTarget::new(unique_address(peer_host, peer_uid), local_dc, remote_dc)
}

fn unreachable_evidence(
  observer_host: &str,
  observer_uid: u64,
  subject_host: &str,
  subject_uid: u64,
  local_dc: DataCenter,
  remote_dc: DataCenter,
) -> CrossDcHeartbeatEvidence {
  CrossDcHeartbeatEvidence::new(
    unique_address(observer_host, observer_uid),
    unique_address(subject_host, subject_uid),
    local_dc,
    remote_dc,
    1,
    HeartbeatEvidenceKind::FirstMissed,
  )
}

fn reachable_evidence(
  observer_host: &str,
  observer_uid: u64,
  subject_host: &str,
  subject_uid: u64,
  local_dc: DataCenter,
  remote_dc: DataCenter,
) -> CrossDcHeartbeatEvidence {
  CrossDcHeartbeatEvidence::new(
    unique_address(observer_host, observer_uid),
    unique_address(subject_host, subject_uid),
    local_dc,
    remote_dc,
    2,
    HeartbeatEvidenceKind::Reachable { latency_ms: 10 },
  )
}

// 要件 3.1: 全観測対象 unreachable で BecameUnreachable が 1 回だけ出力される（ラッチ）
#[test]
fn all_targets_unreachable_emits_became_unreachable_once() {
  let local_dc = DataCenter::new("dc-a");
  let remote_dc = DataCenter::new("dc-b");

  let mut table = DataCenterReachabilityTable::new(local_dc.clone());

  // ターゲット追加: node-c (dc-b) を観測対象に
  let change = make_change(vec![target("node-c", 12, local_dc.clone(), remote_dc.clone())], vec![], vec![]);
  table.apply_target_change(&change);

  // 1 件の観測対象が unreachable → BecameUnreachable が返る
  let ev1 = table.observe(&unreachable_evidence("node-a", 10, "node-c", 12, local_dc.clone(), remote_dc.clone()));
  assert!(
    matches!(ev1, Some(DataCenterReachabilityTransition::BecameUnreachable { ref data_center }) if data_center == &remote_dc),
    "expected BecameUnreachable for dc-b, got {:?}",
    ev1
  );

  // 同じ状態の evidence を再投入してもラッチされているので None
  let ev2 = table.observe(&unreachable_evidence("node-a", 10, "node-c", 12, local_dc.clone(), remote_dc.clone()));
  assert!(ev2.is_none(), "expected None (latch), got {:?}", ev2);
}

// 要件 3.2: ラッチ後の reachable evidence で BecameReachable が 1 回出力される
#[test]
fn reachable_evidence_after_latch_emits_became_reachable_once() {
  let local_dc = DataCenter::new("dc-a");
  let remote_dc = DataCenter::new("dc-b");

  let mut table = DataCenterReachabilityTable::new(local_dc.clone());

  let change = make_change(vec![target("node-c", 12, local_dc.clone(), remote_dc.clone())], vec![], vec![]);
  table.apply_target_change(&change);

  // ラッチ状態に遷移
  let ev1 = table.observe(&unreachable_evidence("node-a", 10, "node-c", 12, local_dc.clone(), remote_dc.clone()));
  assert!(matches!(ev1, Some(DataCenterReachabilityTransition::BecameUnreachable { .. })));

  // reachable evidence → BecameReachable
  let ev2 = table.observe(&reachable_evidence("node-a", 10, "node-c", 12, local_dc.clone(), remote_dc.clone()));
  assert!(
    matches!(ev2, Some(DataCenterReachabilityTransition::BecameReachable { ref data_center }) if data_center == &remote_dc),
    "expected BecameReachable for dc-b, got {:?}",
    ev2
  );

  // 再び reachable evidence を投入してもラッチ解除済みなので None
  let ev3 = table.observe(&reachable_evidence("node-a", 10, "node-c", 12, local_dc.clone(), remote_dc.clone()));
  assert!(ev3.is_none(), "expected None (already reachable), got {:?}", ev3);
}

// 要件 3.4: 自 DC の evidence は無視する
#[test]
fn evidence_from_self_data_center_is_ignored() {
  let self_dc = DataCenter::new("dc-a");

  let mut table = DataCenterReachabilityTable::new(self_dc.clone());

  // 自 DC 宛のターゲット変更 (通常は発生しないが、万一来ても無視)
  let change = make_change(vec![target("node-b", 11, self_dc.clone(), self_dc.clone())], vec![], vec![]);
  table.apply_target_change(&change);

  // remote_data_center が self_dc と同じ evidence → 無視
  let ev = table.observe(&unreachable_evidence("node-a", 10, "node-b", 11, self_dc.clone(), self_dc.clone()));
  assert!(ev.is_none(), "evidence targeting self DC must be ignored, got {:?}", ev);

  // 自 DC が unreachable_data_centers に含まれない
  assert!(table.unreachable_data_centers().is_empty(), "self DC must never appear in unreachable_data_centers");
}

// 観測対象がゼロになった DC はエントリ削除（遷移出力なし）
#[test]
fn removing_all_targets_of_dc_deletes_entry_without_transition() {
  let local_dc = DataCenter::new("dc-a");
  let remote_dc = DataCenter::new("dc-b");

  let mut table = DataCenterReachabilityTable::new(local_dc.clone());

  let t = target("node-c", 12, local_dc.clone(), remote_dc.clone());
  let add_change = make_change(vec![t.clone()], vec![], vec![]);
  table.apply_target_change(&add_change);

  // unreachable 状態にしてからターゲットを除去
  let _ = table.observe(&unreachable_evidence("node-a", 10, "node-c", 12, local_dc.clone(), remote_dc.clone()));

  let remove_change = make_change(vec![], vec![t], vec![]);
  // ターゲット除去時に遷移出力しないことを confirm する手段がないため、
  // apply_target_change 後に observe が None を返すことと
  // unreachable_data_centers が空であることで検証する
  table.apply_target_change(&remove_change);

  // エントリ削除後は unreachable ではなくなっている
  assert!(
    table.unreachable_data_centers().is_empty(),
    "DC entry must be removed when all targets are removed, unreachable list should be empty"
  );

  // ターゲットが無い状態で evidence を投入しても無視される
  let ev = table.observe(&unreachable_evidence("node-a", 10, "node-c", 12, local_dc.clone(), remote_dc.clone()));
  assert!(ev.is_none(), "evidence for removed DC must yield None, got {:?}", ev);
}

// 複数観測対象: 一部が unreachable でも全滅しなければ遷移しない
#[test]
fn partial_unreachable_does_not_trigger_transition() {
  let local_dc = DataCenter::new("dc-a");
  let remote_dc = DataCenter::new("dc-b");

  let mut table = DataCenterReachabilityTable::new(local_dc.clone());

  let change = make_change(
    vec![
      target("node-c", 12, local_dc.clone(), remote_dc.clone()),
      target("node-d", 13, local_dc.clone(), remote_dc.clone()),
    ],
    vec![],
    vec![],
  );
  table.apply_target_change(&change);

  // node-c のみ unreachable → まだ全観測対象が落ちていないので None
  let ev = table.observe(&unreachable_evidence("node-a", 10, "node-c", 12, local_dc.clone(), remote_dc.clone()));
  assert!(ev.is_none(), "partial unreachable must not trigger BecameUnreachable, got {:?}", ev);
}
