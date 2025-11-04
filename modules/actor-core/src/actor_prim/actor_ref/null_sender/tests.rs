use crate::{
  actor_prim::actor_ref::{actor_ref_sender::ActorRefSender, null_sender::NullSender},
  error::SendError,
  messaging::AnyMessage,
};

#[test]
fn always_returns_closed_error() {
  let sender = NullSender;
  let error: SendError = sender.send(AnyMessage::new(1_u8)).unwrap_err();
  match error {
    | SendError::Closed(_) => {},
    | other => panic!("expected closed error, got {other:?}"),
  }
}
