use core::time::Duration;

use fraktor_cluster_core_kernel_rs::membership::{
  GossipEnvelope, GossipOutbound, GossipPayloadKind, GossipTransport, GossipTransportError,
  GossipTransportHandoffError, MembershipDelta, MembershipVersion, NodeRecord, NodeStatus,
};
use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use tokio::{net::UdpSocket, runtime::Handle};

use crate::membership::{TokioGossipTransport, TokioGossipTransportConfig};

fn sample_delta() -> MembershipDelta {
  let record = NodeRecord::new(
    String::from("node-a"),
    String::from("127.0.0.1:11000"),
    NodeStatus::Up,
    MembershipVersion::new(1),
    String::from("1.0.0"),
    vec![String::from("member")],
  );
  MembershipDelta::new(MembershipVersion::new(0), MembershipVersion::new(1), vec![record])
}

fn unique_address(host: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", host, 2552), uid)
}

#[tokio::test]
async fn send_rejects_invalid_authority() {
  let config = TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8);
  let mut transport = TokioGossipTransport::bind(config, Handle::current()).expect("transport bind");
  let outbound = GossipOutbound::new(String::from("invalid-authority"), sample_delta());
  let result = transport.send(outbound);
  assert!(result.is_err());
}

#[tokio::test]
async fn poll_returns_empty_when_no_messages() {
  let config = TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8);
  let mut transport = TokioGossipTransport::bind(config, Handle::current()).expect("transport bind");
  let deltas = transport.poll_deltas();
  assert!(deltas.is_empty());
}

#[tokio::test]
async fn recv_returns_delta_from_allowed_udp_peer() {
  let sender = UdpSocket::bind("127.0.0.1:0").await.expect("sender bind");
  let sender_addr = sender.local_addr().expect("sender local addr");
  let config = TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8)
    .with_allowed_peers(vec![sender_addr.to_string()]);
  let mut transport = TokioGossipTransport::bind(config, Handle::current()).expect("transport bind");

  let payload = transport.encode_delta(&sample_delta()).expect("encode");
  sender.send_to(&payload, transport.local_addr()).await.expect("send");

  tokio::time::sleep(Duration::from_millis(50)).await;
  let deltas = transport.poll_deltas();
  assert_eq!(deltas.len(), 1);
  assert_eq!(deltas[0].1, sample_delta());
}

#[tokio::test]
async fn recv_drops_delta_from_untrusted_udp_peer() {
  let trusted = UdpSocket::bind("127.0.0.1:0").await.expect("trusted bind");
  let sender = UdpSocket::bind("127.0.0.1:0").await.expect("sender bind");
  let config = TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8)
    .with_allowed_peers(vec![trusted.local_addr().expect("trusted local addr").to_string()]);
  let mut transport = TokioGossipTransport::bind(config, Handle::current()).expect("transport bind");

  let payload = transport.encode_delta(&sample_delta()).expect("encode");
  sender.send_to(&payload, transport.local_addr()).await.expect("send");

  tokio::time::sleep(Duration::from_millis(50)).await;
  let deltas = transport.poll_deltas();
  assert!(deltas.is_empty());
}

#[tokio::test]
async fn handoff_envelope_keeps_target_identity_endpoint_and_payload_kind() {
  let peer = unique_address("127.0.0.1", 11);
  let config = TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8)
    .with_allowed_peer_identities(vec![peer.clone()]);
  let transport = TokioGossipTransport::bind(config, Handle::current()).expect("transport bind");
  let envelope = GossipEnvelope::try_new(
    unique_address("127.0.0.2", 10),
    peer.clone(),
    GossipPayloadKind::HeartbeatResponse,
    MembershipVersion::new(2),
    100,
  )
  .expect("envelope");

  let handoff = transport.handoff_envelope(envelope, 100).expect("handoff");

  assert_eq!(handoff.to(), &peer);
  assert_eq!(handoff.payload_kind(), GossipPayloadKind::HeartbeatResponse);
  assert_eq!(handoff.target_endpoint(), "127.0.0.1:2552");
}

