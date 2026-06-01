use alloc::{string::ToString, vec, vec::Vec};

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::membership::{
  CrossDcHeartbeat, CrossDcHeartbeatEvidence, CrossDcHeartbeatRequest, CrossDcHeartbeatResponse,
  CrossDcHeartbeatTarget, CrossDcHeartbeatTargetChange, DataCenter, GossipPayloadKind, HeartbeatEvidenceKind,
  HeartbeatRequest, HeartbeatResponse, MembershipSnapshot, MembershipVersion, NodeRecord, NodeStatus,
};

fn unique_address(host: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", host, 2552), uid)
}

fn record(host: &str, uid: u64, data_center: DataCenter, status: NodeStatus) -> NodeRecord {
  NodeRecord::new_with_identity(
    unique_address(host, uid),
    data_center,
    host.to_string(),
    status,
    MembershipVersion::new(uid),
    "1.0.0".to_string(),
    Vec::new(),
  )
}

#[test]
fn update_targets_selects_only_remote_active_data_centers() {
  let local = unique_address("node-a", 10);
  let dc_a = DataCenter::new("dc-a");
  let dc_b = DataCenter::new("dc-b");
  let snapshot = MembershipSnapshot::new(MembershipVersion::new(1), vec![
    record("node-a", 10, dc_a.clone(), NodeStatus::Up),
    record("node-b", 11, dc_a.clone(), NodeStatus::Up),
    record("node-c", 12, dc_b.clone(), NodeStatus::Up),
    record("node-d", 13, dc_b.clone(), NodeStatus::Dead),
  ]);
  let mut heartbeat = CrossDcHeartbeat::new(local.clone(), dc_a.clone(), 50, 100);

  let change = heartbeat.update_targets(&snapshot);

  assert_eq!(change, CrossDcHeartbeatTargetChange {
    added:    vec![CrossDcHeartbeatTarget::new(unique_address("node-c", 12), dc_a, dc_b)],
    removed:  Vec::new(),
    retained: Vec::new(),
  });
  assert_eq!(heartbeat.targets(), change.added);
}

#[test]
fn cross_dc_tick_wraps_request_with_data_center_pair_and_payload_kind() {
  let local = unique_address("node-a", 10);
  let peer = unique_address("node-c", 12);
  let dc_a = DataCenter::new("dc-a");
  let dc_b = DataCenter::new("dc-b");
  let snapshot = MembershipSnapshot::new(MembershipVersion::new(1), vec![
    record("node-a", 10, dc_a.clone(), NodeStatus::Up),
    record("node-c", 12, dc_b.clone(), NodeStatus::Up),
  ]);
  let mut heartbeat = CrossDcHeartbeat::new(local.clone(), dc_a.clone(), 50, 100);
  let _ = heartbeat.update_targets(&snapshot);

  let requests = heartbeat.tick(1000);

  assert_eq!(requests, vec![CrossDcHeartbeatRequest::new(HeartbeatRequest::new(local, peer, 1, 1100), dc_a, dc_b,)]);
  assert_eq!(requests[0].payload_kind(), GossipPayloadKind::CrossDcHeartbeat);
}

#[test]
fn cross_dc_response_roundtrip_produces_data_center_evidence() {
  let local = unique_address("node-a", 10);
  let peer = unique_address("node-c", 12);
  let dc_a = DataCenter::new("dc-a");
  let dc_b = DataCenter::new("dc-b");
  let snapshot = MembershipSnapshot::new(MembershipVersion::new(1), vec![
    record("node-a", 10, dc_a.clone(), NodeStatus::Up),
    record("node-c", 12, dc_b.clone(), NodeStatus::Up),
  ]);
  let mut heartbeat = CrossDcHeartbeat::new(local.clone(), dc_a.clone(), 50, 100);
  let _ = heartbeat.update_targets(&snapshot);
  let request = heartbeat.tick(1000).remove(0);

  let response = CrossDcHeartbeat::handle_request(request.clone());
  assert_eq!(
    response,
    CrossDcHeartbeatResponse::new(HeartbeatResponse::new(peer.clone(), local.clone(), 1), dc_b.clone(), dc_a.clone(),)
  );

  let evidence = heartbeat.handle_response(response, 1025).expect("cross-DC response should produce evidence");
  assert_eq!(
    evidence,
    CrossDcHeartbeatEvidence::new(local, peer, dc_a, dc_b, 1, HeartbeatEvidenceKind::Reachable { latency_ms: 25 },)
  );
  assert_eq!(evidence.payload_kind(), GossipPayloadKind::CrossDcHeartbeat);
}

