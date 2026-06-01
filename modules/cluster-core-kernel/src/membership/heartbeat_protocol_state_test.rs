use alloc::vec;
use core::slice;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::membership::{HeartbeatEvidenceKind, HeartbeatProtocolState, HeartbeatRequest, HeartbeatResponse};

fn unique_address(host: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", host, 2552), uid)
}

#[test]
fn heartbeat_tick_generates_sequence_per_peer() {
  let local = unique_address("node-a", 10);
  let peer_a = unique_address("node-b", 11);
  let peer_b = unique_address("node-c", 12);
  let mut state = HeartbeatProtocolState::new(local.clone(), 50, 100);

  let first = state.tick(1000, &[peer_a.clone(), peer_b.clone()]);
  let second = state.tick(1100, slice::from_ref(&peer_a));

  assert_eq!(first.len(), 2);
  assert_eq!(first[0], HeartbeatRequest::new(local.clone(), peer_a.clone(), 1, 1100));
  assert_eq!(first[1], HeartbeatRequest::new(local.clone(), peer_b.clone(), 1, 1100));
  assert_eq!(second, vec![HeartbeatRequest::new(local, peer_a, 2, 1150)]);
}

#[test]
fn heartbeat_request_roundtrip_produces_reachable_evidence() {
  let local = unique_address("node-a", 10);
  let peer = unique_address("node-b", 11);
  let mut state = HeartbeatProtocolState::new(local.clone(), 50, 100);
  let request = state.tick(1000, slice::from_ref(&peer)).remove(0);
  let response = HeartbeatProtocolState::handle_request(request.clone());

  assert_eq!(response, HeartbeatResponse::new(peer.clone(), local.clone(), 1));

  let evidence = state.handle_response(response, 1025).expect("matching response should produce evidence");
  assert_eq!(evidence.kind, HeartbeatEvidenceKind::Reachable { latency_ms: 25 });
  assert_eq!(evidence.observer, local);
  assert_eq!(evidence.subject, peer);
  assert_eq!(evidence.sequence, 1);
}

#[test]
fn stale_heartbeat_response_is_not_success_evidence() {
  let local = unique_address("node-a", 10);
  let peer = unique_address("node-b", 11);
  let mut state = HeartbeatProtocolState::new(local, 50, 100);

  let evidence = state.handle_response(HeartbeatResponse::new(peer, unique_address("node-a", 10), 99), 1000);

  assert_eq!(evidence, None);
}

#[test]
fn expired_heartbeat_response_is_not_success_evidence() {
  let local = unique_address("node-a", 10);
  let peer = unique_address("node-b", 11);
  let mut state = HeartbeatProtocolState::new(local.clone(), 50, 100);
  let request = state.tick(1000, slice::from_ref(&peer)).remove(0);
  let response = HeartbeatProtocolState::handle_request(request);

  let evidence = state.handle_response(response, 1101);
  let timeout = state.collect_timeouts(1101);

  assert_eq!(evidence, None);
  assert_eq!(timeout.len(), 1);
  assert_eq!(timeout[0].kind, HeartbeatEvidenceKind::FirstMissed);
  assert_eq!(timeout[0].observer, local);
  assert_eq!(timeout[0].subject, peer);
}

#[test]
fn overlapping_heartbeat_requests_match_by_peer_and_sequence() {
  let local = unique_address("node-a", 10);
  let peer = unique_address("node-b", 11);
  let mut state = HeartbeatProtocolState::new(local.clone(), 50, 100);
  let first_request = state.tick(1000, slice::from_ref(&peer)).remove(0);
  let second_request = state.tick(1010, slice::from_ref(&peer)).remove(0);

  let first_evidence = state
    .handle_response(HeartbeatProtocolState::handle_request(first_request), 1025)
    .expect("first pending sequence should still match");
  let second_evidence = state
    .handle_response(HeartbeatProtocolState::handle_request(second_request), 1030)
    .expect("second pending sequence should also match");

  assert_eq!(first_evidence.observer, local.clone());
  assert_eq!(first_evidence.subject, peer.clone());
  assert_eq!(first_evidence.sequence, 1);
  assert_eq!(first_evidence.kind, HeartbeatEvidenceKind::Reachable { latency_ms: 25 });
  assert_eq!(second_evidence.observer, local);
  assert_eq!(second_evidence.subject, peer);
  assert_eq!(second_evidence.sequence, 2);
  assert_eq!(second_evidence.kind, HeartbeatEvidenceKind::Reachable { latency_ms: 20 });
}

#[test]
fn first_and_regular_heartbeat_timeouts_are_observable() {
  let local = unique_address("node-a", 10);
  let peer = unique_address("node-b", 11);
  let mut state = HeartbeatProtocolState::new(local.clone(), 50, 100);

  let _ = state.tick(1000, slice::from_ref(&peer));
  let first_timeout = state.collect_timeouts(1101);
  assert_eq!(first_timeout.len(), 1);
  assert_eq!(first_timeout[0].kind, HeartbeatEvidenceKind::FirstMissed);

  let _ = state.tick(1200, slice::from_ref(&peer));
  let regular_timeout = state.collect_timeouts(1251);
  assert_eq!(regular_timeout.len(), 1);
  assert_eq!(regular_timeout[0].kind, HeartbeatEvidenceKind::Missed);
  assert_eq!(regular_timeout[0].observer, local);
  assert_eq!(regular_timeout[0].subject, peer);
}