#[tokio::test]
async fn handoff_envelope_rejects_unknown_peer_identity() {
  let config = TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8);
  let transport = TokioGossipTransport::bind(config, Handle::current()).expect("transport bind");
  let peer = unique_address("127.0.0.1", 11);
  let envelope = GossipEnvelope::try_new(
    unique_address("127.0.0.2", 10),
    peer.clone(),
    GossipPayloadKind::FullState,
    MembershipVersion::new(2),
    100,
  )
  .expect("envelope");

  let err = transport.handoff_envelope(envelope, 100).expect_err("unknown peer should fail");

  assert_eq!(err, GossipTransportHandoffError::UnknownPeer { peer });
}

#[tokio::test]
async fn send_envelope_rejects_sender_that_differs_from_local_identity() {
  let local = unique_address("127.0.0.2", 10);
  let forged = unique_address("127.0.0.3", 12);
  let peer = unique_address("127.0.0.1", 11);
  let config = TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8)
    .with_local_identity(local.clone())
    .with_allowed_peer_identities(vec![peer.clone()]);
  let mut transport = TokioGossipTransport::bind(config, Handle::current()).expect("transport bind");
  let envelope =
    GossipEnvelope::try_new(forged.clone(), peer, GossipPayloadKind::FullState, MembershipVersion::new(2), 100)
      .expect("envelope");

  let err = transport.send_envelope(envelope, 100).expect_err("forged sender identity should fail");

  assert_eq!(
    err,
    GossipTransportError::Handoff(GossipTransportHandoffError::InvalidIdentity {
      expected: Box::new(local),
      actual:   Box::new(forged),
    })
  );
}

#[tokio::test]
async fn update_peer_identities_preserves_new_endpoint_mapping() {
  let peer_a = unique_address("127.0.0.1", 11);
  let peer_b = unique_address("127.0.0.2", 12);
  let config =
    TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8).with_allowed_peer_identities(vec![peer_a]);
  let mut transport = TokioGossipTransport::bind(config, Handle::current()).expect("transport bind");
  transport.update_peer_identities(vec![peer_b.clone()]);
  let envelope = GossipEnvelope::try_new(
    unique_address("127.0.0.3", 10),
    peer_b.clone(),
    GossipPayloadKind::Delta,
    MembershipVersion::new(2),
    100,
  )
  .expect("envelope");

  let handoff = transport.handoff_envelope(envelope, 100).expect("handoff");

  assert_eq!(handoff.to(), &peer_b);
  assert_eq!(handoff.target_endpoint(), "127.0.0.2:2552");
}

#[tokio::test]
async fn envelope_roundtrip_distinguishes_gossip_state_and_heartbeat_payloads() {
  let mut sender = TokioGossipTransport::bind(
    TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8),
    Handle::current(),
  )
  .expect("sender bind");
  let sender_addr = sender.local_addr();
  let sender_id = UniqueAddress::new(Address::new("cluster", "127.0.0.1", sender_addr.port()), 10);
  let receiver = TokioGossipTransport::bind(
    TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8)
      .with_allowed_peers(vec![sender_addr.to_string()])
      .with_allowed_peer_identities(vec![sender_id.clone()]),
    Handle::current(),
  )
  .expect("receiver bind");
  let receiver_addr = receiver.local_addr();
  let receiver_id = UniqueAddress::new(Address::new("cluster", "127.0.0.1", receiver_addr.port()), 11);
  sender.update_peer_identities(vec![receiver_id.clone()]);
  let mut receiver = receiver;
  receiver.update_local_identity(receiver_id.clone());

  sender
    .send_envelope(
      GossipEnvelope::try_new(
        sender_id.clone(),
        receiver_id.clone(),
        GossipPayloadKind::FullState,
        MembershipVersion::new(2),
        100,
      )
      .expect("full state envelope"),
      100,
    )
    .expect("send full state");
  sender
    .send_envelope(
      GossipEnvelope::try_new(
        sender_id.clone(),
        receiver_id,
        GossipPayloadKind::HeartbeatRequest,
        MembershipVersion::new(3),
        100,
      )
      .expect("heartbeat envelope"),
      100,
    )
    .expect("send heartbeat");

  for handoff in sender.poll_outbound_handoffs() {
    receiver.accept_handoff(handoff).expect("accept handoff");
  }
  let envelopes = receiver.poll_envelopes().into_iter().collect::<Result<Vec<_>, _>>().expect("valid envelopes");

  assert_eq!(envelopes.len(), 2);
  assert_eq!(envelopes[0].from(), &sender_id);
  assert_eq!(envelopes[0].payload_kind(), GossipPayloadKind::FullState);
  assert_eq!(envelopes[1].from(), &sender_id);
  assert_eq!(envelopes[1].payload_kind(), GossipPayloadKind::HeartbeatRequest);
}

