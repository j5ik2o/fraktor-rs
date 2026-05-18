use alloc::{string::String, vec};
use std::{
  net::{TcpListener as StdTcpListener, TcpStream as StdTcpStream},
  time::Duration,
};

use bytes::Bytes;
use fraktor_remote_core_rs::{
  address::{Address, UniqueAddress},
  config::RemoteCompressionConfig,
  transport::{TransportEndpoint, TransportError},
  wire::{AckPdu, ControlPdu, EnvelopePayload, EnvelopePdu, FlushScope, HandshakePdu, HandshakeReq},
};
use futures::SinkExt as _;
use tokio::{
  io::AsyncWriteExt as _,
  net::{TcpListener, TcpStream},
  sync::mpsc,
  time::timeout,
};
use tokio_util::codec::Framed;

use super::*;

fn ack_frame(sequence_number: u64) -> WireFrame {
  WireFrame::Ack(AckPdu::new(sequence_number, sequence_number.saturating_sub(1), 0))
}

fn flush_request_frame(flush_id: u64, lane_id: u32) -> WireFrame {
  WireFrame::Control(ControlPdu::FlushRequest {
    authority: String::from("local-sys@127.0.0.1:2551"),
    flush_id,
    scope: FlushScope::Shutdown,
    lane_id,
    expected_acks: 2,
  })
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

fn test_manifest_envelope_pdu(manifest: String) -> EnvelopePdu {
  EnvelopePdu::new(
    String::from("fraktor.tcp://local-sys@127.0.0.1:2551/user/a"),
    None,
    1,
    0,
    1,
    EnvelopePayload::new(5, Some(manifest), Bytes::from_static(b"payload")),
  )
}

async fn tcp_stream_pair() -> (TcpStream, TcpStream) {
  let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
  let address = listener.local_addr().expect("listener address");
  let client = TcpStream::connect(address);
  let (accepted, connected) = tokio::join!(listener.accept(), client);
  let (server, _) = accepted.expect("accept connection");
  let client = connected.expect("connect client");
  (client, server)
}

async fn tcp_framed_pair() -> (Framed<TcpStream, WireFrameCodec>, Framed<TcpStream, WireFrameCodec>) {
  let (client, server) = tcp_stream_pair().await;
  (Framed::new(client, WireFrameCodec::new()), Framed::new(server, WireFrameCodec::new()))
}

async fn tcp_stream_pair_with_client_control() -> (TcpStream, TcpStream, TcpStream) {
  let listener = StdTcpListener::bind("127.0.0.1:0").expect("bind std listener");
  let address = listener.local_addr().expect("listener address");
  let client = StdTcpStream::connect(address).expect("connect std client");
  let (server, _) = listener.accept().expect("accept std connection");
  client.set_nonblocking(true).expect("set client nonblocking");
  let client_control = client.try_clone().expect("clone std client");
  client_control.set_nonblocking(true).expect("set control nonblocking");
  server.set_nonblocking(true).expect("set server nonblocking");
  (
    TcpStream::from_std(client).expect("convert client stream"),
    TcpStream::from_std(client_control).expect("convert client control stream"),
    TcpStream::from_std(server).expect("convert server stream"),
  )
}

#[test]
fn writer_lane_index_uses_lane_zero_for_single_lane() {
  assert_eq!(writer_lane_index(b"", 1), 0);
  assert_eq!(writer_lane_index(b"recipient-a", 1), 0);
}

#[test]
fn writer_lane_index_is_stable_and_keyed() {
  let lane_count = 4;
  let base_key = b"recipient-a";
  let base_lane = writer_lane_index(base_key, lane_count);
  let different_key = (0_u8..=u8::MAX)
    .map(|candidate| [candidate])
    .find(|key| writer_lane_index(key, lane_count) != base_lane)
    .expect("test should find a key for a different lane");

  assert_eq!(writer_lane_index(base_key, lane_count), base_lane);
  assert_eq!(writer_lane_index(base_key, lane_count), base_lane);
  assert_ne!(writer_lane_index(&different_key, lane_count), base_lane);
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn next_writer_frame_drains_lanes_round_robin() {
  let (first_tx, first_rx) = mpsc::channel(4);
  let (second_tx, second_rx) = mpsc::channel(4);
  first_tx.try_send(ack_frame(1)).expect("first lane accepts frame");
  first_tx.try_send(ack_frame(2)).expect("first lane accepts second frame");
  second_tx.try_send(ack_frame(3)).expect("second lane accepts frame");
  drop(first_tx);
  drop(second_tx);
  let mut writer_rxs = vec![first_rx, second_rx];
  let mut next_writer_lane = 0;

  assert_eq!(next_writer_frame(&mut writer_rxs, &mut next_writer_lane).await, Some(ack_frame(1)));
  assert_eq!(next_writer_frame(&mut writer_rxs, &mut next_writer_lane).await, Some(ack_frame(3)));
  assert_eq!(next_writer_frame(&mut writer_rxs, &mut next_writer_lane).await, Some(ack_frame(2)));
  assert_eq!(next_writer_frame(&mut writer_rxs, &mut next_writer_lane).await, None);
}

#[test]
fn send_with_lane_key_reports_backpressure_for_selected_lane() {
  let (writer_tx, _writer_rx) = mpsc::channel(1);
  let client = TcpClient { peer_addr: String::from("peer"), writer_txs: vec![writer_tx], task: None };

  client.send_with_lane_key(b"recipient-a", ack_frame(1)).expect("first frame should fit");
  let error =
    client.send_with_lane_key(b"recipient-a", ack_frame(2)).expect_err("full selected lane should report backpressure");

  assert_eq!(error, TransportError::Backpressure);
}

#[test]
fn send_to_lane_id_uses_requested_writer_lane() {
  let (first_tx, mut first_rx) = mpsc::channel(1);
  let (second_tx, mut second_rx) = mpsc::channel(1);
  let client = TcpClient { peer_addr: String::from("peer"), writer_txs: vec![first_tx, second_tx], task: None };

  client.send_to_lane_id(1, flush_request_frame(7, 1)).expect("selected lane should accept flush request");

  assert!(first_rx.try_recv().is_err());
  assert_eq!(second_rx.try_recv().expect("second lane should receive flush request"), flush_request_frame(7, 1));
}

#[test]
fn send_to_lane_id_reports_backpressure_for_selected_lane() {
  let (writer_tx, _writer_rx) = mpsc::channel(1);
  let client = TcpClient { peer_addr: String::from("peer"), writer_txs: vec![writer_tx], task: None };

  client.send_to_lane_id(0, flush_request_frame(7, 0)).expect("first frame should fit");
  let error =
    client.send_to_lane_id(0, flush_request_frame(8, 0)).expect_err("full selected lane should report backpressure");

  assert_eq!(error, TransportError::Backpressure);
}

#[test]
fn send_to_lane_id_rejects_unknown_lane() {
  let (writer_tx, _writer_rx) = mpsc::channel(1);
  let client = TcpClient { peer_addr: String::from("peer"), writer_txs: vec![writer_tx], task: None };

  let error = client.send_to_lane_id(1, flush_request_frame(7, 1)).expect_err("unknown lane id should be rejected");

  assert_eq!(error, TransportError::NotAvailable);
}

#[test]
fn inbound_lane_index_keeps_same_authority_on_same_lane() {
  let authority = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
  let first = WireFrame::Envelope(test_envelope_pdu(
    String::from("fraktor.tcp://local-sys@127.0.0.1:2551/user/a"),
    Some(String::from("fraktor.tcp://remote-sys@10.0.0.1:2552/user/source")),
    1,
    0,
    1,
    Bytes::from_static(b"first"),
  ));
  let second = WireFrame::Envelope(test_envelope_pdu(
    String::from("fraktor.tcp://local-sys@127.0.0.1:2551/user/b"),
    None,
    2,
    0,
    1,
    Bytes::from_static(b"second"),
  ));

  let first_lane = inbound_lane_index("peer-a", Some(&authority), &first, 4);
  let second_lane = inbound_lane_index("peer-a", Some(&authority), &second, 4);

  assert_eq!(first_lane, second_lane);
}

#[test]
fn inbound_lane_index_can_use_frame_authority_before_state_is_known() {
  let from = UniqueAddress::new(Address::new("remote-sys", "10.0.0.1", 2552), 7);
  let to = Address::new("local-sys", "127.0.0.1", 2551);
  let frame = WireFrame::Handshake(HandshakePdu::Req(HandshakeReq::new(from, to)));

  let selected = inbound_lane_index("peer-a", None, &frame, 4);
  let expected = inbound_lane_index("peer-b", Some(&TransportEndpoint::new("remote-sys@10.0.0.1:2552")), &frame, 4);

  assert_eq!(selected, expected);
}

#[test]
fn forward_inbound_tcp_frame_stops_without_inbound_lanes() {
  let mut authority = None;

  let decision = forward_inbound_tcp_frame(ack_frame(1), "peer-a", &mut authority, &[]);

  assert!(matches!(decision, TcpClientLoopDecision::Stop(Some(TransportError::NotAvailable))));
}

#[test]
fn forward_inbound_tcp_frame_stops_when_selected_lane_is_closed() {
  let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
  drop(inbound_rx);
  let mut authority = None;

  let decision = forward_inbound_tcp_frame(ack_frame(1), "peer-a", &mut authority, &[inbound_tx]);

  assert!(matches!(decision, TcpClientLoopDecision::Stop(None)));
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn send_tcp_control_reply_reports_send_failure_after_close() {
  let (mut client, _server) = tcp_framed_pair().await;
  client.close().await.expect("close framed client");

  let decision =
    send_tcp_control_reply(&mut client, ControlPdu::Heartbeat { authority: String::from("local@127.0.0.1:2551") })
      .await;

  assert!(matches!(decision, TcpClientLoopDecision::Stop(Some(TransportError::SendFailed))));
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn send_outbound_tcp_frame_reports_send_failure_after_close() {
  let (mut client, _server) = tcp_framed_pair().await;
  let mut compression_tables = TcpCompressionTables::new(RemoteCompressionConfig::new());
  client.close().await.expect("close framed client");

  let error = send_outbound_tcp_frame(ack_frame(1), &mut client, &mut compression_tables, "peer-a").await;

  assert_eq!(error, Some(TransportError::SendFailed));
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn send_tcp_compression_advertisement_reports_send_failure_after_close() {
  let (mut client, _server) = tcp_framed_pair().await;
  let mut compression_tables = TcpCompressionTables::new(RemoteCompressionConfig::new());
  let _ = compression_tables.apply_outbound_frame(WireFrame::Envelope(test_envelope_pdu(
    String::from("fraktor.tcp://local-sys@127.0.0.1:2551/user/a"),
    None,
    1,
    0,
    1,
    Bytes::from_static(b"payload"),
  )));
  client.close().await.expect("close framed client");

  let error =
    send_tcp_compression_advertisement(&mut client, &mut compression_tables, CompressionTableKind::ActorRef, "local")
      .await;

  assert_eq!(error, Some(TransportError::SendFailed));
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn run_stops_when_writer_send_fails() {
  let (mut client, _server) = tcp_stream_pair().await;
  client.shutdown().await.expect("shutdown client writer");
  let (writer_tx, writer_rx) = mpsc::channel(1);
  writer_tx.send(ack_frame(1)).await.expect("writer lane accepts frame");
  let options = TcpClientRunOptions {
    frame_codec:              WireFrameCodec::new(),
    compression_config:       RemoteCompressionConfig::new(),
    local_authority:          String::from("local@127.0.0.1:2551"),
    connection_loss_reporter: None,
  };

  timeout(Duration::from_secs(1), run(client, String::from("peer-a"), vec![writer_rx], Vec::new(), options))
    .await
    .expect("client run should stop after writer send failure");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn run_stops_when_actor_ref_advertisement_send_fails() {
  run_stops_when_compression_advertisement_send_fails(
    RemoteCompressionConfig::new()
      .with_actor_ref_advertisement_interval(Duration::from_millis(1))
      .with_manifest_max(None),
    WireFrame::Envelope(test_envelope_pdu(
      String::from("fraktor.tcp://local-sys@127.0.0.1:2551/user/a"),
      None,
      1,
      0,
      1,
      Bytes::from_static(b"payload"),
    )),
  )
  .await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn run_stops_when_manifest_advertisement_send_fails() {
  run_stops_when_compression_advertisement_send_fails(
    RemoteCompressionConfig::new()
      .with_actor_ref_max(None)
      .with_manifest_advertisement_interval(Duration::from_millis(1)),
    WireFrame::Envelope(test_manifest_envelope_pdu(String::from("example.Manifest"))),
  )
  .await;
}

async fn run_stops_when_compression_advertisement_send_fails(
  compression_config: RemoteCompressionConfig,
  frame: WireFrame,
) {
  let (client, mut client_control, server) = tcp_stream_pair_with_client_control().await;
  let mut server = Framed::new(server, WireFrameCodec::new());
  let (writer_tx, writer_rx) = mpsc::channel(1);
  writer_tx.send(frame).await.expect("writer lane accepts frame");
  let options = TcpClientRunOptions {
    frame_codec: WireFrameCodec::new(),
    compression_config,
    local_authority: String::from("local@127.0.0.1:2551"),
    connection_loss_reporter: None,
  };
  let task = tokio::spawn(run(client, String::from("peer-a"), vec![writer_rx], Vec::new(), options));

  let first_frame = timeout(Duration::from_secs(1), server.next()).await.expect("first outbound frame should arrive");
  assert!(matches!(first_frame, Some(Ok(WireFrame::Envelope(_)))));
  client_control.shutdown().await.expect("shutdown client writer");

  timeout(Duration::from_secs(1), task)
    .await
    .expect("client run should stop after advertisement send failure")
    .unwrap();
}
