use core::time::Duration;

use bytes::{Bytes, BytesMut};
use fraktor_remote_core_rs::core::{
  address::{Address, UniqueAddress},
  transport::{RemoteTransport, TransportError},
  wire::{AckPdu, ControlPdu, EnvelopePdu, HandshakePdu, HandshakeReq, WireError},
};
use tokio_util::codec::{Decoder, Encoder};

use crate::std::tcp_transport::{frame_codec::WireFrameCodec, wire_frame::WireFrame};

#[test]
fn wire_frame_codec_roundtrips_envelope() {
  let pdu = EnvelopePdu::new("/user/a".into(), None, 42, 0, 1, Bytes::from_static(b"hello"));
  let frame = WireFrame::Envelope(pdu.clone());

  let mut codec = WireFrameCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(frame, &mut buf).unwrap();

  let decoded = codec.decode(&mut buf).unwrap().expect("complete frame");
  assert_eq!(decoded, WireFrame::Envelope(pdu));
  assert!(buf.is_empty(), "decoder should fully consume the buffer");
}

#[test]
fn wire_frame_codec_roundtrips_handshake() {
  let from = UniqueAddress::new(Address::new("sys", "host", 1234), 7);
  let to = Address::new("local-sys", "127.0.0.1", 2551);
  let pdu = HandshakePdu::Req(HandshakeReq::new(from, to));
  let frame = WireFrame::Handshake(pdu.clone());

  let mut codec = WireFrameCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(frame, &mut buf).unwrap();

  let decoded = codec.decode(&mut buf).unwrap().expect("complete frame");
  assert_eq!(decoded, WireFrame::Handshake(pdu));
}

#[test]
fn wire_frame_codec_roundtrips_control() {
  let pdu = ControlPdu::Heartbeat { authority: "sys@host:1".into() };
  let frame = WireFrame::Control(pdu.clone());

  let mut codec = WireFrameCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(frame, &mut buf).unwrap();

  let decoded = codec.decode(&mut buf).unwrap().expect("complete frame");
  assert_eq!(decoded, WireFrame::Control(pdu));
}

#[test]
fn wire_frame_codec_roundtrips_ack() {
  let pdu = AckPdu::new(10, 9, 0);
  let frame = WireFrame::Ack(pdu);

  let mut codec = WireFrameCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(frame, &mut buf).unwrap();

  let decoded = codec.decode(&mut buf).unwrap().expect("complete frame");
  assert_eq!(decoded, WireFrame::Ack(pdu));
}

#[test]
fn wire_frame_codec_returns_none_on_incomplete_frame() {
  let pdu = EnvelopePdu::new("/user/a".into(), None, 1, 0, 0, Bytes::new());
  let frame = WireFrame::Envelope(pdu);

  let mut codec = WireFrameCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(frame, &mut buf).unwrap();

  // Simulate partial arrival: split the encoded buffer in half.
  let half = buf.len() / 2;
  let mut partial = BytesMut::from(&buf[..half]);
  let result = codec.decode(&mut partial).unwrap();
  assert!(result.is_none(), "partial frame should yield None");
}

#[test]
fn wire_frame_codec_handles_multiple_frames_in_one_buffer() {
  let a = WireFrame::Envelope(EnvelopePdu::new("/a".into(), None, 1, 0, 0, Bytes::new()));
  let b = WireFrame::Envelope(EnvelopePdu::new("/b".into(), None, 2, 0, 1, Bytes::new()));

  let mut codec = WireFrameCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(a.clone(), &mut buf).unwrap();
  codec.encode(b.clone(), &mut buf).unwrap();

  let decoded_a = codec.decode(&mut buf).unwrap().expect("first frame");
  assert_eq!(decoded_a, a);
  let decoded_b = codec.decode(&mut buf).unwrap().expect("second frame");
  assert_eq!(decoded_b, b);
  assert!(buf.is_empty());
}

#[test]
fn wire_frame_codec_rejects_oversized_frame_length() {
  let mut codec = WireFrameCodec::new();
  let mut buf = BytesMut::new();
  // 宣言されたフレーム長が 16 MiB 上限を超えるケースを作る。
  buf.extend_from_slice(&(16 * 1024 * 1024 + 1_u32).to_be_bytes());
  // ヘッダ事前チェックを通すための最小バイト数を追加する。
  buf.extend_from_slice(&[1, 0]);

  let err = codec.decode(&mut buf).expect_err("oversized frame must be rejected");
  assert!(matches!(err, crate::std::tcp_transport::FrameCodecError::Wire(WireError::FrameTooLarge)));
  assert_eq!(buf.len(), 6, "oversized header must not partially consume the buffer");
}

