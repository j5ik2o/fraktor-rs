use alloc::{string::String, vec};

use bytes::Bytes;
use fraktor_remote_core_rs::{
  address::{Address, UniqueAddress},
  transport::{TransportEndpoint, TransportError},
  wire::{AckPdu, ControlPdu, EnvelopePayload, EnvelopePdu, FlushScope, HandshakePdu, HandshakeReq},
};
use tokio::sync::mpsc;

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

  assert_eq!(error, TransportError::ConnectionClosed);
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
