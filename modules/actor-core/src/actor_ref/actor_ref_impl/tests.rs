#![cfg(test)]

use core::sync::atomic::{AtomicUsize, Ordering};

use cellactor_utils_core_rs::sync::ArcShared;

use super::{ActorRef, ActorRefSender};
use crate::{NoStdToolbox, any_message::AnyMessage, pid::Pid, send_error::SendError};

struct RecordingSender {
  count: ArcShared<AtomicUsize>,
}

impl RecordingSender {
  fn new() -> (ArcShared<AtomicUsize>, ArcShared<Self>) {
    let count = ArcShared::new(AtomicUsize::new(0));
    let sender = ArcShared::new(Self { count: count.clone() });
    (count, sender)
  }
}

impl ActorRefSender<NoStdToolbox> for RecordingSender {
  fn send(&self, _message: AnyMessage<NoStdToolbox>) -> Result<(), SendError<NoStdToolbox>> {
    use core::sync::atomic::Ordering;
    self.count.fetch_add(1, Ordering::Relaxed);
    Ok(())
  }
}

#[test]
fn null_sender_rejects_messages() {
  let null: ActorRef<NoStdToolbox> = ActorRef::null();
  assert!(null.tell(AnyMessage::new(1_u32)).is_err());
}

#[test]
fn new_actor_ref_forwards_messages() {
  let (count, sender) = RecordingSender::new();
  let actor: ActorRef<NoStdToolbox> = ActorRef::new(Pid::new(1, 0), sender);
  assert!(actor.tell(AnyMessage::new(42_u32)).is_ok());
  assert_eq!(count.load(Ordering::Relaxed), 1);
}
