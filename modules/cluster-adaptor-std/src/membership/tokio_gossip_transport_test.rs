use core::time::Duration;

use fraktor_cluster_core_kernel_rs::membership::{
  GossipOutbound, GossipTransport, MembershipDelta, MembershipVersion, NodeRecord, NodeStatus,
};
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
