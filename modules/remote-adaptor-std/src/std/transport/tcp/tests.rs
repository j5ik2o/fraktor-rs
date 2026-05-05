use core::time::Duration;
use std::time::Instant;

use bytes::{Bytes, BytesMut};
use fraktor_actor_core_rs::core::kernel::{
  actor::{actor_path::ActorPathParser, messaging::AnyMessage},
  event::stream::CorrelationId,
};
use fraktor_remote_core_rs::core::{
  address::{Address, RemoteNodeId, UniqueAddress},
  config::RemoteConfig,
  envelope::{OutboundEnvelope, OutboundPriority},
  extension::RemoteEvent,
  transport::{RemoteTransport, TransportEndpoint, TransportError},
  wire::{AckPdu, ControlPdu, EnvelopePdu, HandshakePdu, HandshakeReq, WireError},
};
use tokio_util::codec::{Decoder, Encoder};

use crate::std::transport::tcp::{frame_codec::WireFrameCodec, wire_frame::WireFrame};

const DEFAULT_MAXIMUM_FRAME_SIZE: usize = 256 * 1024;
const MINIMUM_MAXIMUM_FRAME_SIZE: usize = 32 * 1024;

fn append_declared_frame_header(buf: &mut BytesMut, length: usize) {
  let length = u32::try_from(length).expect("test frame length should fit in u32");
  buf.extend_from_slice(&length.to_be_bytes());
  buf.extend_from_slice(&[1, 0]);
}

fn declared_frame_length(buf: &BytesMut) -> usize {
  let bytes = [buf[0], buf[1], buf[2], buf[3]];
  u32::from_be_bytes(bytes) as usize
}

fn large_envelope_frame() -> WireFrame {
  WireFrame::Envelope(EnvelopePdu::new(
    "/user/large".into(),
    None,
    0x14,
    0,
    1,
    Bytes::from(vec![0_u8; MINIMUM_MAXIMUM_FRAME_SIZE]),
  ))
}

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
  append_declared_frame_header(&mut buf, DEFAULT_MAXIMUM_FRAME_SIZE + 1);

  let err = codec.decode(&mut buf).expect_err("oversized frame must be rejected");
  assert!(matches!(err, crate::std::transport::tcp::FrameCodecError::Wire(WireError::FrameTooLarge)));
  assert_eq!(buf.len(), 6, "oversized header must not partially consume the buffer");
}

#[test]
fn wire_frame_codec_rejects_frame_above_configured_maximum_frame_size() {
  let mut codec = WireFrameCodec::with_maximum_frame_size(64 * 1024);
  let mut buf = BytesMut::new();
  append_declared_frame_header(&mut buf, 64 * 1024 + 1);

  let err = codec.decode(&mut buf).expect_err("oversized frame must be rejected");
  assert!(matches!(err, crate::std::transport::tcp::FrameCodecError::Wire(WireError::FrameTooLarge)));
  assert_eq!(buf.len(), 6, "oversized header must not partially consume the buffer");
}

#[test]
fn wire_frame_codec_rejects_outbound_frame_above_configured_maximum_frame_size() {
  // Given: outbound codec の最大 frame size を最小値にした構成
  let mut codec = WireFrameCodec::with_maximum_frame_size(MINIMUM_MAXIMUM_FRAME_SIZE);
  let frame = large_envelope_frame();
  let mut buf = BytesMut::from(&b"existing bytes"[..]);
  let original = buf.clone();

  // When: 設定上限を超える frame を encode する
  let err = codec.encode(frame, &mut buf).expect_err("oversized outbound frame must be rejected");

  // Then: FrameTooLarge を返し、既存の送信バッファを変更しない
  assert!(matches!(err, crate::std::transport::tcp::FrameCodecError::Wire(WireError::FrameTooLarge)));
  assert_eq!(buf, original, "failed outbound encode must not mutate the destination buffer");
}

#[test]
fn wire_frame_codec_allows_outbound_frame_equal_to_configured_maximum_frame_size() {
  // Given: declared length と同じ最大 frame size
  let frame = large_envelope_frame();
  let mut probe_codec = WireFrameCodec::new();
  let mut probe = BytesMut::new();
  probe_codec.encode(frame.clone(), &mut probe).expect("probe encode should succeed");
  let declared_length = declared_frame_length(&probe);
  let mut codec = WireFrameCodec::with_maximum_frame_size(declared_length);
  let mut buf = BytesMut::new();

  // When: declared length が上限ちょうどの frame を encode する
  codec.encode(frame, &mut buf).expect("frame at the outbound limit should be accepted");

  // Then: length prefix が表す値を境界として許可する
  assert_eq!(declared_frame_length(&buf), declared_length);
}

