use core::time::Duration;
use std::{net::SocketAddr, time::Instant};

use bytes::{Bytes, BytesMut};
use fraktor_actor_adaptor_std_rs::system::create_noop_actor_system;
use fraktor_actor_core_kernel_rs::{
  actor::{
    actor_path::{ActorPath, ActorPathParser},
    messaging::AnyMessage,
  },
  event::stream::CorrelationId,
  serialization::{SerializationExtensionShared, default_serialization_extension_id},
  support::ByteString,
};
use fraktor_remote_core_rs::{
  address::{Address, RemoteNodeId, UniqueAddress},
  config::{RemoteCompressionConfig, RemoteConfig},
  envelope::{OutboundEnvelope, OutboundPriority},
  extension::RemoteEvent,
  transport::{RemoteTransport, TransportEndpoint, TransportError},
  wire::{
    AckPdu, CompressionTableEntry, CompressionTableKind, ControlPdu, EnvelopePayload, EnvelopePdu, HandshakePdu,
    HandshakeReq, WireError,
  },
};
use fraktor_utils_core_rs::sync::ArcShared;
use futures::{SinkExt as _, StreamExt as _};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::codec::{Decoder, Encoder, Framed};

use super::base::outbound_envelope_to_pdu;
use crate::transport::tcp::{
  WireFrame,
  client::{TcpClient, TcpClientConnectOptions},
  frame_codec::WireFrameCodec,
  frame_codec_error::FrameCodecError,
  inbound_frame_event::InboundFrameEvent,
  server::TcpServer,
};

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

fn test_envelope_pdu(
  recipient_path: String,
  sender_path: Option<String>,
  correlation_hi: u64,
  correlation_lo: u32,
  priority: u8,
  payload: Bytes,
) -> EnvelopePdu {
  EnvelopePdu::new(
    recipient_path,
    sender_path,
    correlation_hi,
    correlation_lo,
    priority,
    EnvelopePayload::new(5, None, payload),
  )
}

fn serialization_extension() -> ArcShared<SerializationExtensionShared> {
  let system = create_noop_actor_system();
  system.extended().register_extension(&default_serialization_extension_id())
}

fn outbound_pdu(envelope: &OutboundEnvelope) -> Result<EnvelopePdu, TransportError> {
  let serialization_extension = serialization_extension();
  outbound_envelope_to_pdu(envelope, &serialization_extension)
}

fn serialized_string_payload(value: &str) -> Bytes {
  let bytes = value.as_bytes();
  let mut payload = Vec::with_capacity(4 + bytes.len());
  payload.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
  payload.extend_from_slice(bytes);
  Bytes::from(payload)
}

struct UnsupportedPayload;

fn large_envelope_frame() -> WireFrame {
  WireFrame::Envelope(test_envelope_pdu(
    "/user/large".into(),
    None,
    0x14,
    0,
    1,
    Bytes::from(vec![0_u8; MINIMUM_MAXIMUM_FRAME_SIZE]),
  ))
}

fn test_remote_node(port: u16) -> RemoteNodeId {
  RemoteNodeId::new("remote-sys", "127.0.0.1", Some(port), 1)
}

fn test_recipient(port: u16) -> ActorPath {
  ActorPathParser::parse(&alloc::format!("fraktor.tcp://remote-sys@127.0.0.1:{port}/user/worker")).expect("parse")
}

fn test_envelope(
  port: u16,
  message: AnyMessage,
  correlation_id: CorrelationId,
  sender: Option<ActorPath>,
) -> OutboundEnvelope {
  OutboundEnvelope::new(
    test_recipient(port),
    sender,
    message,
    OutboundPriority::User,
    test_remote_node(port),
    correlation_id,
  )
}

fn assert_connection_lost_event(
  event: RemoteEvent,
  expected_authority: TransportEndpoint,
  expected_cause: TransportError,
) {
  match event {
    | RemoteEvent::ConnectionLost { authority, cause, .. } => {
      assert_eq!(authority, expected_authority);
      assert_eq!(cause, expected_cause);
    },
    | other => panic!("expected connection-lost event, got {other:?}"),
  }
}

fn assert_inbound_frame_event(event: RemoteEvent, expected_frame: WireFrame) {
  match event {
    | RemoteEvent::InboundFrameReceived { frame, .. } => assert_eq!(frame, expected_frame),
    | other => panic!("expected inbound-frame event, got {other:?}"),
  }
}

