use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::membership::{
  GossipEnvelope, GossipEnvelopeDispatchOutcome, GossipEnvelopeError, GossipPayloadKind, MembershipVersion,
};

fn unique_address(host: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", host, 2552), uid)
}

#[test]
fn gossip_envelope_keeps_identity_kind_version_and_deadline() {
  let from = unique_address("node-a", 10);
  let to = unique_address("node-b", 11);

  let envelope =
    GossipEnvelope::try_new(from.clone(), to.clone(), GossipPayloadKind::Delta, MembershipVersion::new(7), 100)
      .expect("confirmed identities should build an envelope");

  assert_eq!(envelope.from(), &from);
  assert_eq!(envelope.to(), &to);
  assert_eq!(envelope.payload_kind(), GossipPayloadKind::Delta);
  assert_eq!(envelope.membership_version(), MembershipVersion::new(7));
  assert_eq!(envelope.deadline_tick(), 100);
  assert_eq!(envelope.dispatch_outcome(100), GossipEnvelopeDispatchOutcome::Ready);
}

#[test]
fn gossip_payload_kind_distinguishes_protocol_payloads() {
  let kinds = [
    GossipPayloadKind::Delta,
    GossipPayloadKind::FullState,
    GossipPayloadKind::SeenDigest,
    GossipPayloadKind::HeartbeatRequest,
    GossipPayloadKind::HeartbeatResponse,
    GossipPayloadKind::CrossDcHeartbeat,
  ];

  assert_eq!(kinds[0], GossipPayloadKind::Delta);
  assert_eq!(kinds[1], GossipPayloadKind::FullState);
  assert_eq!(kinds[2], GossipPayloadKind::SeenDigest);
  assert_eq!(kinds[3], GossipPayloadKind::HeartbeatRequest);
  assert_eq!(kinds[4], GossipPayloadKind::HeartbeatResponse);
  assert_eq!(kinds[5], GossipPayloadKind::CrossDcHeartbeat);
}

#[test]
fn gossip_envelope_rejects_unconfirmed_identity() {
  let from = unique_address("node-a", 0);
  let to = unique_address("node-b", 11);

  let err = GossipEnvelope::try_new(from, to, GossipPayloadKind::FullState, MembershipVersion::new(7), 100)
    .expect_err("unconfirmed from identity should be observable");

  assert_eq!(err, GossipEnvelopeError::UnconfirmedIdentity { from: true, to: false });
}

#[test]
fn gossip_envelope_reports_deadline_expired_outcome() {
  let envelope = GossipEnvelope::try_new(
    unique_address("node-a", 10),
    unique_address("node-b", 11),
    GossipPayloadKind::HeartbeatRequest,
    MembershipVersion::new(7),
    100,
  )
  .expect("confirmed identities should build an envelope");

  assert_eq!(envelope.dispatch_outcome(101), GossipEnvelopeDispatchOutcome::DeadlineExpired {
    deadline_tick: 100,
    now_tick:      101,
  });
}