#[test]
fn wire_frame_codec_rejects_declared_frame_length_smaller_than_header() {
  let mut codec = WireFrameCodec::new();
  let mut buf = BytesMut::new();
  append_declared_frame_header(&mut buf, 1);

  let err = codec.decode(&mut buf).expect_err("too-small frame length must be rejected");
  assert!(matches!(err, crate::std::transport::tcp::FrameCodecError::Wire(WireError::InvalidFormat)));
  assert_eq!(buf.len(), 6, "invalid header must not partially consume the buffer");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_start_binds_listener_and_receives_frame() {
  use tokio::sync::mpsc;

  use crate::std::transport::tcp::{TcpClient, TcpRemoteTransport};

  // Given: port 0 で listen する transport
  let listen_address = Address::new("local-sys", "127.0.0.1", 0);
  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![listen_address]);
  let mut inbound_rx = transport.take_inbound_receiver().expect("inbound receiver should be available");

  // When: transport を開始して peer から frame を送る
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

  // Then: inbound receiver で frame を受け取れる
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
  use crate::std::transport::tcp::TcpRemoteTransport;

  // Given: port 0 を含む advertised address
  let listen_address = Address::new("local-sys", "127.0.0.1", 0);
  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![listen_address]);

  // When: transport を開始する
  transport.start().expect("start should bind listener");

  // Then: advertised address の port が実 bind port に置き換わる
  let addresses = transport.addresses();
  assert_eq!(addresses.len(), 1);
  assert_ne!(addresses[0].port(), 0);
  assert_eq!(transport.default_address().expect("default address").port(), addresses[0].port());

  transport.shutdown().expect("shutdown should stop started transport");
}

