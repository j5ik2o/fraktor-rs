use alloc::string::String;

use bytes::Bytes;
use fraktor_remote_core_rs::{
  transport::TransportError,
  wire::{EnvelopePayload, EnvelopePdu},
};
use tokio::sync::mpsc;

use super::{InboundFrameEvent, WireFrame, authority_for_frame, run_inbound_dispatch};

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
fn senderless_envelope_does_not_use_recipient_authority() {
  let frame = WireFrame::Envelope(test_envelope_pdu(
    String::from("fraktor.tcp://local-sys@127.0.0.1:2551/user/worker"),
    None,
    0,
    0,
    1,
    Bytes::new(),
  ));

  assert_eq!(authority_for_frame(&frame), None);
}

#[test]
fn envelope_sender_path_does_not_define_authority() {
  let frame = WireFrame::Envelope(test_envelope_pdu(
    String::from("fraktor.tcp://local-sys@127.0.0.1:2551/user/worker"),
    Some(String::from("fraktor.tcp://remote-sys@10.0.0.1:2552/user/sender")),
    0,
    0,
    1,
    Bytes::new(),
  ));

  assert_eq!(authority_for_frame(&frame), None);
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn inbound_dispatch_returns_not_available_when_remote_event_receiver_is_closed() {
  let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
  let (event_tx, event_rx) = mpsc::channel(1);
  drop(event_rx);
  inbound_tx
    .send(InboundFrameEvent {
      peer:      String::from("peer-a"),
      authority: None,
      frame:     WireFrame::Envelope(test_envelope_pdu(String::from("/user/worker"), None, 1, 0, 1, Bytes::new())),
    })
    .expect("inbound frame should be accepted");
  drop(inbound_tx);

  let error = run_inbound_dispatch(inbound_rx, event_tx, || 42)
    .await
    .expect_err("closed remote event receiver should surface as transport failure");

  assert_eq!(error, TransportError::NotAvailable);
}
