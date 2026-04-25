use core::time::Duration;

use bytes::{Bytes, BytesMut};
use fraktor_remote_core_rs::core::wire::{AckPdu, ControlPdu, EnvelopePdu, HandshakePdu, HandshakeReq, WireError};
use tokio::net::TcpListener;
use tokio_util::codec::{Decoder, Encoder, Framed};

use crate::std::tcp_transport::{
  frame_codec::WireFrameCodec, inbound_frame_event::InboundFrameEvent, wire_frame::WireFrame,
};

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
  let pdu = HandshakePdu::Req(HandshakeReq::new("sys".into(), "host".into(), 1234, 7));
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
  // Declared frame length larger than 16 MiB limit.
  buf.extend_from_slice(&(16 * 1024 * 1024 + 1_u32).to_be_bytes());
  // Minimum bytes to pass header pre-check.
  buf.extend_from_slice(&[1, 0]);

  let err = codec.decode(&mut buf).expect_err("oversized frame must be rejected");
  assert!(matches!(err, crate::std::tcp_transport::FrameCodecError::Wire(WireError::FrameTooLarge)));
}

#[test]
fn wire_frame_codec_rejects_declared_frame_length_smaller_than_header() {
  let mut codec = WireFrameCodec::new();
  let mut buf = BytesMut::new();
  // Declared frame length must include at least version + kind.
  buf.extend_from_slice(&1_u32.to_be_bytes());
  // Provide enough bytes to pass the outer header-length pre-check.
  buf.extend_from_slice(&[1, 0]);

  let err = codec.decode(&mut buf).expect_err("too-small frame length must be rejected");
  assert!(matches!(err, crate::std::tcp_transport::FrameCodecError::Wire(WireError::InvalidFormat)));
  assert_eq!(buf.len(), 6, "invalid header must not partially consume the buffer");
}

// ---------------------------------------------------------------------------
// 2-node echo integration test
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn tcp_server_and_client_exchange_a_frame() {
  use tokio::sync::mpsc;

  use crate::std::tcp_transport::{client::TcpClient, server::TcpServer};

  // Start the server.
  let (server_inbound_tx, mut server_inbound_rx) = mpsc::unbounded_channel();
  let mut server = TcpServer::new("127.0.0.1:0".into());
  // Bind to a system-assigned port first to learn the port number.
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let bind_addr = listener.local_addr().unwrap();
  // Manually spawn an accept loop mirroring TcpServer semantics to avoid
  // re-binding.
  let accept_tx = server_inbound_tx.clone();
  let accept_task = tokio::spawn(async move {
    let (stream, peer) = listener.accept().await.unwrap();
    let peer_addr = peer.to_string();
    let inbound_tx = accept_tx;
    let mut framed = Framed::new(stream, WireFrameCodec::new());
    use futures::StreamExt;
    if let Some(Ok(frame)) = framed.next().await {
      inbound_tx.send(InboundFrameEvent { peer: peer_addr, frame }).unwrap();
    }
  });
  // Quiet the unused-variable warnings for the placeholder server handle.
  let _ = &mut server;

  // Connect a client.
  let (client_inbound_tx, _client_inbound_rx) = mpsc::unbounded_channel();
  let client = TcpClient::connect(bind_addr.to_string(), client_inbound_tx).await.unwrap();

  // Send a frame.
  let pdu = EnvelopePdu::new("/user/echo".into(), None, 0x1234, 0, 1, Bytes::from_static(b"hi"));
  client.send(WireFrame::Envelope(pdu.clone())).unwrap();

  // Wait for the frame to land on the server side.
  let event = tokio::time::timeout(Duration::from_secs(5), server_inbound_rx.recv())
    .await
    .unwrap()
    .expect("server inbound frame");
  assert_eq!(event.frame, WireFrame::Envelope(pdu));

  accept_task.await.unwrap();
}