#[test]
fn remote_transport_start_without_tokio_runtime_returns_not_available() {
  use crate::std::transport::tcp::TcpRemoteTransport;

  // Given: Tokio runtime 外で作成した transport
  let listen_address = Address::new("local-sys", "127.0.0.1", 0);
  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![listen_address]);

  // When: runtime 外で開始する
  let error = transport.start().expect_err("start without a tokio runtime should fail");

  // Then: 未利用状態のまま NotAvailable を返す
  assert_eq!(error, TransportError::NotAvailable);
  assert_eq!(
    transport.shutdown().expect_err("failed start must not mark transport running"),
    TransportError::NotStarted
  );
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_schedules_handshake_timeout_event() {
  use tokio::sync::mpsc;

  use crate::std::transport::tcp::TcpRemoteTransport;

  let (event_tx, mut event_rx) = mpsc::channel(1);
  let listen_address = Address::new("local-sys", "127.0.0.1", 0);
  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![listen_address]).with_remote_event_sender(event_tx);
  transport.start().expect("transport should start before scheduling a timer");
  let authority = TransportEndpoint::new("remote-sys@10.0.0.1:2552");

  transport
    .schedule_handshake_timeout(&authority, Duration::from_millis(1), 7)
    .expect("timer scheduling should succeed");

  let event = tokio::time::timeout(Duration::from_secs(5), event_rx.recv())
    .await
    .expect("timeout event should arrive")
    .expect("timeout event should be present");
  assert!(matches!(
    event,
    RemoteEvent::HandshakeTimerFired {
      authority: received_authority,
      generation: 7,
      ..
    } if received_authority == authority
  ));

  transport.shutdown().expect("transport shutdown should succeed");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_handshake_timeout_uses_configured_monotonic_epoch() {
  use tokio::sync::mpsc;

  use crate::std::transport::tcp::TcpRemoteTransport;

  let (event_tx, mut event_rx) = mpsc::channel(1);
  let listen_address = Address::new("local-sys", "127.0.0.1", 0);
  let monotonic_epoch = Instant::now() - Duration::from_secs(1);
  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![listen_address])
    .with_monotonic_epoch(monotonic_epoch)
    .with_remote_event_sender(event_tx);
  transport.start().expect("transport should start before scheduling a timer");
  let authority = TransportEndpoint::new("remote-sys@10.0.0.1:2552");

  transport
    .schedule_handshake_timeout(&authority, Duration::from_millis(1), 7)
    .expect("timer scheduling should succeed");

  let event = tokio::time::timeout(Duration::from_secs(5), event_rx.recv())
    .await
    .expect("timeout event should arrive")
    .expect("timeout event should be present");
  assert!(matches!(
    event,
    RemoteEvent::HandshakeTimerFired {
      now_ms,
      ..
    } if now_ms >= 1_000
  ));

  transport.shutdown().expect("transport shutdown should succeed");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_from_config_uses_bind_override_and_advertises_canonical_address() {
  use tokio::sync::mpsc;

  use crate::std::transport::tcp::{TcpClient, TcpRemoteTransport};

  // Given: canonical host と bind host を分けた構成
  let config =
    RemoteConfig::new("canonical.example").with_canonical_port(0).with_bind_hostname("127.0.0.1").with_bind_port(0);
  let mut transport = TcpRemoteTransport::from_config("local-sys", config);
  let mut inbound_rx = transport.take_inbound_receiver().expect("inbound receiver should be available");

  // When: bind override 経由で接続する
  transport.start().expect("transport should bind using configured bind address");
  let advertised = transport.default_address().expect("default address should be available").clone();

  // Then: advertised address は canonical host を保持する
  assert_eq!(advertised.system(), "local-sys");
  assert_eq!(advertised.host(), "canonical.example");
  assert_ne!(advertised.port(), 0);

  let (client_inbound_tx, _client_inbound_rx) = mpsc::unbounded_channel();
  let mut client = TcpClient::connect(alloc::format!("127.0.0.1:{}", advertised.port()), client_inbound_tx)
    .await
    .expect("client should connect through bind override");
  let pdu = EnvelopePdu::new("/user/bind".into(), None, 0x12, 0, 1, Bytes::from_static(b"hi"));
  client.send(WireFrame::Envelope(pdu.clone())).expect("client send should succeed");

  let event = tokio::time::timeout(Duration::from_secs(5), inbound_rx.recv())
    .await
    .expect("frame should arrive before timeout")
    .expect("inbound frame should exist");
  assert_eq!(event.frame, WireFrame::Envelope(pdu));

  client.shutdown();
  transport.shutdown().expect("transport shutdown should succeed");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_from_config_falls_back_to_canonical_bind_address() {
  use tokio::sync::mpsc;

  use crate::std::transport::tcp::{TcpClient, TcpRemoteTransport};

  // Given: bind override を持たない canonical 構成
  let config = RemoteConfig::new("127.0.0.1").with_canonical_port(0);
  let mut transport = TcpRemoteTransport::from_config("local-sys", config);
  let mut inbound_rx = transport.take_inbound_receiver().expect("inbound receiver should be available");

  // When: canonical address 経由で接続する
  transport.start().expect("transport should bind using canonical address");
  let advertised = transport.default_address().expect("default address should be available").clone();

  // Then: canonical address が bind と advertised address の両方に使われる
  assert_eq!(advertised.system(), "local-sys");
  assert_eq!(advertised.host(), "127.0.0.1");
  assert_ne!(advertised.port(), 0);

  let (client_inbound_tx, _client_inbound_rx) = mpsc::unbounded_channel();
  let mut client = TcpClient::connect(alloc::format!("{}:{}", advertised.host(), advertised.port()), client_inbound_tx)
    .await
    .expect("client should connect through canonical bind address");
  let pdu = EnvelopePdu::new("/user/canonical".into(), None, 0x13, 0, 1, Bytes::from_static(b"hi"));
  client.send(WireFrame::Envelope(pdu.clone())).expect("client send should succeed");

  let event = tokio::time::timeout(Duration::from_secs(5), inbound_rx.recv())
    .await
    .expect("frame should arrive before timeout")
    .expect("inbound frame should exist");
  assert_eq!(event.frame, WireFrame::Envelope(pdu));

  client.shutdown();
  transport.shutdown().expect("transport shutdown should succeed");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_from_config_applies_maximum_frame_size_to_inbound_decoder() {
  use tokio::sync::mpsc;

  use crate::std::transport::tcp::{TcpClient, TcpRemoteTransport};

  // Given: inbound decoder の最大 frame size を最小値にした構成
  let config =
    RemoteConfig::new("127.0.0.1").with_canonical_port(0).with_maximum_frame_size(MINIMUM_MAXIMUM_FRAME_SIZE);
  let mut transport = TcpRemoteTransport::from_config("local-sys", config);
  let mut inbound_rx = transport.take_inbound_receiver().expect("inbound receiver should be available");

  // When: 設定上限を超える frame を送る
  transport.start().expect("transport should bind listener");
  let advertised = transport.default_address().expect("default address should be available").clone();

  let (client_inbound_tx, _client_inbound_rx) = mpsc::unbounded_channel();
  let mut client = TcpClient::connect(alloc::format!("{}:{}", advertised.host(), advertised.port()), client_inbound_tx)
    .await
    .expect("client should connect to started transport");
  let pdu =
    EnvelopePdu::new("/user/large".into(), None, 0x14, 0, 1, Bytes::from(vec![0_u8; MINIMUM_MAXIMUM_FRAME_SIZE]));
  client.send(WireFrame::Envelope(pdu)).expect("client send should succeed");

  // Then: inbound delivery 前に拒否される
  let result = tokio::time::timeout(Duration::from_millis(200), inbound_rx.recv()).await;
  assert!(result.is_err(), "oversized inbound frame should be rejected before delivery");

  client.shutdown();
  transport.shutdown().expect("transport shutdown should succeed");
}

// ---------------------------------------------------------------------------
// 2-node echo integration test
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn tcp_server_and_client_exchange_a_frame() {
  use tokio::sync::mpsc;

  use crate::std::transport::tcp::{client::TcpClient, server::TcpServer};

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

  use crate::std::transport::tcp::{TcpRemoteTransport, server::TcpServer};

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

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_send_rejects_user_envelope_until_payload_serialization_is_installed() {
  use tokio::sync::mpsc;

  use crate::std::transport::tcp::{TcpRemoteTransport, server::TcpServer};

  let (server_inbound_tx, mut server_inbound_rx) = mpsc::unbounded_channel();
  let mut server = TcpServer::new("127.0.0.1:0".into());
  let bind_addr = server.start(server_inbound_tx).expect("server should bind to a system-assigned port");

  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![Address::new("local-sys", "127.0.0.1", 0)]);
  transport.start().expect("transport should start before connecting a peer");

  let remote = Address::new("remote-sys", bind_addr.ip().to_string(), bind_addr.port());
  transport.connect_peer(&remote).await.expect("transport should connect to peer before sending envelope");
  let recipient = ActorPathParser::parse("fraktor.tcp://remote-sys@127.0.0.1:2552/user/worker").expect("parse");
  let envelope = OutboundEnvelope::new(
    recipient,
    None,
    AnyMessage::new(String::from("payload")),
    OutboundPriority::User,
    RemoteNodeId::new("remote-sys", bind_addr.ip().to_string(), Some(bind_addr.port()), 1),
    CorrelationId::nil(),
  );

  let result = transport.send(envelope);

  let (err, _envelope) = result.expect_err("send should fail when peer write loop is gone");
  assert_eq!(err, TransportError::SendFailed);
  let inbound = tokio::time::timeout(Duration::from_millis(200), server_inbound_rx.recv()).await;
  assert!(inbound.is_err(), "failed envelope send must not emit an empty payload frame");
  transport.shutdown().expect("transport shutdown should succeed");
  server.shutdown();
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn server_shutdown_aborts_existing_connection_read_loops() {
  use tokio::sync::mpsc;

  use crate::std::transport::tcp::{client::TcpClient, server::TcpServer};

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
    .expect("server inbound frame should arrive before shutdown");
  assert_eq!(event.frame, WireFrame::Envelope(pdu));

  server.shutdown();

  // shutdown 後はサーバ側の inbound_tx がドロップされるため、 channel が close され受信が None
  // を返す。
  let after_shutdown = tokio::time::timeout(Duration::from_secs(5), server_inbound_rx.recv()).await;
  match after_shutdown {
    | Ok(None) => {},
    | Ok(Some(event)) => panic!("no inbound frames should follow shutdown but got {event:?}"),
    | Err(_) => panic!("shutdown must close the inbound channel within the timeout"),
  }
}