fn connect_test_client(peer_addr: String, inbound_tx: UnboundedSender<InboundFrameEvent>) -> TcpClient {
  TcpClient::connect(peer_addr, vec![inbound_tx], TcpClientConnectOptions::new(WireFrameCodec::new()))
    .expect("client should schedule connection")
}

fn make_test_server() -> TcpServer {
  TcpServer::with_frame_codec_and_compression_config(
    String::from("127.0.0.1:0"),
    WireFrameCodec::new(),
    RemoteCompressionConfig::new(),
  )
}

fn start_test_server(server: &mut TcpServer, inbound_tx: UnboundedSender<InboundFrameEvent>) -> SocketAddr {
  server
    .start_with_remote_events(vec![inbound_tx], None, Instant::now(), |bound_port| {
      format!("local@127.0.0.1:{bound_port}")
    })
    .expect("server should bind to a system-assigned port")
}

#[test]
fn outbound_envelope_to_pdu_preserves_metadata_for_vec_u8_payload() {
  let sender = ActorPathParser::parse("fraktor.tcp://local-sys@127.0.0.1:2551/user/source").expect("parse sender");
  let correlation_id = CorrelationId::new(0x1111_2222_3333_4444, 0x5566_7788);
  let payload = Vec::from(&b"vec payload"[..]);
  let envelope = test_envelope(2552, AnyMessage::new(payload), correlation_id, Some(sender.clone()));

  let pdu = outbound_pdu(&envelope).expect("Vec<u8> payload should be supported");

  assert_eq!(pdu.recipient_path(), envelope.recipient().to_canonical_uri());
  assert_eq!(pdu.sender_path(), Some(sender.to_canonical_uri().as_str()));
  assert_eq!(pdu.priority(), OutboundPriority::User.to_wire());
  assert_eq!(pdu.correlation_hi(), correlation_id.hi());
  assert_eq!(pdu.correlation_lo(), correlation_id.lo());
  assert_eq!(pdu.serializer_id(), 5);
  assert_eq!(pdu.manifest(), None);
  assert_eq!(pdu.payload(), &Bytes::from_static(b"vec payload"));
}

#[test]
fn outbound_envelope_to_pdu_supports_byte_string_payload() {
  let envelope =
    test_envelope(2552, AnyMessage::new(ByteString::from_slice(b"byte string payload")), CorrelationId::nil(), None);

  let pdu = outbound_pdu(&envelope).expect("ByteString payload should be supported");

  assert_eq!(pdu.serializer_id(), 6);
  assert_eq!(pdu.manifest(), None);
  assert_eq!(pdu.payload(), &Bytes::from_static(b"byte string payload"));
}

#[test]
fn outbound_envelope_to_pdu_supports_string_payload() {
  let envelope = test_envelope(2552, AnyMessage::new(String::from("payload")), CorrelationId::nil(), None);

  let pdu = outbound_pdu(&envelope).expect("String payload should be supported");

  assert_eq!(pdu.serializer_id(), 4);
  assert_eq!(pdu.manifest(), None);
  assert_eq!(pdu.payload(), &serialized_string_payload("payload"));
}

#[test]
fn outbound_envelope_to_pdu_rejects_bytes_payload_without_custom_serializer() {
  let envelope = test_envelope(2552, AnyMessage::new(Bytes::from_static(b"bytes payload")), CorrelationId::nil(), None);

  let err = outbound_pdu(&envelope).expect_err("bytes::Bytes should require a custom serializer");

  assert_eq!(err, TransportError::SendFailed);
}

#[test]
fn outbound_envelope_to_pdu_rejects_unsupported_payload() {
  let envelope = test_envelope(2552, AnyMessage::new(UnsupportedPayload), CorrelationId::nil(), None);

  let err = outbound_pdu(&envelope).expect_err("unregistered payload should fail serialization");

  assert_eq!(err, TransportError::SendFailed);
}

