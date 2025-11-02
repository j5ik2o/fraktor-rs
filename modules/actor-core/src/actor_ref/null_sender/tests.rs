#![cfg(test)]

use crate::{actor_ref::actor_ref_sender::ActorRefSender, actor_ref::null_sender::NullSender, any_message::AnyMessage, send_error::SendError};

#[test]
fn always_returns_closed_error() {
  let sender = NullSender::default();
  let error = sender.send(AnyMessage::new(1_u8)).unwrap_err();
  match error {
    | SendError::Closed(_) => {}
    | other => panic!("expected closed error, got {other:?}"),
  }
}