#[test]
fn cross_dc_response_is_ignored_after_target_removal() {
  let local = unique_address("node-a", 10);
  let dc_a = DataCenter::new("dc-a");
  let dc_b = DataCenter::new("dc-b");
  let first = MembershipSnapshot::new(MembershipVersion::new(1), vec![
    record("node-a", 10, dc_a.clone(), NodeStatus::Up),
    record("node-c", 12, dc_b.clone(), NodeStatus::Up),
  ]);
  let second =
    MembershipSnapshot::new(MembershipVersion::new(2), vec![record("node-a", 10, dc_a.clone(), NodeStatus::Up)]);
  let mut heartbeat = CrossDcHeartbeat::new(local, dc_a, 50, 100);
  let _ = heartbeat.update_targets(&first);
  let request = heartbeat.tick(1000).remove(0);
  let response = CrossDcHeartbeat::handle_request(request);
  let change = heartbeat.update_targets(&second);

  assert_eq!(change.removed.len(), 1);
  assert_eq!(heartbeat.handle_response(response, 1025), None);
}

#[test]
fn cross_dc_response_with_mismatched_data_center_pair_is_ignored() {
  let local = unique_address("node-a", 10);
  let peer = unique_address("node-c", 12);
  let dc_a = DataCenter::new("dc-a");
  let dc_b = DataCenter::new("dc-b");
  let dc_c = DataCenter::new("dc-c");
  let snapshot = MembershipSnapshot::new(MembershipVersion::new(1), vec![
    record("node-a", 10, dc_a.clone(), NodeStatus::Up),
    record("node-c", 12, dc_b.clone(), NodeStatus::Up),
  ]);
  let mut heartbeat = CrossDcHeartbeat::new(local.clone(), dc_a.clone(), 50, 100);
  let _ = heartbeat.update_targets(&snapshot);
  let _ = heartbeat.tick(1000).remove(0);

  let evidence =
    heartbeat.handle_response(CrossDcHeartbeatResponse::new(HeartbeatResponse::new(peer, local, 1), dc_c, dc_a), 1025);

  assert_eq!(evidence, None);
}

#[test]
fn target_update_reports_added_removed_and_retained() {
  let local = unique_address("node-a", 10);
  let dc_a = DataCenter::new("dc-a");
  let dc_b = DataCenter::new("dc-b");
  let dc_c = DataCenter::new("dc-c");
  let first = MembershipSnapshot::new(MembershipVersion::new(1), vec![
    record("node-a", 10, dc_a.clone(), NodeStatus::Up),
    record("node-b", 11, dc_b.clone(), NodeStatus::Up),
    record("node-c", 12, dc_c.clone(), NodeStatus::Up),
  ]);
  let second = MembershipSnapshot::new(MembershipVersion::new(2), vec![
    record("node-a", 10, dc_a.clone(), NodeStatus::Up),
    record("node-b", 11, dc_b.clone(), NodeStatus::Up),
    record("node-d", 13, dc_c.clone(), NodeStatus::Up),
  ]);
  let mut heartbeat = CrossDcHeartbeat::new(local, dc_a.clone(), 50, 100);
  let _ = heartbeat.update_targets(&first);

  let change = heartbeat.update_targets(&second);

  assert_eq!(change, CrossDcHeartbeatTargetChange {
    added:    vec![CrossDcHeartbeatTarget::new(unique_address("node-d", 13), dc_a.clone(), dc_c.clone())],
    removed:  vec![CrossDcHeartbeatTarget::new(unique_address("node-c", 12), dc_a.clone(), dc_c)],
    retained: vec![CrossDcHeartbeatTarget::new(unique_address("node-b", 11), dc_a, dc_b)],
  });
}

#[test]
fn cross_dc_timeout_evidence_keeps_data_center_pair_without_downing_decision() {
  let local = unique_address("node-a", 10);
  let peer = unique_address("node-c", 12);
  let dc_a = DataCenter::new("dc-a");
  let dc_b = DataCenter::new("dc-b");
  let snapshot = MembershipSnapshot::new(MembershipVersion::new(1), vec![
    record("node-a", 10, dc_a.clone(), NodeStatus::Up),
    record("node-c", 12, dc_b.clone(), NodeStatus::Up),
  ]);
  let mut heartbeat = CrossDcHeartbeat::new(local.clone(), dc_a.clone(), 50, 100);
  let _ = heartbeat.update_targets(&snapshot);
  let _ = heartbeat.tick(1000);

  let evidence = heartbeat.collect_timeouts(1101);

  assert_eq!(evidence, vec![CrossDcHeartbeatEvidence::new(
    local,
    peer,
    dc_a,
    dc_b,
    1,
    HeartbeatEvidenceKind::FirstMissed,
  )]);
}
