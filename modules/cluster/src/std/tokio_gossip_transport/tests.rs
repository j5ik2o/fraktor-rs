use core::time::Duration;

use tokio::net::UdpSocket;

use crate::{
  core::membership::{GossipOutbound, GossipTransport, MembershipDelta, MembershipVersion, NodeRecord, NodeStatus},
  std::{TokioGossipTransport, TokioGossipTransportConfig},
};

fn free_udp_addr() -> std::net::SocketAddr {
  let socket = std::net::UdpSocket::bind("127.0.0.1:0").expect("bind");
  socket.local_addr().expect("local addr")
}

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
  let bind_addr = free_udp_addr();
  let config = TokioGossipTransportConfig::new(bind_addr.to_string(), 1024, 8);
  let mut transport = TokioGossipTransport::bind(config, tokio::runtime::Handle::current()).expect("transport bind");
  let outbound = GossipOutbound::new(String::from("invalid-authority"), sample_delta());
  let result = transport.send(outbound);
  assert!(result.is_err());
}

#[tokio::test]
async fn poll_returns_empty_when_no_messages() {
  let bind_addr = free_udp_addr();
  let config = TokioGossipTransportConfig::new(bind_addr.to_string(), 1024, 8);
  let mut transport = TokioGossipTransport::bind(config, tokio::runtime::Handle::current()).expect("transport bind");
  let deltas = transport.poll_deltas();
  assert!(deltas.is_empty());
}

#[tokio::test]
async fn recv_returns_delta_from_udp() {
  let bind_addr = free_udp_addr();
  let config = TokioGossipTransportConfig::new(bind_addr.to_string(), 1024, 8);
  let mut transport = TokioGossipTransport::bind(config, tokio::runtime::Handle::current()).expect("transport bind");

  let sender = UdpSocket::bind("127.0.0.1:0").await.expect("sender bind");
  let payload = transport.encode_delta(&sample_delta()).expect("encode");
  sender.send_to(&payload, bind_addr).await.expect("send");

  tokio::time::sleep(Duration::from_millis(50)).await;
  let deltas = transport.poll_deltas();
  assert_eq!(deltas.len(), 1);
  assert_eq!(deltas[0].1, sample_delta());
}
