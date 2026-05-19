use bytes::Bytes;
use fraktor_actor_core_kernel_rs::{
  actor::{
    actor_path::{ActorPath, ActorPathParser},
    messaging::AnyMessage,
  },
  event::stream::CorrelationId,
};
use fraktor_remote_core_rs::{
  address::RemoteNodeId,
  config::RemoteConfig,
  envelope::{OutboundEnvelope, OutboundPriority},
};

use super::*;

fn test_actor_path(uri: &str) -> ActorPath {
  ActorPathParser::parse(uri).expect("test actor path should parse")
}

fn outbound_envelope(correlation_id: CorrelationId) -> OutboundEnvelope {
  OutboundEnvelope::new(
    test_actor_path("fraktor.tcp://remote-sys@127.0.0.1:2552/user/worker"),
    Some(test_actor_path("fraktor.tcp://local-sys@127.0.0.1:2551/user/source")),
    AnyMessage::new(Bytes::from_static(b"payload")),
    OutboundPriority::User,
    RemoteNodeId::new("remote-sys", "127.0.0.1", Some(2552), 1),
    correlation_id,
  )
}

#[test]
fn from_config_applies_inbound_and_outbound_lane_counts() {
  let config = RemoteConfig::new("127.0.0.1").with_inbound_lanes(3).with_outbound_lanes(4);
  let transport = TcpRemoteTransport::from_config("local-sys", config);

  assert_eq!(transport.inbound_lanes, 3);
  assert_eq!(transport.outbound_lanes, 4);
  assert_eq!(transport.inbound_txs.len(), 3);
  assert_eq!(transport.inbound_rxs.as_ref().expect("inbound receivers").len(), 3);
}

#[test]
fn outbound_lane_key_ignores_correlation_id_for_same_actor_pair() {
  let first = outbound_envelope(CorrelationId::new(0x1111, 0x2222));
  let second = outbound_envelope(CorrelationId::new(0x3333, 0x4444));

  assert_eq!(outbound_lane_key_for_envelope(&first), outbound_lane_key_for_envelope(&second));
}

#[test]
fn local_authority_from_addresses_rewrites_ephemeral_port() {
  let addresses = vec![Address::new("local-sys", "127.0.0.1", 0)];

  let authority = TcpRemoteTransport::local_authority_from_addresses(&addresses, 2551);

  assert_eq!(authority, "local-sys@127.0.0.1:2551");
}

#[test]
fn local_authority_from_addresses_preserves_fixed_port() {
  let addresses = vec![Address::new("local-sys", "127.0.0.1", 2552)];

  let authority = TcpRemoteTransport::local_authority_from_addresses(&addresses, 2551);

  assert_eq!(authority, "local-sys@127.0.0.1:2552");
}