#[test]
fn wire_frame_codec_rejects_declared_frame_length_smaller_than_header() {
  let mut codec = WireFrameCodec::new();
  let mut buf = BytesMut::new();
  // 宣言されたフレーム長には最低でも version + kind が必要。
  buf.extend_from_slice(&1_u32.to_be_bytes());
  // 外側のヘッダ長チェックを通すための十分なバイト数を追加する。
  buf.extend_from_slice(&[1, 0]);

  let err = codec.decode(&mut buf).expect_err("too-small frame length must be rejected");
  assert!(matches!(err, crate::std::tcp_transport::FrameCodecError::Wire(WireError::InvalidFormat)));
  assert_eq!(buf.len(), 6, "invalid header must not partially consume the buffer");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_start_binds_listener_and_receives_frame() {
  use tokio::sync::mpsc;

  use crate::std::tcp_transport::{TcpClient, TcpRemoteTransport};

  // Given
  let listen_address = Address::new("local-sys", "127.0.0.1", 0);
  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![listen_address]);
  let mut inbound_rx = transport.take_inbound_receiver().expect("inbound receiver should be available");

  // When
  transport.start().expect("start should bind listener");
  let bound_address = transport.default_address().expect("default address should be available").clone();
  assert_ne!(bound_address.port(), 0, "port 0 should be replaced by the actual bound port");

  let (client_inbound_tx, _client_inbound_rx) = mpsc::unbounded_channel();
  let mut client =
    TcpClient::connect(alloc::format!("{}:{}", bound_address.host(), bound_address.port()), client_inbound_tx)
      .await
      .expect("client should connect to started transport");
  let pdu = EnvelopePdu::new("/user/echo".into(), None, 0x1234, 0, 1, Bytes::from_static(b"hi"));
  client.send(WireFrame::Envelope(pdu.clone())).expect("client send should succeed");

  // Then
  let event = tokio::time::timeout(Duration::from_secs(5), inbound_rx.recv())
    .await
    .expect("frame should arrive before timeout")
    .expect("inbound frame should exist");
  assert_eq!(event.frame, WireFrame::Envelope(pdu));

  client.shutdown();
  transport.shutdown().expect("shutdown should stop started transport");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_start_rewrites_port_zero_advertised_addresses() {
  use crate::std::tcp_transport::TcpRemoteTransport;

  // Given
  let listen_address = Address::new("local-sys", "127.0.0.1", 0);
  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![listen_address]);

  // When
  transport.start().expect("start should bind listener");

  // Then
  let addresses = transport.addresses();
  assert_eq!(addresses.len(), 1);
  assert_ne!(addresses[0].port(), 0);
  assert_eq!(transport.default_address().expect("default address").port(), addresses[0].port());

  transport.shutdown().expect("shutdown should stop started transport");
}

#[test]
fn remote_transport_start_without_tokio_runtime_returns_not_available() {
  use crate::std::tcp_transport::TcpRemoteTransport;

  // Given
  let listen_address = Address::new("local-sys", "127.0.0.1", 0);
  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![listen_address]);

  // When
  let error = transport.start().expect_err("start without a tokio runtime should fail");

  // Then
  assert_eq!(error, TransportError::NotAvailable);
  assert_eq!(
    transport.shutdown().expect_err("failed start must not mark transport running"),
    TransportError::NotStarted
  );
}

// ---------------------------------------------------------------------------
// 2-node echo integration test
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn tcp_server_and_client_exchange_a_frame() {
  use tokio::sync::mpsc;

  use crate::std::tcp_transport::{client::TcpClient, server::TcpServer};

  let (server_inbound_tx, mut server_inbound_rx) = mpsc::unbounded_channel();
  let mut server = TcpServer::new("127.0.0.1:0".into());
  let bind_addr = server.start(server_inbound_tx).expect("server should bind to a system-assigned port");

  let (client_inbound_tx, _client_inbound_rx) = mpsc::unbounded_channel();
  let client = TcpClient::connect(bind_addr.to_string(), client_inbound_tx).await.unwrap();

  let pdu = EnvelopePdu::new("/user/echo".into(), None, 0x1234, 0, 1, Bytes::from_static(b"hi"));
  client.send(WireFrame::Envelope(pdu.clone())).unwrap();

  let event = tokio::time::timeout(Duration::from_secs(5), server_inbound_rx.recv())
    .await
    .unwrap()
    .expect("server inbound frame");
  assert_eq!(event.frame, WireFrame::Envelope(pdu));

  server.shutdown();
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_send_handshake_writes_handshake_frame_to_connected_peer() {
  use tokio::sync::mpsc;

  use crate::std::tcp_transport::{TcpRemoteTransport, server::TcpServer};

  let (server_inbound_tx, mut server_inbound_rx) = mpsc::unbounded_channel();
  let mut server = TcpServer::new("127.0.0.1:0".into());
  let bind_addr = server.start(server_inbound_tx).expect("server should bind to a system-assigned port");

  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![Address::new("local-sys", "127.0.0.1", 0)]);
  transport.start().expect("transport should start before connecting a peer");

  let remote = Address::new("remote-sys", bind_addr.ip().to_string(), bind_addr.port());
  transport.connect_peer(&remote).await.expect("transport should connect to peer before sending handshake");

  let from = UniqueAddress::new(transport.default_address().expect("default local address").clone(), 1);
  let pdu = HandshakePdu::Req(HandshakeReq::new(from, remote.clone()));
  transport.send_handshake(&remote, pdu.clone()).expect("handshake send should be enqueued");

  let event = tokio::time::timeout(Duration::from_secs(5), server_inbound_rx.recv())
    .await
    .expect("handshake should arrive before timeout")
    .expect("server inbound frame");
  assert_eq!(event.frame, WireFrame::Handshake(pdu));

  transport.shutdown().expect("transport shutdown should succeed");
  server.shutdown();
}
