use alloc::vec;
use core::slice;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::membership::{
  GossipEnvelope, GossipPayloadKind, GossipTransportHandoff, GossipTransportHandoffError, MembershipVersion,
};

fn unique_address(host: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", host, 2552), uid)
}

#[test]
fn handoff_keeps_envelope_identity_kind_and_endpoint_mapping() {
  let local = unique_address("node-a", 10);
  let peer = unique_address("node-b", 11);
  let envelope = GossipEnvelope::try_new(
    local.clone(),
    peer.clone(),
    GossipPayloadKind::HeartbeatRequest,
    MembershipVersion::new(7),
    100,
  )
  .expect("envelope");

  let handoff = GossipTransportHandoff::try_new(envelope.clone(), slice::from_ref(&peer), 100).expect("handoff");

  assert_eq!(handoff.envelope(), &envelope);
  assert_eq!(handoff.from(), &local);
  assert_eq!(handoff.to(), &peer);
  assert_eq!(handoff.payload_kind(), GossipPayloadKind::HeartbeatRequest);
  assert_eq!(handoff.target_endpoint(), "node-b:2552");
}

#[test]
fn handoff_brackets_ipv6_target_endpoint() {
  let local = unique_address("node-a", 10);
  let peer = unique_address("::1", 11);
  let envelope =
    GossipEnvelope::try_new(local, peer.clone(), GossipPayloadKind::HeartbeatRequest, MembershipVersion::new(7), 100)
      .expect("envelope");

  let handoff = GossipTransportHandoff::try_new(envelope, slice::from_ref(&peer), 100).expect("handoff");

  assert_eq!(handoff.target_endpoint(), "[::1]:2552");
}

#[test]
fn handoff_rejects_unknown_peer_without_losing_identity() {
  let local = unique_address("node-a", 10);
  let peer = unique_address("node-b", 11);
  let envelope =
    GossipEnvelope::try_new(local, peer.clone(), GossipPayloadKind::FullState, MembershipVersion::new(7), 100)
      .expect("envelope");

  let err = GossipTransportHandoff::try_new(envelope, &[], 100).expect_err("unknown peer should fail");

  assert_eq!(err, GossipTransportHandoffError::UnknownPeer { peer });
}

#[test]
fn handoff_rejects_expired_envelope() {
  let local = unique_address("node-a", 10);
  let peer = unique_address("node-b", 11);
  let envelope = GossipEnvelope::try_new(local, peer.clone(), GossipPayloadKind::Delta, MembershipVersion::new(7), 100)
    .expect("envelope");

  let err = GossipTransportHandoff::try_new(envelope, &[peer], 101).expect_err("expired envelope should fail");

  assert_eq!(err, GossipTransportHandoffError::DeadlineExpired { deadline_tick: 100, now_tick: 101 });
}

#[test]
fn handoff_observes_unknown_payload_kind_tag() {
  let err = GossipTransportHandoff::payload_kind_from_tag(99).expect_err("unknown tag should fail");

  assert_eq!(err, GossipTransportHandoffError::UnknownPayloadKind { tag: 99 });
}

#[test]
fn handoff_distinguishes_heartbeat_and_gossip_payload_tags() {
  let kinds = vec![
    GossipTransportHandoff::payload_kind_from_tag(0).expect("delta"),
    GossipTransportHandoff::payload_kind_from_tag(3).expect("heartbeat request"),
    GossipTransportHandoff::payload_kind_from_tag(4).expect("heartbeat response"),
    GossipTransportHandoff::payload_kind_from_tag(5).expect("cross dc heartbeat"),
    GossipTransportHandoff::payload_kind_from_tag(6).expect("pubsub status"),
    GossipTransportHandoff::payload_kind_from_tag(7).expect("pubsub delta"),
  ];

  assert_eq!(kinds, vec![
    GossipPayloadKind::Delta,
    GossipPayloadKind::HeartbeatRequest,
    GossipPayloadKind::HeartbeatResponse,
    GossipPayloadKind::CrossDcHeartbeat,
    GossipPayloadKind::PubSubRegistryStatus,
    GossipPayloadKind::PubSubRegistryDelta,
  ]);
}