#[tokio::test]
async fn accept_handoff_uses_advertised_local_identity_endpoint() {
  let mut sender = TokioGossipTransport::bind(
    TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8),
    Handle::current(),
  )
  .expect("sender bind");
  let sender_addr = sender.local_addr();
  let sender_id = UniqueAddress::new(Address::new("cluster", "127.0.0.1", sender_addr.port()), 10);
  let mut receiver = TokioGossipTransport::bind(
    TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8)
      .with_allowed_peer_identities(vec![sender_id.clone()]),
    Handle::current(),
  )
  .expect("receiver bind");
  let receiver_id =
    UniqueAddress::new(Address::new("cluster", "cluster-node.example.test", receiver.local_addr().port()), 11);
  sender.update_peer_identities(vec![receiver_id.clone()]);
  receiver.update_local_identity(receiver_id.clone());

  sender
    .send_envelope(
      GossipEnvelope::try_new(
        sender_id.clone(),
        receiver_id,
        GossipPayloadKind::FullState,
        MembershipVersion::new(2),
        100,
      )
      .expect("envelope"),
      100,
    )
    .expect("send envelope");
  let handoff = sender.poll_outbound_handoffs().remove(0);

  receiver.accept_handoff(handoff).expect("accept handoff");

  let envelopes = receiver.poll_envelopes().into_iter().collect::<Result<Vec<_>, _>>().expect("valid envelopes");
  assert_eq!(envelopes.len(), 1);
  assert_eq!(envelopes[0].from(), &sender_id);
}

#[tokio::test]
async fn accept_handoff_rejects_mismatched_target_identity() {
  let mut sender = TokioGossipTransport::bind(
    TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8),
    Handle::current(),
  )
  .expect("sender bind");
  let sender_addr = sender.local_addr();
  let sender_id = UniqueAddress::new(Address::new("cluster", "127.0.0.1", sender_addr.port()), 10);
  let mut receiver = TokioGossipTransport::bind(
    TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8)
      .with_allowed_peer_identities(vec![sender_id.clone()]),
    Handle::current(),
  )
  .expect("receiver bind");
  let receiver_addr = receiver.local_addr();
  let receiver_id = UniqueAddress::new(Address::new("cluster", "127.0.0.1", receiver_addr.port()), 11);
  let stale_receiver_id = UniqueAddress::new(Address::new("cluster", "127.0.0.1", receiver_addr.port()), 99);
  sender.update_peer_identities(vec![stale_receiver_id.clone()]);
  receiver.update_local_identity(receiver_id.clone());

  sender
    .send_envelope(
      GossipEnvelope::try_new(
        sender_id,
        stale_receiver_id.clone(),
        GossipPayloadKind::FullState,
        MembershipVersion::new(2),
        100,
      )
      .expect("envelope"),
      100,
    )
    .expect("send envelope");
  let handoff = sender.poll_outbound_handoffs().remove(0);

  let err = receiver.accept_handoff(handoff).expect_err("mismatched target identity should fail");

  assert_eq!(
    err,
    GossipTransportError::Handoff(GossipTransportHandoffError::InvalidIdentity {
      expected: Box::new(receiver_id),
      actual:   Box::new(stale_receiver_id),
    })
  );
}

#[tokio::test]
async fn invalid_logical_payload_kind_is_observable_as_transport_error() {
  let transport = TokioGossipTransport::bind(
    TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8),
    Handle::current(),
  )
  .expect("transport bind");

  let err = transport.receive_payload_kind_tag(99).expect_err("unknown payload kind should fail");

  assert_eq!(err, GossipTransportError::Handoff(GossipTransportHandoffError::UnknownPayloadKind { tag: 99 }));
}