#[test]
fn wire_frame_codec_roundtrips_envelope() {
  let pdu = test_envelope_pdu("/user/a".into(), None, 42, 0, 1, Bytes::from_static(b"hello"));
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
  let pdu = test_envelope_pdu("/user/a".into(), None, 1, 0, 1, Bytes::new());
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
  let a = WireFrame::Envelope(test_envelope_pdu("/a".into(), None, 1, 0, 1, Bytes::new()));
  let b = WireFrame::Envelope(test_envelope_pdu("/b".into(), None, 2, 0, 1, Bytes::new()));

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
  assert!(matches!(err, FrameCodecError::Wire(WireError::FrameTooLarge)));
  assert_eq!(buf.len(), 6, "oversized header must not partially consume the buffer");
}

#[test]
fn wire_frame_codec_rejects_frame_above_configured_maximum_frame_size() {
  let mut codec = WireFrameCodec::with_maximum_frame_size(64 * 1024);
  let mut buf = BytesMut::new();
  append_declared_frame_header(&mut buf, 64 * 1024 + 1);

  let err = codec.decode(&mut buf).expect_err("oversized frame must be rejected");
  assert!(matches!(err, FrameCodecError::Wire(WireError::FrameTooLarge)));
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
  assert!(matches!(err, FrameCodecError::Wire(WireError::FrameTooLarge)));
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
  assert!(matches!(err, FrameCodecError::Wire(WireError::InvalidFormat)));
  assert_eq!(buf.len(), 6, "invalid header must not partially consume the buffer");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_start_binds_listener_and_receives_frame() {
  use tokio::sync::mpsc;

  use crate::transport::tcp::TcpRemoteTransport;

  // Given: port 0 で listen する transport
  let listen_address = Address::new("local-sys", "127.0.0.1", 0);
  let (event_tx, mut event_rx) = mpsc::channel(4);
  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![listen_address]).with_remote_event_sender(event_tx);

  // When: transport を開始して peer から frame を送る
  transport.start().expect("start should bind listener");
  let bound_address = transport.default_address().expect("default address should be available").clone();
  assert_ne!(bound_address.port(), 0, "port 0 should be replaced by the actual bound port");

  let (client_inbound_tx, _client_inbound_rx) = mpsc::unbounded_channel();
  let mut client =
    connect_test_client(alloc::format!("{}:{}", bound_address.host(), bound_address.port()), client_inbound_tx);
  let pdu = test_envelope_pdu("/user/echo".into(), None, 0x1234, 0, 1, Bytes::from_static(b"hi"));
  client.send(WireFrame::Envelope(pdu.clone())).expect("client send should succeed");

  // Then: remote event 経路で frame を受け取れる
  let event = tokio::time::timeout(Duration::from_secs(5), event_rx.recv())
    .await
    .expect("frame should arrive before timeout")
    .expect("inbound event should exist");
  assert_inbound_frame_event(event, WireFrame::Envelope(pdu));

  client.shutdown();
  transport.shutdown().expect("shutdown should stop started transport");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_start_rewrites_port_zero_advertised_addresses() {
  use crate::transport::tcp::TcpRemoteTransport;

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
  use crate::transport::tcp::TcpRemoteTransport;

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

  use crate::transport::tcp::TcpRemoteTransport;

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
async fn remote_transport_client_connection_close_emits_connection_lost() {
  use tokio::{net::TcpListener, sync::mpsc};

  use crate::transport::tcp::TcpRemoteTransport;

  let listener = TcpListener::bind("127.0.0.1:0").await.expect("peer listener should bind");
  let bind_addr = listener.local_addr().expect("peer listener local addr");
  let (event_tx, mut event_rx) = mpsc::channel(4);
  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![Address::new("local-sys", "127.0.0.1", 0)])
    .with_remote_event_sender(event_tx);
  transport.start().expect("transport should start before connecting a peer");
  let remote = Address::new("remote-sys", bind_addr.ip().to_string(), bind_addr.port());

  transport.connect_peer(&remote).expect("transport should connect to peer");
  let (accepted_stream, _) = listener.accept().await.expect("peer should accept transport connection");
  drop(accepted_stream);

  let event = tokio::time::timeout(Duration::from_secs(5), event_rx.recv())
    .await
    .expect("connection-lost event should arrive")
    .expect("connection-lost event should be present");
  assert_connection_lost_event(event, TransportEndpoint::new(remote.to_string()), TransportError::ConnectionClosed);

  transport.shutdown().expect("transport shutdown should succeed");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_reconnects_after_client_connection_loss() {
  use tokio::{net::TcpListener, sync::mpsc};

  use crate::transport::tcp::TcpRemoteTransport;

  let listener = TcpListener::bind("127.0.0.1:0").await.expect("peer listener should bind");
  let bind_addr = listener.local_addr().expect("peer listener local addr");
  let (event_tx, mut event_rx) = mpsc::channel(4);
  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![Address::new("local-sys", "127.0.0.1", 0)])
    .with_remote_event_sender(event_tx);
  transport.start().expect("transport should start before connecting a peer");
  let remote = Address::new("remote-sys", bind_addr.ip().to_string(), bind_addr.port());

  transport.connect_peer(&remote).expect("transport should connect to peer");
  let (first_stream, _) = listener.accept().await.expect("peer should accept initial connection");
  drop(first_stream);

  let event = tokio::time::timeout(Duration::from_secs(5), event_rx.recv())
    .await
    .expect("connection-lost event should arrive")
    .expect("connection-lost event should be present");
  assert_connection_lost_event(event, TransportEndpoint::new(remote.to_string()), TransportError::ConnectionClosed);

  let (second_stream, _) = tokio::time::timeout(Duration::from_secs(5), async {
    loop {
      transport.connect_peer(&remote).expect("transport should reconnect to peer");
      if let Ok(accepted) = tokio::time::timeout(Duration::from_millis(50), listener.accept()).await {
        break accepted.expect("peer should accept replacement connection");
      }
    }
  })
  .await
  .expect("replacement connection should be accepted");
  let mut framed = Framed::new(second_stream, WireFrameCodec::new());
  let from = UniqueAddress::new(transport.default_address().expect("default local address").clone(), 1);
  let pdu = HandshakePdu::Req(HandshakeReq::new(from, remote.clone()));

  transport.send_handshake(&remote, pdu.clone()).expect("handshake send should use replacement writer");
  let frame = tokio::time::timeout(Duration::from_secs(5), framed.next())
    .await
    .expect("replacement connection should receive a frame")
    .expect("replacement connection should stay open")
    .expect("replacement connection frame should decode");
  assert_eq!(frame, WireFrame::Handshake(pdu));

  transport.shutdown().expect("transport shutdown should succeed");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_server_connection_close_emits_connection_lost_after_authority_is_known() {
  use tokio::sync::mpsc;

  use crate::transport::tcp::TcpRemoteTransport;

  let (event_tx, mut event_rx) = mpsc::channel(4);
  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![Address::new("local-sys", "127.0.0.1", 0)])
    .with_remote_event_sender(event_tx);
  transport.start().expect("transport should start before accepting a peer");
  let bound_address = transport.default_address().expect("default address should be available").clone();
  let (client_inbound_tx, _client_inbound_rx) = mpsc::unbounded_channel();
  let mut client =
    connect_test_client(alloc::format!("{}:{}", bound_address.host(), bound_address.port()), client_inbound_tx);
  let remote = Address::new("remote-sys", "127.0.0.1", 2552);
  let from = UniqueAddress::new(remote.clone(), 7);
  let pdu = HandshakePdu::Req(HandshakeReq::new(from, bound_address));

  client.send(WireFrame::Handshake(pdu)).expect("client send should succeed");
  let inbound_event = tokio::time::timeout(Duration::from_secs(5), event_rx.recv())
    .await
    .expect("inbound frame event should arrive")
    .expect("inbound frame event should be present");
  assert!(matches!(
    inbound_event,
    RemoteEvent::InboundFrameReceived {
      authority,
      ..
    } if authority == TransportEndpoint::new(remote.to_string())
  ));

  client.shutdown();
  let connection_lost = tokio::time::timeout(Duration::from_secs(5), event_rx.recv())
    .await
    .expect("connection-lost event should arrive")
    .expect("connection-lost event should be present");
  assert_connection_lost_event(
    connection_lost,
    TransportEndpoint::new(remote.to_string()),
    TransportError::ConnectionClosed,
  );

  transport.shutdown().expect("transport shutdown should succeed");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_shutdown_does_not_emit_connection_lost() {
  use tokio::{net::TcpListener, sync::mpsc};

  use crate::transport::tcp::TcpRemoteTransport;

  let listener = TcpListener::bind("127.0.0.1:0").await.expect("peer listener should bind");
  let bind_addr = listener.local_addr().expect("peer listener local addr");
  let (event_tx, mut event_rx) = mpsc::channel(4);
  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![Address::new("local-sys", "127.0.0.1", 0)])
    .with_remote_event_sender(event_tx);
  transport.start().expect("transport should start before connecting a peer");
  let remote = Address::new("remote-sys", bind_addr.ip().to_string(), bind_addr.port());

  transport.connect_peer(&remote).expect("transport should connect to peer");
  let (accepted_stream, _) = listener.accept().await.expect("peer should accept transport connection");
  transport.shutdown().expect("transport shutdown should succeed");

  let event = tokio::time::timeout(Duration::from_millis(100), event_rx.recv()).await;
  assert!(event.is_err(), "transport shutdown must not emit connection-lost recovery events");
  drop(accepted_stream);
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_restart_respawns_inbound_worker() {
  use tokio::sync::mpsc;

  use crate::transport::tcp::TcpRemoteTransport;

  let (event_tx, mut event_rx) = mpsc::channel(4);
  let listen_address = Address::new("local-sys", "127.0.0.1", 0);
  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![listen_address]).with_remote_event_sender(event_tx);

  transport.start().expect("initial transport start should succeed");
  transport.shutdown().expect("initial transport shutdown should succeed");
  transport.start().expect("restart should recreate the inbound worker");

  let bound_address = transport.default_address().expect("default address should be available").clone();
  let (client_inbound_tx, _client_inbound_rx) = mpsc::unbounded_channel();
  let mut client =
    connect_test_client(alloc::format!("{}:{}", bound_address.host(), bound_address.port()), client_inbound_tx);
  let remote = Address::new("remote-sys", "127.0.0.1", 2552);
  let from = UniqueAddress::new(remote.clone(), 7);
  let pdu = HandshakePdu::Req(HandshakeReq::new(from, bound_address));

  client.send(WireFrame::Handshake(pdu)).expect("client send should succeed");

  let event = tokio::time::timeout(Duration::from_secs(5), event_rx.recv())
    .await
    .expect("restarted inbound worker should emit a remote event")
    .expect("remote event should be present");
  assert!(matches!(
    event,
    RemoteEvent::InboundFrameReceived {
      authority,
      ..
    } if authority == TransportEndpoint::new(remote.to_string())
  ));

  client.shutdown();
  transport.shutdown().expect("transport shutdown should succeed");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_handshake_timeout_uses_configured_monotonic_epoch() {
  use tokio::sync::mpsc;

  use crate::transport::tcp::TcpRemoteTransport;

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

  use crate::transport::tcp::TcpRemoteTransport;

  // Given: canonical host と bind host を分けた構成
  let config =
    RemoteConfig::new("canonical.example").with_canonical_port(0).with_bind_hostname("127.0.0.1").with_bind_port(0);
  let (event_tx, mut event_rx) = mpsc::channel(4);
  let mut transport = TcpRemoteTransport::from_config("local-sys", config).with_remote_event_sender(event_tx);

  // When: bind override 経由で接続する
  transport.start().expect("transport should bind using configured bind address");
  let advertised = transport.default_address().expect("default address should be available").clone();

  // Then: advertised address は canonical host を保持する
  assert_eq!(advertised.system(), "local-sys");
  assert_eq!(advertised.host(), "canonical.example");
  assert_ne!(advertised.port(), 0);

  let (client_inbound_tx, _client_inbound_rx) = mpsc::unbounded_channel();
  let mut client = connect_test_client(alloc::format!("127.0.0.1:{}", advertised.port()), client_inbound_tx);
  let pdu = test_envelope_pdu("/user/bind".into(), None, 0x12, 0, 1, Bytes::from_static(b"hi"));
  client.send(WireFrame::Envelope(pdu.clone())).expect("client send should succeed");

  let event = tokio::time::timeout(Duration::from_secs(5), event_rx.recv())
    .await
    .expect("frame should arrive before timeout")
    .expect("inbound event should exist");
  assert_inbound_frame_event(event, WireFrame::Envelope(pdu));

  client.shutdown();
  transport.shutdown().expect("transport shutdown should succeed");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_from_config_falls_back_to_canonical_bind_address() {
  use tokio::sync::mpsc;

  use crate::transport::tcp::TcpRemoteTransport;

  // Given: bind override を持たない canonical 構成
  let config = RemoteConfig::new("127.0.0.1").with_canonical_port(0);
  let (event_tx, mut event_rx) = mpsc::channel(4);
  let mut transport = TcpRemoteTransport::from_config("local-sys", config).with_remote_event_sender(event_tx);

  // When: canonical address 経由で接続する
  transport.start().expect("transport should bind using canonical address");
  let advertised = transport.default_address().expect("default address should be available").clone();

  // Then: canonical address が bind と advertised address の両方に使われる
  assert_eq!(advertised.system(), "local-sys");
  assert_eq!(advertised.host(), "127.0.0.1");
  assert_ne!(advertised.port(), 0);

  let (client_inbound_tx, _client_inbound_rx) = mpsc::unbounded_channel();
  let mut client =
    connect_test_client(alloc::format!("{}:{}", advertised.host(), advertised.port()), client_inbound_tx);
  let pdu = test_envelope_pdu("/user/canonical".into(), None, 0x13, 0, 1, Bytes::from_static(b"hi"));
  client.send(WireFrame::Envelope(pdu.clone())).expect("client send should succeed");

  let event = tokio::time::timeout(Duration::from_secs(5), event_rx.recv())
    .await
    .expect("frame should arrive before timeout")
    .expect("inbound event should exist");
  assert_inbound_frame_event(event, WireFrame::Envelope(pdu));

  client.shutdown();
  transport.shutdown().expect("transport shutdown should succeed");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_from_config_applies_maximum_frame_size_to_inbound_decoder() {
  use tokio::sync::mpsc;

  use crate::transport::tcp::TcpRemoteTransport;

  // Given: inbound decoder の最大 frame size を最小値にした構成
  let config =
    RemoteConfig::new("127.0.0.1").with_canonical_port(0).with_maximum_frame_size(MINIMUM_MAXIMUM_FRAME_SIZE);
  let (event_tx, mut event_rx) = mpsc::channel(4);
  let mut transport = TcpRemoteTransport::from_config("local-sys", config).with_remote_event_sender(event_tx);

  // When: 設定上限を超える frame を送る
  transport.start().expect("transport should bind listener");
  let advertised = transport.default_address().expect("default address should be available").clone();

  let (client_inbound_tx, _client_inbound_rx) = mpsc::unbounded_channel();
  let mut client =
    connect_test_client(alloc::format!("{}:{}", advertised.host(), advertised.port()), client_inbound_tx);
  let pdu =
    test_envelope_pdu("/user/large".into(), None, 0x14, 0, 1, Bytes::from(vec![0_u8; MINIMUM_MAXIMUM_FRAME_SIZE]));
  client.send(WireFrame::Envelope(pdu)).expect("client send should succeed");

  // Then: inbound delivery 前に拒否される
  let result = tokio::time::timeout(Duration::from_millis(200), event_rx.recv()).await;
  assert!(result.is_err(), "oversized inbound frame should be rejected before remote event delivery");

  client.shutdown();
  transport.shutdown().expect("transport shutdown should succeed");
}

// ---------------------------------------------------------------------------
// 2-node echo integration test
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn tcp_server_and_client_exchange_a_frame() {
  use tokio::sync::mpsc;

  let (server_inbound_tx, mut server_inbound_rx) = mpsc::unbounded_channel();
  let mut server = make_test_server();
  let bind_addr = start_test_server(&mut server, server_inbound_tx);

  let (client_inbound_tx, _client_inbound_rx) = mpsc::unbounded_channel();
  let client = connect_test_client(bind_addr.to_string(), client_inbound_tx);

  let pdu = test_envelope_pdu("/user/echo".into(), None, 0x1234, 0, 1, Bytes::from_static(b"hi"));
  client.send(WireFrame::Envelope(pdu.clone())).unwrap();

  let event = tokio::time::timeout(Duration::from_secs(5), server_inbound_rx.recv())
    .await
    .unwrap()
    .expect("server inbound frame");
  assert_eq!(event.frame, WireFrame::Envelope(pdu));

  server.shutdown();
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn tcp_server_consumes_compression_advertisement_and_replies_with_ack() {
  use tokio::{net::TcpStream, sync::mpsc};

  let (server_inbound_tx, mut server_inbound_rx) = mpsc::unbounded_channel();
  let mut server = make_test_server();
  let bind_addr = start_test_server(&mut server, server_inbound_tx);
  let mut framed =
    Framed::new(TcpStream::connect(bind_addr).await.expect("client stream should connect"), WireFrameCodec::new());
  let advertisement = WireFrame::Control(ControlPdu::CompressionAdvertisement {
    authority:  "remote@host:1".to_string(),
    table_kind: CompressionTableKind::ActorRef,
    generation: 7,
    entries:    vec![CompressionTableEntry::new(3, "/user/a".to_string())],
  });

  framed.send(advertisement).await.expect("advertisement should be written");
  let ack = tokio::time::timeout(Duration::from_secs(5), framed.next())
    .await
    .expect("ack should arrive before timeout")
    .expect("ack frame")
    .expect("ack frame should decode");

  assert!(matches!(
    ack,
    WireFrame::Control(ControlPdu::CompressionAck { table_kind: CompressionTableKind::ActorRef, generation: 7, .. })
  ));
  tokio::task::yield_now().await;
  assert!(
    matches!(server_inbound_rx.try_recv(), Err(mpsc::error::TryRecvError::Empty)),
    "compression advertisement must not reach the inbound event loop"
  );
  server.shutdown();
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_send_handshake_writes_handshake_frame_to_connected_peer() {
  use tokio::sync::mpsc;

  use crate::transport::tcp::TcpRemoteTransport;

  let (server_inbound_tx, mut server_inbound_rx) = mpsc::unbounded_channel();
  let mut server = make_test_server();
  let bind_addr = start_test_server(&mut server, server_inbound_tx);

  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![Address::new("local-sys", "127.0.0.1", 0)]);
  transport.start().expect("transport should start before connecting a peer");

  let remote = Address::new("remote-sys", bind_addr.ip().to_string(), bind_addr.port());
  transport.connect_peer(&remote).expect("transport should connect to peer before sending handshake");

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
async fn remote_transport_send_writes_envelope_frame_to_connected_peer() {
  use tokio::sync::mpsc;

  use crate::transport::tcp::TcpRemoteTransport;

  let (server_inbound_tx, mut server_inbound_rx) = mpsc::unbounded_channel();
  let mut server = make_test_server();
  let bind_addr = start_test_server(&mut server, server_inbound_tx);

  let serialization_extension = serialization_extension();
  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![Address::new("local-sys", "127.0.0.1", 0)])
    .with_serialization_extension(serialization_extension.clone());
  transport.start().expect("transport should start before connecting a peer");

  let remote = Address::new("remote-sys", bind_addr.ip().to_string(), bind_addr.port());
  transport.connect_peer(&remote).expect("transport should connect to peer before sending envelope");
  let envelope = test_envelope(
    bind_addr.port(),
    AnyMessage::new(Vec::from(&b"payload"[..])),
    CorrelationId::new(0x1234, 0x5678),
    None,
  );
  let expected = outbound_envelope_to_pdu(&envelope, &serialization_extension).expect("test payload should encode");
  assert_eq!(expected.serializer_id(), 5);
  assert_eq!(expected.manifest(), None);
  assert_eq!(expected.payload(), &Bytes::from_static(b"payload"));

  transport.send(envelope).expect("supported envelope should be enqueued");

  let event = tokio::time::timeout(Duration::from_secs(5), server_inbound_rx.recv())
    .await
    .expect("envelope should arrive before timeout")
    .expect("server inbound frame");
  assert_eq!(event.frame, WireFrame::Envelope(expected));

  transport.shutdown().expect("transport shutdown should succeed");
  server.shutdown();
}

#[test]
fn remote_transport_send_before_start_returns_original_envelope() {
  use crate::transport::tcp::TcpRemoteTransport;

  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![Address::new("local-sys", "127.0.0.1", 0)]);
  let envelope = test_envelope(2552, AnyMessage::new(Bytes::from_static(b"payload")), CorrelationId::nil(), None);

  let (err, returned) = transport.send(envelope).expect_err("send before start should fail with the original envelope");

  assert_eq!(err, TransportError::NotStarted);
  assert_eq!(returned.message().downcast_ref::<Bytes>(), Some(&Bytes::from_static(b"payload")));
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_send_without_peer_writer_returns_original_envelope() {
  use crate::transport::tcp::TcpRemoteTransport;

  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![Address::new("local-sys", "127.0.0.1", 0)]);
  transport.start().expect("transport should start");
  let envelope = test_envelope(2552, AnyMessage::new(Bytes::from_static(b"payload")), CorrelationId::nil(), None);

  let (err, returned) = transport.send(envelope).expect_err("missing peer writer should return the original envelope");

  assert_eq!(err, TransportError::ConnectionClosed);
  assert_eq!(returned.message().downcast_ref::<Bytes>(), Some(&Bytes::from_static(b"payload")));
  transport.shutdown().expect("transport shutdown should succeed");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_send_without_serialization_extension_returns_original_envelope() {
  use tokio::sync::mpsc;

  use crate::transport::tcp::TcpRemoteTransport;

  let (server_inbound_tx, mut server_inbound_rx) = mpsc::unbounded_channel();
  let mut server = make_test_server();
  let bind_addr = start_test_server(&mut server, server_inbound_tx);

  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![Address::new("local-sys", "127.0.0.1", 0)]);
  transport.start().expect("transport should start before connecting a peer");

  let remote = Address::new("remote-sys", bind_addr.ip().to_string(), bind_addr.port());
  transport.connect_peer(&remote).expect("transport should connect to peer before sending envelope");
  let envelope =
    test_envelope(bind_addr.port(), AnyMessage::new(Vec::from(&b"payload"[..])), CorrelationId::nil(), None);

  let result = transport.send(envelope);

  let (err, returned) = result.expect_err("send should reject envelopes until serialization is connected");
  assert_eq!(err, TransportError::NotAvailable);
  assert_eq!(returned.message().downcast_ref::<Vec<u8>>(), Some(&Vec::from(&b"payload"[..])));
  let inbound = tokio::time::timeout(Duration::from_millis(200), server_inbound_rx.recv()).await;
  assert!(inbound.is_err(), "missing serialization extension must not emit an envelope frame");
  transport.shutdown().expect("transport shutdown should succeed");
  server.shutdown();
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_transport_send_rejects_unsupported_payload_without_emitting_frame() {
  use tokio::sync::mpsc;

  use crate::transport::tcp::TcpRemoteTransport;

  let (server_inbound_tx, mut server_inbound_rx) = mpsc::unbounded_channel();
  let mut server = make_test_server();
  let bind_addr = start_test_server(&mut server, server_inbound_tx);

  let mut transport = TcpRemoteTransport::new("127.0.0.1:0", vec![Address::new("local-sys", "127.0.0.1", 0)])
    .with_serialization_extension(serialization_extension());
  transport.start().expect("transport should start before connecting a peer");

  let remote = Address::new("remote-sys", bind_addr.ip().to_string(), bind_addr.port());
  transport.connect_peer(&remote).expect("transport should connect to peer before sending envelope");
  let recipient = ActorPathParser::parse("fraktor.tcp://remote-sys@127.0.0.1:2552/user/worker").expect("parse");
  let envelope = OutboundEnvelope::new(
    recipient,
    None,
    AnyMessage::new(UnsupportedPayload),
    OutboundPriority::User,
    RemoteNodeId::new("remote-sys", bind_addr.ip().to_string(), Some(bind_addr.port()), 1),
    CorrelationId::nil(),
  );

  let result = transport.send(envelope);

  let (err, returned) = result.expect_err("send should reject unsupported payload before emitting any frame");
  assert_eq!(err, TransportError::SendFailed);
  assert!(returned.message().downcast_ref::<UnsupportedPayload>().is_some());
  let inbound = tokio::time::timeout(Duration::from_millis(200), server_inbound_rx.recv()).await;
  assert!(inbound.is_err(), "unsupported payload send must not emit an empty payload frame");
  transport.shutdown().expect("transport shutdown should succeed");
  server.shutdown();
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn server_shutdown_aborts_existing_connection_read_loops() {
  use tokio::sync::mpsc;

  let (server_inbound_tx, mut server_inbound_rx) = mpsc::unbounded_channel();
  let mut server = make_test_server();
  let bind_addr = start_test_server(&mut server, server_inbound_tx);

  let (client_inbound_tx, _client_inbound_rx) = mpsc::unbounded_channel();
  let client = connect_test_client(bind_addr.to_string(), client_inbound_tx);
  let pdu = test_envelope_pdu("/user/echo".into(), None, 0x1234, 0, 1, Bytes::from_static(b"hi"));
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
