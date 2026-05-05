use alloc::string::String;

use bytes::Bytes;
use fraktor_remote_core_rs::core::{transport::TransportEndpoint, wire::EnvelopePdu};

use super::{WireFrame, authority_for_frame};

#[test]
fn senderless_envelope_does_not_use_recipient_authority() {
  let frame = WireFrame::Envelope(EnvelopePdu::new(
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
fn envelope_authority_comes_from_sender_path_when_present() {
  let frame = WireFrame::Envelope(EnvelopePdu::new(
    String::from("fraktor.tcp://local-sys@127.0.0.1:2551/user/worker"),
    Some(String::from("fraktor.tcp://remote-sys@10.0.0.1:2552/user/sender")),
    0,
    0,
    1,
    Bytes::new(),
  ));

  assert_eq!(authority_for_frame(&frame), Some(TransportEndpoint::new("remote-sys@10.0.0.1:2552")));
}
