use alloc::boxed::Box;

use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess, SpinSyncMutex};

use super::ActorRefSenderShared;
use crate::actor::{
  actor_ref::{ActorRefSender, SendOutcome},
  error::SendError,
  messaging::AnyMessage,
};

struct RecordingSender {
  sends: ArcShared<SpinSyncMutex<usize>>,
}

impl ActorRefSender for RecordingSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    *self.sends.lock() += 1;
    Ok(SendOutcome::Delivered)
  }
}

struct SchedulingSender {
  scheduled: ArcShared<SpinSyncMutex<bool>>,
}

impl ActorRefSender for SchedulingSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    let scheduled = self.scheduled.clone();
    Ok(SendOutcome::Schedule(Box::new(move || {
      *scheduled.lock() = true;
    })))
  }
}

#[test]
fn send_delegates_to_inner_sender() {
  let sends = ArcShared::new(SpinSyncMutex::new(0));
  let mut shared = ActorRefSenderShared::new(Box::new(RecordingSender { sends: sends.clone() }));

  shared.send(AnyMessage::new("payload")).expect("send");

  assert_eq!(*sends.lock(), 1);
}

#[test]
fn send_runs_scheduled_outcome_after_lock_is_released() {
  let scheduled = ArcShared::new(SpinSyncMutex::new(false));
  let mut shared = ActorRefSenderShared::new(Box::new(SchedulingSender { scheduled: scheduled.clone() }));

  shared.send(AnyMessage::new("payload")).expect("send");

  assert!(*scheduled.lock());
}

#[test]
fn shared_access_write_reaches_inner_sender() {
  let sends = ArcShared::new(SpinSyncMutex::new(0));
  let shared = ActorRefSenderShared::new(Box::new(RecordingSender { sends: sends.clone() }));
  let cloned = shared.clone();

  shared.with_write(|sender| {
    let _outcome = sender.send(AnyMessage::new("payload")).expect("send");
  });
  cloned.with_write(|sender| {
    let _outcome = sender.send(AnyMessage::new("payload")).expect("send");
  });

  assert_eq!(*sends.lock(), 2);
}
